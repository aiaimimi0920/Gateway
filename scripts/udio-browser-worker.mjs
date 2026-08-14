import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { GetObjectCommand, PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_WAIT_TIMEOUT_MS = 4 * 60 * 1000;
const DEFAULT_POLL_INTERVAL_MS = 3_000;
const DEFAULT_CHALLENGE_RETRY_INTERVAL_MS = 5_000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_NAVIGATION_TIMEOUT_MS = 45_000;
const DEFAULT_CDP_CONNECT_TIMEOUT_MS = 300_000;
const DEFAULT_HCAPTCHA_SITEKEY = "2945592b-1928-43a9-8473-7e7fed3d752e";
const DEFAULT_HCAPTCHA_API_SRC =
  "https://js.hcaptcha.com/1/api.js?render=explicit&sentry=false&uj=false";
const MAX_ERROR_BODY_LENGTH = 8_000;

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const MACOS_BROWSER_PATHS = [
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
];
const LINUX_BROWSER_PATHS = [
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
];

let objectStorageClient = null;

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
        context =
          browser
            .contexts()
            .find((candidate) =>
              candidate.pages().some((candidatePage) => candidatePage.url().startsWith(baseUrl)),
            ) ??
          browser.contexts()[0] ??
          (await browser.newContext({
            locale,
            userAgent: normalizeString(input.userAgent) ?? undefined,
          }));
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
        printJsonAndExit({
          ok: true,
          status: 200,
          result: {
            trackIds,
            songs: latestSongs,
            completed: songsReadyForTarget(latestSongs, targetAssetKind),
            message: null,
          },
        });
      }

      const waitDeadline = Date.now() + waitTimeoutMs;
      while (Date.now() < waitDeadline) {
        if (songsReadyForTarget(latestSongs, targetAssetKind)) {
          await debugLog(debugLogPath, "worker_success", {
            trackIds,
            completed: true,
            songCount: latestSongs.length,
          });
          printJsonAndExit({
            ok: true,
            status: 200,
            result: {
              trackIds,
              songs: latestSongs,
              completed: true,
              message: null,
            },
          });
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
      printJsonAndExit({
        ok: true,
        status: 200,
        result: {
          trackIds,
          songs: latestSongs,
          completed: songsReadyForTarget(latestSongs, targetAssetKind),
          message: timeoutMessageForTarget(targetAssetKind),
        },
      });
    } finally {
      if (context && runtimeStateObjectKey) {
        await maybePersistRuntimeState(context, runtimeStateObjectKey, storedState).catch(
          () => undefined,
        );
      }
      await closeBrowser();
    }
  } catch (error) {
    await debugLog(debugLogPath, "worker_error", normalizeError(error));
    printJsonAndExit({
      ok: false,
      error: normalizeError(error),
    });
  }
}

async function refreshCaptchaToken(page, options) {
  const {
    baseUrl,
    referer,
    navigationTimeoutMs,
    manualChallengeWaitMs,
    debugLogPath,
    allowNavigation,
  } = options;
  const sitekey =
    normalizeString(process.env.UDIO_HCAPTCHA_SITEKEY) ?? DEFAULT_HCAPTCHA_SITEKEY;
  const apiSrc =
    normalizeString(process.env.UDIO_HCAPTCHA_API_SRC) ?? DEFAULT_HCAPTCHA_API_SRC;
  await debugLog(debugLogPath, "captcha_refresh_start", {
    url: safePageUrl(page),
    manualChallengeWaitMs,
  });
  try {
    const token = await obtainHCaptchaToken(page, sitekey, apiSrc, manualChallengeWaitMs);
    await debugLog(debugLogPath, "captcha_refresh_ready", {
      tokenLength: token?.length ?? 0,
      mode: "current_page",
    });
    return token;
  } catch (error) {
    await debugLog(debugLogPath, "captcha_refresh_retry_surface", {
      message: error?.message ?? String(error),
    });
    if (!allowNavigation) {
      throw error;
    }
    await openChallengeSurface(page, baseUrl, referer, navigationTimeoutMs, {
      allowNavigation,
    });
    const token = await obtainHCaptchaToken(page, sitekey, apiSrc, manualChallengeWaitMs);
    await debugLog(debugLogPath, "captcha_refresh_ready", {
      tokenLength: token?.length ?? 0,
      mode: "challenge_surface",
    });
    return token;
  }
}

async function debugLog(logPath, stage, details = {}) {
  const normalizedPath = normalizeString(logPath);
  if (!normalizedPath) {
    return;
  }
  const entry = {
    ts: new Date().toISOString(),
    stage,
    details,
  };
  await mkdir(path.dirname(normalizedPath), { recursive: true }).catch(() => undefined);
  await appendFile(normalizedPath, `${JSON.stringify(entry)}\n`, "utf8").catch(() => undefined);
}

function safePageUrl(page) {
  try {
    return page?.url?.() ?? null;
  } catch {
    return null;
  }
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeBaseUrl(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "udio_browser_missing_base_url",
    });
  }
  return normalized.replace(/\/+$/, "");
}

function normalizeLocale(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    return null;
  }
  return normalized.split(",")[0]?.trim() || null;
}

function normalizeTimeoutMs(value, fallback, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(Math.max(Math.trunc(parsed), min), max);
}

