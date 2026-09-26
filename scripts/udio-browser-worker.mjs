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
import { requireJsonResponse, isChallengeOutcome, trimBody, normalizeError, extractTrackIds, readSongs, songsReadyForTarget, timeoutMessageForTarget } from "./udio-browser/responses.mjs";
import { runWithChallengeRetries, browserFetch } from "./udio-browser/transport.mjs";
import { refreshCaptchaToken } from "./udio-browser/captcha.mjs";
import { submitGenerationThroughPageUi, resolveVercelCheckpointInBorrowedPage } from "./udio-browser/native-flow.mjs";

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
          await browser?.close().catch(() => undefined);
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
      const overallDeadline = Date.now() + timeoutMs;
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

      await debugLog(debugLogPath, "submit_start", {
        hasCaptchaToken: Boolean(normalizeString(requestBody.captchaToken)),
        promptLength:
          normalizeString(requestBody.gen_params?.prompt ?? requestBody.prompt)?.length ?? 0,
      });
      const submitGeneration = async () => {
        const outcome = browserCdpUrl
          ? await submitGenerationThroughPageUi(page, {
              baseUrl,
              requestBody,
              timeoutMs: Math.min(
                Math.max(60_000, manualChallengeWaitMs + 60_000),
                Math.max(60_000, overallDeadline - Date.now()),
              ),
            })
          : await browserFetch(page, {
              requestUrl: `${baseUrl}/api/generate-proxy`,
              method: "POST",
              headers: {
                accept: "application/json, text/plain, */*",
                "content-type": "application/json",
                origin,
                referer,
                ...(authToken ? { authorization: authToken } : {}),
              },
              bodyText: JSON.stringify(requestBody),
              timeoutMs: Math.min(timeoutMs, 60_000),
            });
        await debugLog(debugLogPath, "submit_action_result", {
          ok: outcome.ok,
          status: outcome.status,
          contentType: outcome.contentType,
          mitigated: outcome.mitigated,
          transportError: Boolean(outcome.transportError),
          textLength: String(outcome.text ?? "").length,
          challengeOutcome: isChallengeOutcome(outcome),
        });
        return outcome;
      };
      let submitOutcome = await submitGeneration();
      if (isChallengeOutcome(submitOutcome)) {
        if (browserCdpUrl) {
          await debugLog(debugLogPath, "native_checkpoint_start", {
            status: submitOutcome.status,
          });
          await resolveVercelCheckpointInBorrowedPage(page, {
            baseUrl,
            referer,
            timeoutMs: Math.min(
              Math.max(60_000, manualChallengeWaitMs),
              Math.max(60_000, overallDeadline - Date.now()),
            ),
          });
          await debugLog(debugLogPath, "native_checkpoint_ready", {});
        } else {
          await debugLog(debugLogPath, "submit_challenge_refresh_start", {});
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
            source: "submit_retry",
          });
        }
        submitOutcome = await submitGeneration();
        if (isChallengeOutcome(submitOutcome)) {
          throw Object.assign(
            new Error(
              "Udio requires a browser security check or captcha challenge before generation can continue.",
            ),
            {
              status: submitOutcome.status ?? 429,
              code: "udio_browser_challenge_required",
              body: trimBody(submitOutcome.text ?? ""),
            },
          );
        }
      }
      await debugLog(debugLogPath, "submit_outcome", {
        ok: submitOutcome.ok,
        status: submitOutcome.status,
        contentType: submitOutcome.contentType,
        mitigated: submitOutcome.mitigated,
        textLength: String(submitOutcome.text ?? "").length,
      });

      const submitJson = requireJsonResponse(
        submitOutcome,
        "udio_invalid_generation_json",
        "Udio generation returned invalid JSON.",
      );
      const trackIds = extractTrackIds(submitJson);
      let latestSongs = readSongs(submitJson);
      await debugLog(debugLogPath, "submit_json_ready", {
        trackIds,
        songCount: latestSongs.length,
      });
      if (!parseBoolean(input.waitAudio, true)) {
        await debugLog(debugLogPath, "worker_success_no_wait", {
          trackIds,
          completed: songsReadyForTarget(latestSongs, targetAssetKind),
        });
        return {
          ok: true,
          status: 200,
          result: {
            trackIds,
            songs: latestSongs,
            completed: songsReadyForTarget(latestSongs, targetAssetKind),
            message: null,
          },
        };
      }

      const waitDeadline = Date.now() + waitTimeoutMs;
      while (Date.now() < waitDeadline) {
        if (songsReadyForTarget(latestSongs, targetAssetKind)) {
          await debugLog(debugLogPath, "worker_success", {
            trackIds,
            completed: true,
            songCount: latestSongs.length,
          });
          return {
            ok: true,
            status: 200,
            result: {
              trackIds,
              songs: latestSongs,
              completed: true,
              message: null,
            },
          };
        }

        await page.waitForTimeout(Math.min(pollIntervalMs, Math.max(250, waitDeadline - Date.now())));
        if (Date.now() >= waitDeadline) {
          break;
        }

        const idsQuery = encodeURIComponent(trackIds.join(","));
        await debugLog(debugLogPath, "poll_start", {
          trackIds,
          remainingMs: Math.max(0, waitDeadline - Date.now()),
        });
        const pollOutcome = await runWithChallengeRetries({
          page,
          overallDeadline: waitDeadline,
          retryIntervalMs: challengeRetryIntervalMs,
          onChallenge: async () => {
            await openChallengeSurface(page, baseUrl, referer, navigationTimeoutMs, {
              allowNavigation: !browserCdpUrl,
            });
          },
          action: () =>
            browserFetch(page, {
              requestUrl: `${baseUrl}/api/songs?songIds=${idsQuery}&readOnly=false&checkIfReadyToStream=true`,
              method: "GET",
              headers: {
                accept: "application/json, text/plain, */*",
                origin,
                referer,
                ...(authToken ? { authorization: authToken } : {}),
              },
              timeoutMs: Math.min(timeoutMs, 45_000),
            }),
        });
        await debugLog(debugLogPath, "poll_outcome", {
          ok: pollOutcome.ok,
          status: pollOutcome.status,
          contentType: pollOutcome.contentType,
          textLength: String(pollOutcome.text ?? "").length,
        });
        const pollJson = requireJsonResponse(
          pollOutcome,
          "udio_invalid_poll_json",
          "Udio songs polling returned invalid JSON.",
        );
        latestSongs = readSongs(pollJson);
        await debugLog(debugLogPath, "poll_json_ready", {
          songCount: latestSongs.length,
          statuses: latestSongs.map((song) => String(song?.status ?? song?.state ?? "")),
        });
      }

      await debugLog(debugLogPath, "worker_timeout", {
        trackIds,
        completed: songsReadyForTarget(latestSongs, targetAssetKind),
        songCount: latestSongs.length,
      });
      return {
        ok: true,
        status: 200,
        result: {
          trackIds,
          songs: latestSongs,
          completed: songsReadyForTarget(latestSongs, targetAssetKind),
          message: timeoutMessageForTarget(targetAssetKind),
        },
      };
    } finally {
      if (context && runtimeStateObjectKey) {
        await maybePersistRuntimeState(context, runtimeStateObjectKey, storedState, baseUrl).catch(
          () => undefined,
        );
      }
      await closeBrowser();
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
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function printJsonAndExit(value) {
  process.stdout.write(JSON.stringify(value));
  process.exit(0);
}

// Emit and terminate only after main's persistence and owned-browser cleanup.
main().then(printJsonAndExit);
