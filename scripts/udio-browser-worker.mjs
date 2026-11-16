import { chromium } from "playwright-core";
import process from "node:process";
import {
  DEFAULT_TIMEOUT_MS, DEFAULT_WAIT_TIMEOUT_MS, DEFAULT_POLL_INTERVAL_MS,
  DEFAULT_CHALLENGE_RETRY_INTERVAL_MS, DEFAULT_LOCALE,
  DEFAULT_NAVIGATION_TIMEOUT_MS, DEFAULT_CDP_CONNECT_TIMEOUT_MS,
  normalizeString, normalizeTimeoutMs, normalizeBaseUrl, normalizeTargetAssetKind,
  normalizeLocale, parseBoolean, validateInput, deepClone,
} from "./udio-browser/request.mjs";
import { resolveUdioAccessToken, parseCookieHeader } from "./udio-browser/auth.mjs";
import { maybeReadRuntimeState, maybePersistRuntimeState, hasPersistableRuntimeState } from "./udio-browser/storage.mjs";
import { debugLog } from "./udio-browser/diagnostics.mjs";
import { safePageUrl, resolveExecutablePath, resolveBorrowedContext, resolveWorkerPage, createWorkerPage, openChallengeSurface } from "./udio-browser/browser.mjs";
import { requireJsonResponse, normalizeError } from "./udio-browser/responses.mjs";
import { runWithChallengeRetries, browserFetch } from "./udio-browser/transport.mjs";
import { refreshCaptchaToken } from "./udio-browser/captcha.mjs";
import { submitGenerationThroughPageUi } from "./udio-browser/native-flow.mjs";
import { runGeneration } from "./udio-browser/generation.mjs";