function normalizeTargetAssetKind(value) {
  const normalized = normalizeString(String(value ?? ""))?.toLowerCase();
  if (!normalized) {
    return "audio";
  }
  if (["image", "audio", "video"].includes(normalized)) {
    return normalized;
  }
  throw Object.assign(new Error("targetAssetKind must be image, audio, or video."), {
    status: 400,
    code: "udio_browser_invalid_target_asset_kind",
  });
}

function parseBoolean(value, fallback) {
  if (typeof value === "boolean") {
    return value;
  }
  const normalized = normalizeString(String(value ?? ""))?.toLowerCase();
  if (!normalized) {
    return fallback;
  }
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }
  return fallback;
}

async function createWorkerPage(context, referer, navigationTimeoutMs) {
  const page = await context.newPage();
  await gotoUdio(page, referer, navigationTimeoutMs);
  await page.waitForTimeout(1_500);
  return page;
}

async function resolveWorkerPage(context, options) {
  const { targetUrl, fallbackUrl, navigationTimeoutMs, borrowedContext } = options;
  const target = safeUrl(targetUrl);
  const fallback = safeUrl(fallbackUrl);
  const existingPage = context
    .pages()
    .find((page) => pageMatchesTarget(page, target) || pageMatchesTarget(page, fallback));
  if (borrowedContext && !existingPage) {
    throw new Error("Udio CDP session does not contain the expected create page.");
  }
  const page = existingPage ?? (await context.newPage());

  if (!borrowedContext && (!existingPage || !pageMatchesTarget(page, target))) {
    await page.goto(target?.toString() ?? fallbackUrl, {
      waitUntil: "domcontentloaded",
      timeout: navigationTimeoutMs,
    });
  } else {
    await page.waitForLoadState("domcontentloaded", { timeout: navigationTimeoutMs }).catch(
      () => undefined,
    );
  }
  await page.waitForTimeout(1_500);
  return page;
}

function safeUrl(value) {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

function pageMatchesTarget(page, targetUrl) {
  if (!targetUrl) {
    return false;
  }
  try {
    const current = new URL(page.url());
    return (
      current.origin === targetUrl.origin &&
      normalizePathname(current.pathname) === normalizePathname(targetUrl.pathname)
    );
  } catch {
    return false;
  }
}

function normalizePathname(value) {
  const normalized = String(value || "/").trim();
  if (!normalized) {
    return "/";
  }
  return normalized.replace(/\/+$/, "") || "/";
}

function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_BROWSER_PATHS
      : process.platform === "darwin"
        ? MACOS_BROWSER_PATHS
        : LINUX_BROWSER_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

function validateInput(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw Object.assign(new Error("Expected a JSON object on stdin."), {
      status: 400,
      code: "udio_browser_invalid_input",
    });
  }
  if (!normalizeString(input.baseUrl)) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "udio_browser_missing_base_url",
    });
  }
  if (!input.requestBody || typeof input.requestBody !== "object" || Array.isArray(input.requestBody)) {
    throw Object.assign(new Error("requestBody must be a JSON object."), {
      status: 400,
      code: "udio_browser_missing_request_body",
    });
  }
  normalizeTargetAssetKind(input.targetAssetKind);
  const browserCdpUrl = normalizeString(
    input.browserCdpUrl ?? process.env.UDIO_BROWSER_CDP_URL ?? null,
  );
  if (
    !normalizeString(input.cookieHeader) &&
    !normalizeString(input.runtimeStateObjectKey) &&
    !browserCdpUrl
  ) {
    throw Object.assign(
      new Error("Udio browser worker requires cookieHeader, runtimeStateObjectKey, or browserCdpUrl."),
      {
        status: 400,
        code: "udio_browser_missing_runtime_auth",
      },
    );
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

function normalizeError(error) {
  const status =
    typeof error?.status === "number" && Number.isFinite(error.status) ? error.status : 500;
  return {
    status,
    code:
      typeof error?.code === "string" && error.code.trim()
        ? error.code.trim()
        : "udio_browser_worker_failed",
    message:
      typeof error?.message === "string" && error.message.trim()
        ? error.message.trim()
        : String(error),
    body:
      typeof error?.body === "string" && error.body.trim()
        ? trimBody(error.body)
        : undefined,
  };
}

function trimBody(value) {
  const text = String(value ?? "").trim();
  if (!text) {
    return "";
  }
  return text.slice(0, MAX_ERROR_BODY_LENGTH);
}

function deepClone(value) {
  return JSON.parse(JSON.stringify(value ?? {}));
}

async function gotoUdio(page, url, timeoutMs) {
  await page.goto(url, {
    waitUntil: "domcontentloaded",
    timeout: timeoutMs,
  });
  await page.waitForLoadState("networkidle", {
    timeout: Math.min(timeoutMs, 10_000),
  }).catch(() => undefined);
}

async function openChallengeSurface(page, baseUrl, referer, timeoutMs, options = {}) {
  if (options.allowNavigation === false) {
    return;
  }
  const targetUrl = page.url()?.startsWith(baseUrl) ? page.url() : referer;
  try {
    await gotoUdio(page, targetUrl || referer || `${baseUrl}/`, timeoutMs);
  } catch {
    await gotoUdio(page, `${baseUrl}/`, timeoutMs).catch(() => undefined);
  }
}

async function resolveVercelCheckpointInBorrowedPage(page, options) {
  const { baseUrl, referer, timeoutMs } = options;
  const checkpointUrl = `${baseUrl}/api/users/current`;
  const response = await page.goto(checkpointUrl, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, DEFAULT_NAVIGATION_TIMEOUT_MS),
  });
  if (!response) {
    throw Object.assign(new Error("Udio security checkpoint did not return a response."), {
      status: 502,
      code: "udio_checkpoint_navigation_failed",
    });
  }

  const initialOutcome = {
    status: response.status(),
    contentType: response.headers()["content-type"] ?? "",
    mitigated: response.headers()["x-vercel-mitigated"] ?? "",
    text: await response.text().catch(() => ""),
  };
  if (!response.ok() && !isChallengeOutcome(initialOutcome)) {
    requireJsonResponse(
      initialOutcome,
      "udio_checkpoint_invalid_json",
      "Udio security checkpoint probe returned invalid JSON.",
    );
  }

  const deadline = Date.now() + timeoutMs;
  while (!response.ok() && Date.now() < deadline) {
    const probe = await page.evaluate(async (requestUrl) => {
      try {
        const result = await fetch(requestUrl, { credentials: "include" });
        return {
          ok: result.ok,
          status: result.status,
          mitigated: result.headers.get("x-vercel-mitigated") || "",
        };
      } catch {
        return { ok: false, status: 0, mitigated: "" };
      }
    }, checkpointUrl);
    if (probe.ok && !probe.mitigated) {
      await gotoUdio(page, referer || `${baseUrl}/create`, DEFAULT_NAVIGATION_TIMEOUT_MS);
      return;
    }
    await page.waitForTimeout(Math.min(1_000, Math.max(100, deadline - Date.now())));
  }

  if (response.ok()) {
    await gotoUdio(page, referer || `${baseUrl}/create`, DEFAULT_NAVIGATION_TIMEOUT_MS);
    return;
  }
  throw Object.assign(
    new Error("Udio requires the visible Vercel Security Checkpoint to be completed."),
    {
      status: 429,
      code: "udio_browser_challenge_required",
    },
  );
}

