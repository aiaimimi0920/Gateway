import { chromium } from "playwright-core";
import process from "node:process";
import {
  validateInput, normalizeString, normalizeBaseUrl, normalizeLocale, normalizeTimeoutMs,
  normalizeTargetAssetKind, parseBoolean, extractPrompt, readClips, collectClipIds,
  clipsTerminal, clipsReadyForTarget, classifyChallengeLikeBody, createError, normalizeError,
} from "./suno-browser/pure.mjs";
import {
  resolveBrowserExecutionTarget, releaseEasyBrowserLease, pageLooksChallenged,
  pageLooksSignedOut, runCreateThroughPageUi, maybeClassifyVideoSubscriptionRequired,
  parseCookieHeader, resolveExecutablePath, resolveWorkerPage, dismissKnownBlockingOverlays,
} from "./suno-browser/browser.mjs";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_WAIT_TIMEOUT_MS = 150_000;
const DEFAULT_POLL_INTERVAL_MS = 3_000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_NAVIGATION_TIMEOUT_MS = 45_000;

async function main() {
  let browser = null;
  let context = null;
  let closeBrowser = async () => undefined;
  let easyBrowserLease = null;
  try {
    const raw = await readStdin();
    const input = JSON.parse(raw);
    validateInput(input);

    const timeoutMs = normalizeTimeoutMs(input.timeoutMs, DEFAULT_TIMEOUT_MS, 30_000, 30 * 60 * 1000);
    const waitTimeoutMs = normalizeTimeoutMs(
      input.waitTimeoutMs,
      DEFAULT_WAIT_TIMEOUT_MS,
      10_000,
      timeoutMs,
    );
    const pollIntervalMs = normalizeTimeoutMs(
      input.pollIntervalMs,
      DEFAULT_POLL_INTERVAL_MS,
      1_000,
      10_000,
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
      browser = await chromium.connectOverCDP(effectiveBrowserTarget.browserCdpUrl);
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
        executablePath,
        headless: parseBoolean(process.env.SUNO_BROWSER_HEADLESS, true),
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
        ],
      });
      closeBrowser = async () => {
        await browser?.close().catch(() => undefined);
      };

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

    const createOutcome = await runCreateThroughPageUi(page, {
      prompt,
      timeoutMs,
    }).catch(async (error) => {
      if (error?.code === "suno_browser_challenge_required") {
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
        `Suno browser worker failed to execute the generate request: ${error.message}`,
      );
    });

    const createStatus = createOutcome.status;
    const createBodyText = createOutcome.bodyText;
    if (createStatus < 200 || createStatus >= 300) {
      const code = classifyChallengeLikeBody(createBodyText)
        ? "suno_browser_challenge_required"
        : "suno_browser_generation_failed";
      throw createError(
        createStatus,
        code,
        `Suno browser generation returned HTTP ${createStatus}.`,
        createBodyText,
      );
    }

    let createBody;
    try {
      createBody = JSON.parse(createBodyText);
    } catch (error) {
      throw createError(
        500,
        "suno_browser_invalid_generation_json",
        `Suno browser generation returned invalid JSON: ${error.message}`,
        createBodyText,
      );
    }

    let clips = readClips(createBody);
    const clipIds = collectClipIds(clips);
    if (!clipIds.length) {
      throw createError(
        500,
        "suno_browser_missing_clip_ids",
        "Suno browser generation did not return any clip ids.",
        createBodyText,
      );
    }

    const targetAssetKind = normalizeTargetAssetKind(input.targetAssetKind);
    if (!waitCompletion || clipsReadyForTarget(clips, targetAssetKind)) {
      printJsonAndExit({
        ok: true,
        status: 200,
        result: {
          clips,
          completed: clipsTerminal(clips),
          message: null,
        },
      });
      return;
    }

    if (targetAssetKind === "video" && (await maybeClassifyVideoSubscriptionRequired(page))) {
      throw createError(
        402,
        "suno_video_subscription_required",
        "Suno video generation requires an entitled account on the current Publish -> Animate web flow.",
      );
    }

    const waitDeadline = Date.now() + waitTimeoutMs;
    while (Date.now() < waitDeadline) {
      const remainingMs = waitDeadline - Date.now();
      if (remainingMs <= 0) {
        break;
      }

      let feedResponse;
      try {
        feedResponse = await page.waitForResponse(
          (response) =>
            /\/api\/feed\/v3\/?$/.test(response.url()) &&
            response.request().method().toUpperCase() === "POST",
          {
            timeout: Math.max(1_000, Math.min(remainingMs, pollIntervalMs + 5_000)),
          },
        );
      } catch (error) {
        if (String(error?.message ?? "").toLowerCase().includes("timeout")) {
          continue;
        }
        throw error;
      }

      const pollOutcome = {
        status: feedResponse.status(),
        bodyText: await feedResponse.text(),
      };

      if (pollOutcome.status < 200 || pollOutcome.status >= 300) {
        const code = classifyChallengeLikeBody(pollOutcome.bodyText)
          ? "suno_browser_challenge_required"
          : "suno_browser_generation_failed";
        throw createError(
          pollOutcome.status,
          code,
          `Suno browser feed polling returned HTTP ${pollOutcome.status}.`,
          pollOutcome.bodyText,
        );
      }

      let pollBody;
      try {
        pollBody = JSON.parse(pollOutcome.bodyText);
      } catch (error) {
        throw createError(
          500,
          "suno_browser_invalid_generation_json",
          `Suno browser feed polling returned invalid JSON: ${error.message}`,
          pollOutcome.bodyText,
        );
      }

      const polledClips = readClips(pollBody);
      const matchedClips = clipIds.length
        ? polledClips.filter((clip) => clipIds.includes(normalizeString(clip?.id)))
        : polledClips;
      if (matchedClips.length) {
        clips = matchedClips;
      }
      if (clipsReadyForTarget(clips, targetAssetKind)) {
        printJsonAndExit({
          ok: true,
          status: 200,
          result: {
            clips,
            completed: true,
            message: null,
          },
        });
        return;
      }

      if (targetAssetKind === "video" && clipsTerminal(clips)) {
        if (await maybeClassifyVideoSubscriptionRequired(page)) {
          throw createError(
            402,
            "suno_video_subscription_required",
            "Suno video generation requires an entitled account on the current Publish -> Animate web flow.",
          );
        }
        break;
      }
    }

    printJsonAndExit({
      ok: true,
      status: 200,
      result: {
        clips,
        completed: clipsReadyForTarget(clips, normalizeTargetAssetKind(input.targetAssetKind)),
        message: "Timed out waiting for Suno clip generation to reach a terminal state.",
      },
    });
  } catch (error) {
    printJsonAndExit({
      ok: false,
      error: normalizeError(error),
    });
  } finally {
    await closeBrowser();
    await releaseEasyBrowserLease(easyBrowserLease).catch(() => undefined);
  }
}

function printJsonAndExit(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
  process.exit(0);
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

await main();