async function main() {
  let browser = null;
  let closeBrowser = async () => undefined;
  let context = null;
  let runtimeStateObjectKey = null;
  let storedState = null;
  let debugLogPath = null;
  try {
    const raw = await readStdin();
    const input = JSON.parse(raw);
    validateInput(input);

    runtimeStateObjectKey = normalizeString(input.runtimeStateObjectKey);
    debugLogPath = normalizeString(
      input.debugLogPath ?? process.env.UDIO_BROWSER_DEBUG_LOG_PATH ?? null,
    );
    const timeoutMs = normalizeTimeoutMs(input.timeoutMs, DEFAULT_TIMEOUT_MS, 30_000, 30 * 60 * 1000);
    const overallDeadline = Date.now() + timeoutMs;
    const waitTimeoutMs = normalizeTimeoutMs(
      input.waitTimeoutMs,
      DEFAULT_WAIT_TIMEOUT_MS,
      15_000,
      timeoutMs,
    );
    const pollIntervalMs = normalizeTimeoutMs(
      input.pollIntervalMs,
      DEFAULT_POLL_INTERVAL_MS,
      1_000,
      10_000,
    );
    const challengeRetryIntervalMs = normalizeTimeoutMs(
      process.env.UDIO_BROWSER_CHALLENGE_RETRY_INTERVAL_MS,
      DEFAULT_CHALLENGE_RETRY_INTERVAL_MS,
      1_000,
      30_000,
    );
    const baseUrl = normalizeBaseUrl(input.baseUrl);
    const targetAssetKind = normalizeTargetAssetKind(input.targetAssetKind);
    const origin = normalizeString(input.origin) ?? baseUrl;
    const referer =
      normalizeString(input.referer) ??
      normalizeString(input.browserCdpTargetUrl ?? process.env.UDIO_BROWSER_CDP_TARGET_URL ?? null) ??
      `${baseUrl}/create`;
    const locale = normalizeLocale(input.acceptLanguage) ?? DEFAULT_LOCALE;
    const navigationTimeoutMs = Math.min(timeoutMs, DEFAULT_NAVIGATION_TIMEOUT_MS);
    const cdpConnectTimeoutMs = Math.min(
      timeoutMs,
      normalizeTimeoutMs(
        input.browserCdpConnectTimeoutMs ?? process.env.UDIO_BROWSER_CDP_CONNECT_TIMEOUT_MS,
        DEFAULT_CDP_CONNECT_TIMEOUT_MS,
        5_000,
        5 * 60 * 1000,
      ),
    );
    const browserCdpUrl = normalizeString(
      input.browserCdpUrl ?? process.env.UDIO_BROWSER_CDP_URL ?? null,
    );
    const browserCdpTargetUrl =
      normalizeString(
        input.browserCdpTargetUrl ?? process.env.UDIO_BROWSER_CDP_TARGET_URL ?? null,
      ) ?? `${baseUrl}/create`;
    const headless = browserCdpUrl ? false : parseBoolean(process.env.UDIO_BROWSER_HEADLESS, true);
    const manualChallengeWaitMs = headless
      ? 0
      : normalizeTimeoutMs(process.env.UDIO_HCAPTCHA_MANUAL_WAIT_MS, 0, 0, timeoutMs);
    await debugLog(debugLogPath, "worker_start", {
      baseUrl,
      targetAssetKind,
      hasBrowserCdpUrl: Boolean(browserCdpUrl),
      hasRuntimeStateObjectKey: Boolean(runtimeStateObjectKey),
      hasCookieHeader: Boolean(normalizeString(input.cookieHeader)),
      timeoutMs,
      waitTimeoutMs,
      pollIntervalMs,
      manualChallengeWaitMs,
      cdpConnectTimeoutMs,
      model: normalizeString(input.requestBody?.model),
      responseFormat: normalizeString(
        input.requestBody?.response_format ?? input.requestBody?.responseFormat,
      ),
    });

    try {
      const cookieHeader = normalizeString(input.cookieHeader);
      if (browserCdpUrl) {
        await debugLog(debugLogPath, "browser_connect_cdp_start", {
          browserCdpUrl,
          browserCdpTargetUrl,
          cdpConnectTimeoutMs,
        });
        browser = await chromium.connectOverCDP(browserCdpUrl, {
          timeout: cdpConnectTimeoutMs,
        });
        // Playwright close disconnects only this CDP client; it does not own the borrowed browser.
        closeBrowser = async () => { await browser.close(); };
        context = resolveBorrowedContext(browser, baseUrl, browserCdpTargetUrl, referer);
        await debugLog(debugLogPath, "browser_connect_cdp_ready", {
          contextPageCount: context.pages().length,
        });
      } else {
        const executablePath = resolveExecutablePath(
          input.browserExecutablePath ?? process.env.UDIO_BROWSER_EXECUTABLE_PATH ?? null,
        );
        if (!executablePath) {
          throw Object.assign(
            new Error("Unable to locate a Chromium-compatible browser. Set UDIO_BROWSER_EXECUTABLE_PATH."),
            { status: 500, code: "udio_browser_not_found" },
          );
        }

        browser = await chromium.launch({
          executablePath,
          headless,
          args: [
            "--disable-blink-features=AutomationControlled",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
          ],
        });
        closeBrowser = async () => {
          await browser?.close();
        };

        const contextOptions = {
          locale,
          userAgent: normalizeString(input.userAgent) ?? undefined,
        };
        storedState = await maybeReadRuntimeState(runtimeStateObjectKey);
        await debugLog(debugLogPath, "browser_launch_ready", {
          executablePath,
          headless,
          hasStoredState: Boolean(storedState),
        });
        if (runtimeStateObjectKey && !storedState && !cookieHeader) {
          throw Object.assign(
            new Error(
              "Udio browser runtime state could not be loaded. Re-export storageState or provide cookieHeader.",
            ),
            {
              status: 500,
              code: "udio_browser_runtime_state_unavailable",
            },
          );
        }
        if (
          runtimeStateObjectKey &&
          storedState &&
          !hasPersistableRuntimeState(storedState) &&
          !cookieHeader
        ) {
          throw Object.assign(
            new Error(
              "Udio browser runtime state is present but no longer contains authenticated session material.",
            ),
            {
              status: 401,
              code: "udio_browser_runtime_state_missing_auth",
            },
          );
        }
        if (storedState) {
          contextOptions.storageState = storedState;
        }

        context = await browser.newContext(contextOptions);
      }
      // The exported runtime state is refreshed by the signed-in helper and is
      // newer than a static credential cookie. Do not overwrite it with stale
      // cookie material after the context has been created.
      if (cookieHeader && !runtimeStateObjectKey && !browserCdpUrl) {
        await context.addCookies(parseCookieHeader(cookieHeader, baseUrl));
        await debugLog(debugLogPath, "cookie_header_loaded", {
          cookieCount: parseCookieHeader(cookieHeader, baseUrl).length,
        });
      }

      const page = browserCdpUrl
        ? await resolveWorkerPage(context, {
            targetUrl: browserCdpTargetUrl,
            fallbackUrl: referer,
            navigationTimeoutMs,
            borrowedContext: true,
          })
        : await createWorkerPage(context, referer, navigationTimeoutMs);
      await debugLog(debugLogPath, "worker_page_ready", {
        url: safePageUrl(page),
      });

      const requestBody = deepClone(input.requestBody);
      const authToken = await resolveUdioAccessToken(page, baseUrl);
      await debugLog(debugLogPath, "auth_token_resolved", {
        hasAuthToken: Boolean(authToken),
      });
      if (!browserCdpUrl) {
        await debugLog(debugLogPath, "captcha_probe_start", {});
        const captchaRequirement = requireJsonResponse(
          await runWithChallengeRetries({
            page,
            overallDeadline,
            retryIntervalMs: challengeRetryIntervalMs,
            onChallenge: async () => {
              await openChallengeSurface(page, baseUrl, referer, navigationTimeoutMs, {
                allowNavigation: true,
              });
            },
            action: () =>
              browserFetch(page, {
                requestUrl: `${baseUrl}/api/generate-proxy/captcha`,
                method: "GET",
                headers: {
                  accept: "application/json, text/plain, */*",
                  origin,
                  referer,
                  ...(authToken ? { authorization: authToken } : {}),
                },
                timeoutMs: Math.min(timeoutMs, 30_000),
              }),
          }),
          "udio_invalid_captcha_json",
          "Udio captcha probe returned invalid JSON.",
        );
        await debugLog(debugLogPath, "captcha_probe_result", {
          required: Boolean(captchaRequirement?.required),
        });
        if (captchaRequirement?.required) {
          delete requestBody.captchaToken;
          requestBody.captchaToken = await refreshCaptchaToken(page, {
            baseUrl,
            referer,
            navigationTimeoutMs,
            manualChallengeWaitMs,
            debugLogPath,
            allowNavigation: true,
          });
          await debugLog(debugLogPath, "captcha_token_ready", {
            tokenLength: requestBody.captchaToken?.length ?? 0,
            source: "pre_submit",
          });
        }
      } else {
        await debugLog(debugLogPath, "native_submit_selected", {});
      }

      const headers = { accept: "application/json", origin, referer, ...(authToken ? { authorization: authToken } : {}) };
      return await runGeneration({
        deadline: overallDeadline, now: () => Date.now(), waitTimeoutMs, pollIntervalMs, targetAssetKind,
        waitAudio: parseBoolean(input.waitAudio, true), wait: (ms) => page.waitForTimeout(ms),
        submit: (remainingMs) => browserCdpUrl
          ? submitGenerationThroughPageUi(page, { baseUrl, requestBody, timeoutMs: remainingMs })
          : browserFetch(page, { requestUrl: `${baseUrl}/api/generate-proxy`, method: "POST",
              headers: { ...headers, "content-type": "application/json" }, bodyText: JSON.stringify(requestBody),
              timeoutMs: Math.min(remainingMs, 60_000) }),
        poll: (ids, remainingMs) => browserFetch(page, {
          requestUrl: `${baseUrl}/api/songs?songIds=${encodeURIComponent(ids.join(","))}&readOnly=false&checkIfReadyToStream=true`,
          method: "GET", headers, timeoutMs: Math.min(remainingMs, 45_000),
        }),
      });
    } finally {
      if (context && runtimeStateObjectKey) {
        await maybePersistRuntimeState(context, runtimeStateObjectKey, storedState, baseUrl).catch(
          () => undefined,
        );
      }
      try { await closeBrowser(); }
      catch { throw Object.assign(new Error("Udio browser cleanup failed; remote generation may still exist."), {
        status: 502, code: "udio_browser_cleanup_failed",
      }); }
    }
  } catch (error) {
    await debugLog(debugLogPath, "worker_error", normalizeError(error));
    return {
      ok: false,
      error: normalizeError(error),
    };
  }
}

async function readStdin() {
  const chunks = [];
  let bytes = 0;
  for await (const chunk of process.stdin) {
    const buffer = Buffer.from(chunk);
    bytes += buffer.length;
    if (bytes > 1024 * 1024) throw Object.assign(new Error("Udio worker input exceeds 1 MiB."), {
      status: 413, code: "udio_browser_worker_input_too_large",
    });
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString("utf8");
}

async function writeJson(value) {
  // Await pipe flush; force-exit can truncate JSON and bypass remaining handle cleanup.
  await new Promise((resolve, reject) => process.stdout.write(`${JSON.stringify(value)}\n`, error => error ? reject(error) : resolve()));
}

// Emit and terminate only after main's persistence and owned-browser cleanup.
main().then(writeJson);