async function submitGenerationThroughPageUi(page, options) {
  const { baseUrl, requestBody, timeoutMs } = options;
  const prompt = normalizeString(requestBody?.gen_params?.prompt ?? requestBody?.prompt);
  if (!prompt) {
    throw Object.assign(new Error("Udio native generation requires a non-empty prompt."), {
      status: 400,
      code: "udio_browser_missing_prompt",
    });
  }

  await page.evaluate(() => {
    for (const id of [
      "__udio-hcaptcha-manual-wrapper",
      "__udio-hcaptcha-manual-container",
      "__udio-hcaptcha-container",
    ]) {
      document.getElementById(id)?.remove();
    }
  });

  const promptInput = page.locator("textarea").first();
  await promptInput.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 30_000) });
  await promptInput.fill(prompt);

  const createButton = await waitForNativeCreateButton(page, Math.min(timeoutMs, 60_000));

  const responsePromise = page
    .waitForResponse(
      (response) => {
        try {
          const requestUrl = new URL(response.url());
          return (
            requestUrl.origin === baseUrl &&
            normalizePathname(requestUrl.pathname) === "/api/generate-proxy" &&
            response.request().method() === "POST"
          );
        } catch {
          return false;
        }
      },
      { timeout: timeoutMs },
    )
    .then(
      (response) => ({ response, error: null }),
      (error) => ({ response: null, error }),
    );
  await createButton.click();

  try {
    const deadline = Date.now() + timeoutMs;
    let sawCreateUnavailable = false;
    let challengeCycleRetries = 0;
    let responseResult = null;
    while (Date.now() < deadline) {
      responseResult = await Promise.race([
        responsePromise,
        page.waitForTimeout(Math.min(250, Math.max(50, deadline - Date.now()))).then(() => null),
      ]);
      if (responseResult) {
        break;
      }

      const availableCreateButton = await findNativeCreateButton(page);
      if (!availableCreateButton) {
        sawCreateUnavailable = true;
        continue;
      }
      if (sawCreateUnavailable && challengeCycleRetries < 3) {
        sawCreateUnavailable = false;
        challengeCycleRetries += 1;
        await availableCreateButton.click();
      }
    }
    responseResult ??= await responsePromise;
    if (responseResult.error) {
      throw responseResult.error;
    }
    const response = responseResult.response;
    const headers = response.headers();
    const text = await response.text().catch(() => "");
    return {
      ok: response.ok(),
      status: response.status(),
      contentType: headers["content-type"] ?? "",
      mitigated: headers["x-vercel-mitigated"] ?? "",
      text,
    };
  } catch (error) {
    return {
      ok: false,
      status: 504,
      transportError: true,
      text: typeof error?.message === "string" ? error.message : String(error),
    };
  }
}

async function waitForNativeCreateButton(page, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const createButton = await findNativeCreateButton(page);
    if (createButton) {
      return createButton;
    }
    await page.waitForTimeout(Math.min(500, Math.max(50, deadline - Date.now())));
  }
  throw Object.assign(new Error("Udio native Create button did not become available."), {
    status: 409,
    code: "udio_browser_create_unavailable",
  });
}

