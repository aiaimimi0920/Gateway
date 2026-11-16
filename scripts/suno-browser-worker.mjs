import { chromium } from "playwright-core";
import process from "node:process";
import {
  validateInput, normalizeString, normalizeBaseUrl, normalizeLocale, normalizeTimeoutMs,
  normalizeTargetAssetKind, parseBoolean, extractPrompt, createError, normalizeError,
} from "./suno-browser/pure.mjs";
import {
  resolveBrowserExecutionTarget, releaseEasyBrowserLease, pageLooksChallenged,
  pageLooksSignedOut, runCreateThroughPageUi,
  parseCookieHeader, resolveExecutablePath, resolveWorkerPage, dismissKnownBlockingOverlays,
} from "./suno-browser/browser.mjs";

import { createSunoResponses } from "./suno-browser/responses.mjs";
import { waitForGeneration } from "./suno-browser/generation.mjs";
import { boundNativeModel, createNativeModelGate } from "./suno-browser/native-model.mjs";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_WAIT_TIMEOUT_MS = 150_000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_NAVIGATION_TIMEOUT_MS = 45_000;

async function main() {
  let browser = null;
  let context = null;
  let easyBrowserLease = null;
  let responses = null;
  let modelGate = null;
  async function generate() {
    const raw = await readStdin();
    const input = JSON.parse(raw);
    validateInput(input);

    const timeoutMs = normalizeTimeoutMs(input.timeoutMs, DEFAULT_TIMEOUT_MS, 30_000, 30 * 60 * 1000);
    const deadline = Date.now() + timeoutMs;
    const waitTimeoutMs = normalizeTimeoutMs(
      input.waitTimeoutMs,
      DEFAULT_WAIT_TIMEOUT_MS,
      10_000,
      timeoutMs,
    );
    const waitCompletion = parseBoolean(input.waitCompletion, true);
    const baseUrl = normalizeBaseUrl(input.baseUrl);
    const origin = normalizeString(input.origin) ?? "https://suno.com";
    const referer = normalizeString(input.referer) ?? `${origin.replace(/\/+$/, "")}/create`;
    const locale = normalizeLocale(input.acceptLanguage) ?? DEFAULT_LOCALE;
    const navigationTimeoutMs = Math.min(timeoutMs, DEFAULT_NAVIGATION_TIMEOUT_MS);
    const browserCdpUrl = normalizeString(
      input.browserCdpUrl ?? process.env.SUNO_BROWSER_CDP_URL ?? null,
    );
    const browserCdpTargetUrl =
      normalizeString(
        input.browserCdpTargetUrl ?? process.env.SUNO_BROWSER_CDP_TARGET_URL ?? null,
      ) ?? referer;
    const prompt = extractPrompt(input.requestBody);
    if (!prompt) {
      throw createError(
        400,
        "suno_browser_missing_prompt",
        "Suno browser worker requires a non-empty prompt-compatible field.",
      );
    }
    const targetAssetKind = normalizeTargetAssetKind(input.targetAssetKind);
    const wireModel = boundNativeModel(input.requestBody);
    const effectiveBrowserTarget = await resolveBrowserExecutionTarget({
      input,
      browserCdpUrl,
      browserCdpTargetUrl,
      referer,
      timeoutMs: navigationTimeoutMs,
    });
    easyBrowserLease = effectiveBrowserTarget.lease ?? null;

    const cookieHeader = normalizeString(input.cookieHeader);

    if (effectiveBrowserTarget.browserCdpUrl) {
      browser = await chromium.connectOverCDP(effectiveBrowserTarget.browserCdpUrl, { timeout: navigationTimeoutMs });
      context =
        browser
          .contexts()
          .find((candidate) =>
            candidate
              .pages()
              .some((candidatePage) => candidatePage.url().startsWith("https://suno.com")),
          ) ??
        browser.contexts()[0] ??
        (await browser.newContext());
    } else {
      const executablePath = resolveExecutablePath(
        input.browserExecutablePath ?? process.env.SUNO_BROWSER_EXECUTABLE_PATH ?? null,
      );
      if (!executablePath) {
        throw createError(
          500,
          "suno_browser_not_found",
          "Unable to locate a Chromium-compatible browser. Set SUNO_BROWSER_EXECUTABLE_PATH.",
        );
      }

      browser = await chromium.launch({
        executablePath, timeout: navigationTimeoutMs,
        headless: parseBoolean(process.env.SUNO_BROWSER_HEADLESS, true),
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
        ],
      });

      context = await browser.newContext({
        locale,
        userAgent: normalizeString(input.userAgent) ?? undefined,
      });
    }

    if (cookieHeader && !effectiveBrowserTarget.browserCdpUrl) {
      await context.addCookies(parseCookieHeader(cookieHeader));
    }

    const page = await resolveWorkerPage(context, {
      targetUrl: effectiveBrowserTarget.browserCdpTargetUrl,
      fallbackUrl: referer,
      navigationTimeoutMs,
      forceNavigate: Boolean(cookieHeader && !effectiveBrowserTarget.browserCdpUrl),
      borrowedContext: Boolean(effectiveBrowserTarget.browserCdpUrl),
    });
    await dismissKnownBlockingOverlays(page);

    if (await pageLooksChallenged(page)) {
      throw createError(
        403,
        "suno_browser_challenge_required",
        "Suno browser worker hit an active challenge before generation could start.",
      );
    }

    if (await pageLooksSignedOut(page)) {
      throw createError(
        401,
        "suno_browser_auth_required",
        "Suno browser worker did not find an authenticated create page session.",
      );
    }

    modelGate = createNativeModelGate(page, baseUrl, wireModel, prompt, reason => responses?.fail(reason));
    await modelGate.ready;
    modelGate.check();
    responses = createSunoResponses(page, baseUrl, undefined, modelGate.admitted);
    await responses.ready;
    if (Date.now() >= deadline) throw createError(504, "suno_browser_wait_timeout", "Suno request deadline expired before submission.");
    const createBody = await runCreateThroughPageUi(page, {
      prompt, responses, modelGate,
      timeoutMs: deadline - Date.now(),
    }).catch(async (error) => {
      if (typeof error?.code === "string" && error.code.startsWith("suno_")) {
        throw error;
      }
      if (await pageLooksChallenged(page)) {
        throw createError(
          403,
          "suno_browser_challenge_required",
          "Suno browser worker hit a challenge while sending the generate request.",
        );
      }
      throw createError(
        504,
        "suno_browser_generation_failed",
        "Suno browser worker failed to execute the generate request; details omitted.",
      );
    });

    const result = await waitForGeneration(page, responses, createBody, {
      targetAssetKind, waitCompletion, waitTimeoutMs, deadline,
    });
    modelGate.check();
    result.result.wireModel = wireModel;
    return result;
  }
  let outcome;
  try {
    outcome = await generate();
  } catch (error) {
    outcome = { ok: false, error: normalizeError(error) };
  }
  // Close an owned browser; for connectOverCDP this disconnects only our connection.
  // Always attempt lease release even when disconnect/close fails.
  let cleanupFailed = false;
  try { await modelGate?.stop(); } catch { cleanupFailed = true; }
  try { await responses?.stop(); } catch { cleanupFailed = true; }
  try { await browser?.close(); } catch { cleanupFailed = true; }
  try { await releaseEasyBrowserLease(easyBrowserLease); } catch { cleanupFailed = true; }
  if (cleanupFailed) {
    const message = "Suno browser resource cleanup failed; remote generation may still exist.";
    if (outcome.ok) {
      outcome = { ok: false, error: normalizeError(createError(502, "suno_browser_cleanup_failed", message)) };
    } else {
      outcome.error.message = `${outcome.error.message} ${message}`;
    }
  }
  return outcome;
}

async function writeJson(value) {
  // Do not force process.exit(): it bypasses cleanup and can truncate piped stdout.
  await new Promise((resolve, reject) => {
    process.stdout.write(`${JSON.stringify(value)}\n`, (error) => error ? reject(error) : resolve());
  });
}

async function readStdin() {
  const chunks = [];
  let bytes = 0;
  for await (const chunk of process.stdin) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    bytes += buffer.length;
    if (bytes > 1024 * 1024) {
      throw createError(413, "suno_browser_worker_input_too_large", "Suno worker input exceeds 1 MiB.");
    }
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString("utf8");
}

await writeJson(await main());