async function findNativeCreateButton(page) {
  const createButtons = page.getByRole("button", { name: "Create", exact: true });
  for (let index = 0; index < (await createButtons.count()); index += 1) {
    const candidate = createButtons.nth(index);
    if ((await candidate.isVisible()) && (await candidate.isEnabled())) {
      return candidate;
    }
  }
  return null;
}

async function browserFetch(page, args) {
  return await page.evaluate(
    async ({ requestUrl, method, headers, bodyText, timeoutMs }) => {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
      try {
        const response = await fetch(requestUrl, {
          method,
          headers,
          body: typeof bodyText === "string" ? bodyText : undefined,
          credentials: "include",
          signal: controller.signal,
        });
        const text = await response.text().catch(() => "");
        return {
          ok: response.ok,
          status: response.status,
          contentType: response.headers.get("content-type") || "",
          mitigated: response.headers.get("x-vercel-mitigated") || "",
          text,
        };
      } catch (error) {
        return {
          ok: false,
          status: 504,
          transportError: true,
          text: typeof error?.message === "string" ? error.message : String(error),
        };
      } finally {
        clearTimeout(timeoutId);
      }
    },
    {
      requestUrl: args.requestUrl,
      method: args.method,
      headers: args.headers ?? {},
      bodyText: args.bodyText ?? null,
      timeoutMs: args.timeoutMs,
    },
  );
}

async function runWithChallengeRetries(args) {
  let challengeAttempted = false;
  let lastChallenge = null;
  while (Date.now() < args.overallDeadline) {
    const outcome = await args.action();
    if (outcome.transportError) {
      throw Object.assign(new Error(`Browser request failed: ${outcome.text}`), {
        status: outcome.status ?? 504,
        code: "udio_browser_transport_failed",
        body: outcome.text,
      });
    }
    if (!isChallengeOutcome(outcome)) {
      return outcome;
    }

    lastChallenge = outcome;
    if (!challengeAttempted) {
      challengeAttempted = true;
      await args.onChallenge();
    }

    const remainingMs = args.overallDeadline - Date.now();
    if (remainingMs <= 0) {
      break;
    }
    await new Promise((resolve) =>
      setTimeout(resolve, Math.min(args.retryIntervalMs, remainingMs)),
    );
  }

  throw Object.assign(
    new Error(
      "Udio requires a browser security check or captcha challenge before generation can continue.",
    ),
    {
      status: lastChallenge?.status ?? 429,
      code: "udio_browser_challenge_required",
      body: trimBody(lastChallenge?.text ?? ""),
    },
  );
}

function isChallengeOutcome(outcome) {
  const contentType = String(outcome.contentType ?? "").toLowerCase();
  const mitigated = String(outcome.mitigated ?? "").toLowerCase();
  const lower = String(outcome.text ?? "").toLowerCase();
  const userDisallowed = lower.includes("user disallowed");
  return (
    (Number(outcome.status ?? 0) === 403 && userDisallowed) ||
    ([403, 429].includes(Number(outcome.status ?? 0)) &&
      (mitigated.includes("challenge") ||
        contentType.includes("text/html") ||
        lower.includes("<!doctype html") ||
        lower.includes("<html")) &&
      (lower.includes("vercel security checkpoint") ||
        lower.includes("x-vercel-challenge-token") ||
        lower.includes("x-vercel-mitigated") ||
        lower.includes("security checkpoint") ||
        lower.includes("data-astro-cid")))
  );
}

function requireJsonResponse(outcome, invalidJsonCode, invalidJsonMessage) {
  if (outcome.ok) {
    try {
      return JSON.parse(outcome.text);
    } catch (error) {
      throw Object.assign(new Error(`${invalidJsonMessage} ${error}`), {
        status: 502,
        code: invalidJsonCode,
        body: trimBody(outcome.text),
      });
    }
  }

  if (Number(outcome.status) === 401) {
    throw Object.assign(new Error("Udio browser session is not authenticated."), {
      status: 401,
      code: "udio_session_unauthorized",
      body: trimBody(outcome.text),
    });
  }

  const lower = String(outcome.text ?? "").toLowerCase();
  if (
    [400, 403, 422].includes(Number(outcome.status)) &&
    (lower.includes("captcha") || lower.includes("hcaptcha"))
  ) {
    throw Object.assign(new Error("Udio captcha verification failed."), {
      status: Number(outcome.status) || 400,
      code: "udio_captcha_verification_failed",
      body: trimBody(outcome.text),
    });
  }

  if (Number(outcome.status) === 403 && lower.includes("user disallowed")) {
    throw Object.assign(
      new Error(
        "Udio rejected the current challenge token or browser clearance. Refresh the challenge in the same browser context and retry.",
      ),
      {
        status: 403,
        code: "udio_browser_challenge_required",
        body: trimBody(outcome.text),
      },
    );
  }

  throw Object.assign(new Error(`Udio request failed with HTTP ${outcome.status}.`), {
    status: Number(outcome.status) || 500,
    code: "udio_browser_request_failed",
    body: trimBody(outcome.text),
  });
}

function extractTrackIds(body) {
  const direct = Array.isArray(body?.track_ids)
    ? body.track_ids
    : Array.isArray(body?.trackIds)
      ? body.trackIds
      : null;
  if (direct?.length) {
    const ids = direct
      .map((value) => (typeof value === "string" ? value.trim() : ""))
      .filter(Boolean);
    if (ids.length) {
      return ids;
    }
  }

  const songs = readSongs(body);
  const songIds = songs
    .map((song) => (typeof song?.id === "string" ? song.id.trim() : ""))
    .filter(Boolean);
  if (songIds.length) {
    return songIds;
  }

  throw Object.assign(new Error("Udio generation response missing track ids."), {
    status: 502,
    code: "udio_missing_track_ids",
    body: trimBody(JSON.stringify(body)),
  });
}

function readSongs(body) {
  const songs = Array.isArray(body?.songs) ? body.songs : [];
  return songs.filter((song) => song && typeof song === "object" && !Array.isArray(song));
}

function songsReadyForTarget(songs, targetAssetKind) {
  return (
    Array.isArray(songs) &&
    songs.length > 0 &&
    songs.every((song) => songHasTargetAsset(song, targetAssetKind))
  );
}

function songHasTargetAsset(song, targetAssetKind) {
  if (!song || typeof song !== "object") {
    return false;
  }
  if (targetAssetKind === "image") {
    return hasNonEmptyString(
      song?.image_url ??
        song?.imageUrl ??
        song?.image_path ??
        song?.imagePath ??
        song?.cover_image_url ??
        song?.coverImageUrl ??
        song?.cover_art_url ??
        song?.coverArtUrl,
    );
  }
  if (targetAssetKind === "video") {
    return hasNonEmptyString(song?.video_url ?? song?.videoUrl ?? song?.video_path ?? song?.videoPath);
  }
  return hasNonEmptyString(song?.song_path ?? song?.songPath ?? song?.audio_url ?? song?.audioUrl);
}

function hasNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function timeoutMessageForTarget(targetAssetKind) {
  switch (targetAssetKind) {
    case "image":
      return "Timed out waiting for Udio cover art to reach a terminal state.";
    case "video":
      return "Timed out waiting for Udio video to reach a terminal state.";
    default:
      return "Timed out waiting for Udio audio to reach a terminal state.";
  }
}

async function obtainHCaptchaToken(page, sitekey, apiSrc, manualChallengeWaitMs = 0) {
  try {
    return await page.evaluate(async ({ sitekey, apiSrc, manualChallengeWaitMs }) => {
    const scriptId = "__udio-hcaptcha-script";
    const containerId = "__udio-hcaptcha-container";
    const manualContainerId = "__udio-hcaptcha-manual-container";
    const manualWrapperId = "__udio-hcaptcha-manual-wrapper";
    const executeTimeoutMs = 15_000;

    const waitForHCaptcha = async () => {
      if (window.hcaptcha) {
        return window.hcaptcha;
      }

      await new Promise((resolve, reject) => {
        const existing = document.getElementById(scriptId);
        if (existing) {
          const startedAt = Date.now();
          const poll = () => {
            if (window.hcaptcha) {
              resolve();
              return;
            }
            if (Date.now() - startedAt > 20_000) {
              reject(new Error("Timed out waiting for hCaptcha to load."));
              return;
            }
            setTimeout(poll, 100);
          };
          poll();
          return;
        }

        const script = document.createElement("script");
        script.id = scriptId;
        script.async = true;
        script.src = apiSrc;
        script.onload = () => resolve();
        script.onerror = () => reject(new Error("Failed to load hCaptcha API."));
        document.head.appendChild(script);
      });

      if (!window.hcaptcha) {
        throw new Error("hCaptcha API did not become available.");
      }
      return window.hcaptcha;
    };

    const ensureContainer = (id, styleText, textContent = "") => {
      let node = document.getElementById(id);
      if (!node) {
        node = document.createElement("div");
        node.id = id;
        if (textContent) {
          node.textContent = textContent;
        }
        document.body.appendChild(node);
      }
      node.style.cssText = styleText;
      return node;
    };

    const withTimeout = async (promise, timeoutMs, label) => {
      let timer = null;
      try {
        return await Promise.race([
          promise,
          new Promise((_, reject) => {
            timer = setTimeout(() => {
              reject(new Error(`${label} timed out after ${timeoutMs}ms.`));
            }, timeoutMs);
          }),
        ]);
      } finally {
        if (timer) {
          clearTimeout(timer);
        }
      }
    };

    const readVisibleChallengeFrame = () => {
      const frames = Array.from(document.querySelectorAll('iframe[src*="hcaptcha"]'));
      for (const frame of frames) {
        if (!(frame instanceof HTMLIFrameElement)) {
          continue;
        }
        const rect = frame.getBoundingClientRect();
        const style = window.getComputedStyle(frame);
        if (
          rect.width >= 120 &&
          rect.height >= 120 &&
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          style.opacity !== "0"
        ) {
          return {
            title: frame.getAttribute("title") || null,
            src: frame.getAttribute("src") || null,
            width: Math.round(rect.width),
            height: Math.round(rect.height),
          };
        }
      }
      return null;
    };

    const throwVisibleChallengeRequired = (frame) => {
      const title = typeof frame?.title === "string" && frame.title.trim() ? frame.title.trim() : "visible hCaptcha challenge";
      throw new Error(`challenge required: ${title}`);
    };

    const waitForSolvedToken = async (hcaptcha, widgetId, timeoutMs) => {
      if (!timeoutMs || timeoutMs <= 0) {
        return null;
      }
      const deadline = Date.now() + timeoutMs;
      while (Date.now() < deadline) {
        const token = readWidgetToken(hcaptcha, widgetId);
        if (token) {
          return token;
        }
        await new Promise((resolve) => setTimeout(resolve, 250));
      }
      return null;
    };

    const validWidgetId = (widgetId) =>
      typeof widgetId === "number" ||
      (typeof widgetId === "string" && Boolean(widgetId.trim()));

    const readWidgetToken = (hcaptcha, widgetId) => {
      const callbackToken = window.__udioManualHcaptchaResolvedToken;
      if (typeof callbackToken === "string" && callbackToken.trim()) {
        return callbackToken.trim();
      }
      if (!validWidgetId(widgetId) || typeof hcaptcha?.getResponse !== "function") {
        return null;
      }
      try {
        const token = hcaptcha.getResponse(widgetId);
        return typeof token === "string" && token.trim() ? token.trim() : null;
      } catch {
        return null;
      }
    };

    const extractExecuteToken = (execution, hcaptcha, widgetId) => {
      const direct =
        (typeof execution === "object" && typeof execution?.response === "string"
          ? execution.response
          : typeof execution === "string"
            ? execution
            : null) || readWidgetToken(hcaptcha, widgetId);
      return typeof direct === "string" && direct.trim() ? direct.trim() : null;
    };

    const hcaptcha = await waitForHCaptcha();
    const removeWidget = (widgetId, container) => {
      if (validWidgetId(widgetId) && typeof hcaptcha.remove === "function") {
        try {
          hcaptcha.remove(widgetId);
        } catch {
          // Ignore stale widget ids.
        }
      }
      container?.replaceChildren();
    };

    if (manualChallengeWaitMs > 0) {
      ensureContainer(
        manualWrapperId,
        [
          "position:fixed",
          "right:24px",
          "bottom:24px",
          "z-index:2147483647",
          "max-width:360px",
          "padding:12px 14px",
          "border-radius:12px",
          "background:rgba(16,18,24,0.94)",
          "color:#fff",
          "font:500 14px/1.45 Inter, Arial, sans-serif",
          "box-shadow:0 20px 60px rgba(0,0,0,0.45)",
        ].join(";"),
        "Complete the captcha in this window to continue the Udio generation request.",
      );

      const manualContainer = ensureContainer(
        manualContainerId,
        [
          "position:fixed",
          "right:24px",
          "bottom:96px",
          "z-index:2147483647",
          "min-width:304px",
          "min-height:78px",
          "padding:4px",
          "border-radius:12px",
          "background:rgba(255,255,255,0.98)",
          "box-shadow:0 20px 60px rgba(0,0,0,0.35)",
        ].join(";"),
      );
      removeWidget(window.__udioManualHcaptchaWidgetId, manualContainer);
      window.__udioManualHcaptchaResolvedToken = null;
      const manualWidgetId = hcaptcha.render(manualContainer, {
        sitekey,
        size: "normal",
        theme: "light",
        callback: (token) => {
          window.__udioManualHcaptchaResolvedToken =
            typeof token === "string" && token.trim() ? token.trim() : null;
        },
        "expired-callback": () => {
          window.__udioManualHcaptchaResolvedToken = null;
        },
        "error-callback": () => {
          window.__udioManualHcaptchaResolvedToken = null;
        },
      });
      window.__udioManualHcaptchaWidgetId = manualWidgetId;

      const manualToken = await waitForSolvedToken(hcaptcha, manualWidgetId, manualChallengeWaitMs);
      if (manualToken) {
        window.__udioManualHcaptchaWidgetId = null;
        window.__udioManualHcaptchaResolvedToken = null;
        removeWidget(manualWidgetId, manualContainer);
        return manualToken;
      }
      throwVisibleChallengeRequired(
        readVisibleChallengeFrame() ?? { title: "hCaptcha challenge was not completed" },
      );
    }

    const container = ensureContainer(
      containerId,
      "position:fixed;left:-9999px;top:-9999px;width:1px;height:1px;opacity:0;pointer-events:none;",
    );

    removeWidget(window.__udioHcaptchaWidgetId, container);
    const widgetId = hcaptcha.render(container, {
      sitekey,
      size: "invisible",
      theme: "dark",
    });
    window.__udioHcaptchaWidgetId = widgetId;

    const execution = await withTimeout(
      hcaptcha.execute(widgetId, { async: true }),
      executeTimeoutMs,
      "hCaptcha helper execute",
    );
    const token =
      (typeof execution === "object" && typeof execution?.response === "string"
        ? execution.response
        : typeof execution === "string"
          ? execution
          : null) || readWidgetToken(hcaptcha, widgetId);

    if (!token || typeof token !== "string") {
      const visibleChallengeAfterHelperExecute = readVisibleChallengeFrame();
      if (visibleChallengeAfterHelperExecute) {
        throwVisibleChallengeRequired(visibleChallengeAfterHelperExecute);
      }
      throw new Error("hCaptcha execute completed without returning a token.");
    }

    window.__udioHcaptchaWidgetId = null;
    removeWidget(widgetId, container);
    return token;
    }, { sitekey, apiSrc, manualChallengeWaitMs });
  } catch (error) {
    const message = typeof error?.message === "string" ? error.message : String(error);
    const lower = message.toLowerCase();
    if (
      lower.includes("challenge-expired") ||
      lower.includes("challenge-closed") ||
      lower.includes("challenge required") ||
      (lower.includes("hcaptcha") && lower.includes("timed out"))
    ) {
      throw Object.assign(
        new Error(
          "Udio requires a browser security check or captcha challenge before generation can continue.",
        ),
        {
          status: 429,
          code: "udio_browser_challenge_required",
          body: trimBody(message),
        },
      );
    }
    throw error;
  }
}

async function resolveUdioAccessToken(page, baseUrl) {
  const context = page.context();
  let cookies = await context.cookies([`${baseUrl}/`]).catch(() => []);
  // Some CDP-backed Chromium contexts return an empty URL-filtered cookie set
  // even though the provider cookies are present. Fall back to the complete
  // context cookie jar and let the auth-cookie prefix filter narrow it.
  if (!cookies.length) {
    cookies = await context.cookies().catch(() => []);
  }
  if (!cookies.length) {
    const session = await context.newCDPSession(page).catch(() => null);
    if (session) {
      try {
        const result = await session.send("Network.getAllCookies");
        cookies = Array.isArray(result?.cookies) ? result.cookies : [];
      } finally {
        await session.detach().catch(() => undefined);
      }
    }
  }
  const tokenFromCookie = extractSupabaseAccessTokenFromCookies(cookies);
  if (tokenFromCookie) {
    return tokenFromCookie;
  }

  return await page.evaluate(() => {
    try {
      for (let index = 0; index < localStorage.length; index += 1) {
        const key = localStorage.key(index);
        if (!key) continue;
        const lower = key.toLowerCase();
        if (!lower.includes("supabase") && !lower.includes("auth")) {
          continue;
        }
        const raw = localStorage.getItem(key);
        if (!raw) continue;
        try {
          const parsed = JSON.parse(raw);
          const token =
            parsed?.access_token ??
            parsed?.currentSession?.access_token ??
            parsed?.session?.access_token ??
            parsed?.data?.session?.access_token;
          if (typeof token === "string" && token.trim()) {
            return token.trim();
          }
        } catch {
          // Ignore non-JSON auth cache entries.
        }
      }
    } catch {
      // Ignore localStorage access failures.
    }
    return null;
  });
}

function extractSupabaseAccessTokenFromCookies(cookies) {
  const prefix = "sb-ssr-production-auth-token";
  const parts = cookies
    .filter((cookie) => typeof cookie?.name === "string" && cookie.name.startsWith(prefix))
    .map((cookie) => ({
      index: cookie.name === prefix ? 0 : parseCookiePartIndex(cookie.name),
      value: String(cookie.value ?? ""),
    }))
    .filter((entry) => Number.isInteger(entry.index))
    .sort((left, right) => left.index - right.index);

  if (!parts.length) {
    return null;
  }

  const joined = parts.map((entry) => entry.value).join("");
  return extractAccessTokenFromSupabaseCookie(joined);
}

function parseCookiePartIndex(name) {
  const suffix = name.slice(name.lastIndexOf(".") + 1);
  const parsed = Number.parseInt(suffix, 10);
  return Number.isFinite(parsed) ? parsed : Number.NaN;
}

function extractAccessTokenFromSupabaseCookie(rawValue) {
  const normalized = normalizeString(rawValue);
  if (!normalized) {
    return null;
  }

  let payload = normalized;
  if (payload.startsWith("base64-")) {
    payload = payload.slice("base64-".length);
    payload = Buffer.from(normalizeBase64(payload), "base64").toString("utf8");
  }

  try {
    const parsed = JSON.parse(payload);
    const token =
      parsed?.access_token ??
      parsed?.accessToken ??
      parsed?.currentSession?.access_token ??
      parsed?.session?.access_token;
    return typeof token === "string" && token.trim() ? token.trim() : null;
  } catch {
    return null;
  }
}

function normalizeBase64(value) {
  const normalized = String(value ?? "").replace(/-/g, "+").replace(/_/g, "/");
  const remainder = normalized.length % 4;
  if (remainder === 0) {
    return normalized;
  }
  return normalized.padEnd(normalized.length + (4 - remainder), "=");
}

function resolveObjectStorageConfig() {
  const driver =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER) ??
    normalizeString(process.env.OBJECT_STORAGE_DRIVER) ??
    "local";
  const localDir =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.OBJECT_STORAGE_LOCAL_DIR) ??
    ".runtime/ai-gateway-objects";

  return {
    driver,
    localDir,
    bucket:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_BUCKET) ??
      normalizeString(process.env.OBJECT_STORAGE_BUCKET),
    region:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_REGION) ??
      normalizeString(process.env.OBJECT_STORAGE_REGION) ??
      "auto",
    endpoint:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ENDPOINT) ??
      normalizeString(process.env.OBJECT_STORAGE_ENDPOINT),
    accessKeyId:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID) ??
      normalizeString(process.env.OBJECT_STORAGE_ACCESS_KEY_ID),
    secretAccessKey:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY) ??
      normalizeString(process.env.OBJECT_STORAGE_SECRET_ACCESS_KEY),
    forcePathStyle: ["1", "true", "yes", "on"].includes(
      (process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
        process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
        "")
        .trim()
        .toLowerCase(),
    ),
  };
}

function getStorageRoot() {
  const config = resolveObjectStorageConfig();
  return path.resolve(process.cwd(), config.localDir);
}

async function toBuffer(stream) {
  if (!stream) return Buffer.alloc(0);
  if (Buffer.isBuffer(stream)) return stream;
  if (typeof stream === "object" && typeof stream.transformToByteArray === "function") {
    return Buffer.from(await stream.transformToByteArray());
  }
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("Udio worker object storage is not fully configured.");
  }
  objectStorageClient = new S3Client({
    region: config.region,
    endpoint: config.endpoint,
    forcePathStyle: config.forcePathStyle,
    credentials: {
      accessKeyId: config.accessKeyId,
      secretAccessKey: config.secretAccessKey,
    },
  });
  return objectStorageClient;
}

async function maybeReadRuntimeState(objectKey) {
  const normalized = normalizeString(objectKey);
  if (!normalized) {
    return null;
  }

  try {
    const config = resolveObjectStorageConfig();
    let buffer;
    if (config.driver === "local") {
      const absolutePath = path.join(getStorageRoot(), ...normalized.split("/"));
      buffer = await readFile(absolutePath);
    } else {
      const client = getS3Client(config);
      const response = await client.send(
        new GetObjectCommand({
          Bucket: config.bucket,
          Key: normalized,
        }),
      );
      buffer = await toBuffer(response.Body);
    }

    if (!buffer?.length) {
      return null;
    }
    return JSON.parse(buffer.toString("utf8"));
  } catch {
    return null;
  }
}

async function maybePersistRuntimeState(context, objectKey, existingState = null) {
  const normalized = normalizeString(objectKey);
  if (!normalized) {
    return;
  }

  const state = await context.storageState();
  if (!hasPersistableRuntimeState(state) && !hasPersistableRuntimeState(existingState)) {
    return;
  }
  if (!shouldPersistRuntimeState(existingState, state)) {
    return;
  }
  const buffer = Buffer.from(JSON.stringify(state, null, 2), "utf8");
  const config = resolveObjectStorageConfig();

  if (config.driver === "local") {
    const absolutePath = path.join(getStorageRoot(), ...normalized.split("/"));
    await mkdir(path.dirname(absolutePath), { recursive: true });
    await writeFile(absolutePath, buffer);
    return;
  }

  const client = getS3Client(config);
  await client.send(
    new PutObjectCommand({
      Bucket: config.bucket,
      Key: normalized,
      Body: buffer,
      ContentType: "application/json",
    }),
  );
}

function shouldPersistRuntimeState(existingState, nextState) {
  if (hasPersistableRuntimeState(nextState)) {
    return true;
  }
  if (hasPersistableRuntimeState(existingState)) {
    return false;
  }
  return true;
}

function hasPersistableRuntimeState(state) {
  return hasUdioAuthCookies(state) || hasUdioAuthLocalStorage(state);
}

function hasUdioAuthCookies(state) {
  const cookies = Array.isArray(state?.cookies) ? state.cookies : [];
  return cookies.some(
    (cookie) =>
      typeof cookie?.name === "string" &&
      cookie.name.startsWith("sb-ssr-production-auth-token"),
  );
}

function hasUdioAuthLocalStorage(state) {
  const origins = Array.isArray(state?.origins) ? state.origins : [];
  for (const origin of origins) {
    const entries = Array.isArray(origin?.localStorage) ? origin.localStorage : [];
    for (const entry of entries) {
      const key = String(entry?.name ?? "");
      const raw = String(entry?.value ?? "");
      if (!key || !raw) {
        continue;
      }
      const lower = key.toLowerCase();
      if (!lower.includes("supabase") && !lower.includes("auth")) {
        continue;
      }
      try {
        const parsed = JSON.parse(raw);
        const token =
          parsed?.access_token ??
          parsed?.currentSession?.access_token ??
          parsed?.session?.access_token ??
          parsed?.data?.session?.access_token;
        if (typeof token === "string" && token.trim()) {
          return true;
        }
      } catch {
        // Ignore non-JSON cache entries.
      }
    }
  }
  return false;
}

function parseCookieHeader(rawHeader, baseUrl) {
  if (!rawHeader) {
    return [];
  }
  const cookies = [];
  for (const segment of rawHeader.split(";")) {
    const raw = segment.trim();
    if (!raw) {
      continue;
    }
    const separator = raw.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    const name = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim();
    if (!name || !value) {
      continue;
    }
    cookies.push({
      name,
      value,
      url: `${baseUrl}/`,
      secure: true,
      sameSite: "Lax",
      httpOnly: name.startsWith("sb-"),
    });
  }
  return cookies;
}

main();
