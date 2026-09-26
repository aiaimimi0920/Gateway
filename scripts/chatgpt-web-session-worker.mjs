import { chromium } from "playwright-core";
import { randomUUID } from "node:crypto";
import { rm } from "node:fs/promises";
import {
  DEFAULT_BASE_URL,
  normalizeBaseUrl,
  dedupeStrings,
  normalizeString,
  normalizeTimeoutMs,
  parseBoolean,
  resolveHeadlessMode,
  resolveProxySettings,
  normalizeAuthSeed,
} from "./chatgpt-web-session/configuration.mjs";
import { createWorkerError, serializeError } from "./chatgpt-web-session/errors.mjs";
import { maybeWriteCredentialFile } from "./chatgpt-web-session/credentials.mjs";
import { sleep } from "./chatgpt-web-session/timing.mjs";
import { resolveMailboxContext } from "./chatgpt-web-session/mailbox-context.mjs";
import { loginExistingAccount } from "./chatgpt-web-session/account-login.mjs";
import { bootstrapChatGptOauthSession } from "./chatgpt-web-session/auth-navigation.mjs";
import { safePageUrl } from "./chatgpt-web-session/page-state.mjs";
import {
  primeBrowserWithImportedCookies,
  collectCookieHeader,
  findCookieValue,
  decodeJwtExpIso,
} from "./chatgpt-web-session/cookies.mjs";
import {
  resolveExecutablePath,
  resolveProfileSource,
  cloneBrowserProfile,
  createFreshBrowserProfile,
} from "./chatgpt-web-session/profile.mjs";
import { runChatGptBrowserProbe, extractBootstrapArtifacts } from "./chatgpt-web-session/browser-probe.mjs";
import { runChatGptBrowserRelay } from "./chatgpt-web-session/http-relay.mjs";
import { DEFAULT_POW_SCRIPT } from "./chatgpt-web-session/sentinel-pow.mjs";
import { navigateWithChallengeSettle, detectBrowserChallenge } from "./chatgpt-web-session/navigation.mjs";

// This worker only refreshes or re-materializes an existing ChatGPT Web session.
// It may use an existing browser profile or best-effort browser login seed, but
// it does not register accounts, solve mailbox/OTP flows, or own EasyProtocol's
// registration pipeline.

const DEFAULT_ACCEPT_LANGUAGE = "zh-CN,zh;q=0.9,en;q=0.8,en-US;q=0.7";
const DEFAULT_LANGUAGE_CODE = "zh-CN";
const DEFAULT_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";
const DEFAULT_CLIENT_VERSION = "prod-be885abbfcfe7b1f511e88b3003d9ee44757fbad";
const DEFAULT_CLIENT_BUILD_NUMBER = "5955942";
const DEFAULT_TIMEZONE = "Asia/Shanghai";
const DEFAULT_MODELS_PATH = "/backend-api/models?history_and_training_disabled=false";
async function main() {
  let browser = null;
  let context = null;
  let clonedUserDataDir = null;
  let killTimer = null;
  let currentStage = "init";

  try {
    const input = JSON.parse(await readStdin());
    const requestedTimeoutMs = normalizeTimeoutMs(input?.timeoutMs);
    const hardTimeoutMs = Math.min(requestedTimeoutMs + 45_000, 10 * 60 * 1000);
    killTimer = setTimeout(() => {
      printAndExit(
        {
          ok: false,
          error: serializeError(
            createWorkerError(
              504,
              "chatgpt_web_browser_refresh_timeout",
              `ChatGPT Web browser refresh exceeded the worker deadline (${hardTimeoutMs} ms) at stage=${currentStage}.`,
            ),
          ),
        },
        1,
      );
    }, hardTimeoutMs);
    const authSeed = normalizeAuthSeed(input?.authSeed);
    const mailboxContext = resolveMailboxContext(input, authSeed);
    const baseUrl = normalizeBaseUrl(input?.baseUrl ?? DEFAULT_BASE_URL);
    const modelsPath = normalizePath(input?.modelsPath) ?? DEFAULT_MODELS_PATH;
    const authUrl = normalizeString(input?.authUrl) ?? null;
    const timeoutMs = requestedTimeoutMs;
    const executablePath = resolveExecutablePath(
      input?.browserExecutablePath ??
        process.env.CHATGPT_WEB_BROWSER_EXECUTABLE_PATH ??
        process.env.PRODUCER_BROWSER_EXECUTABLE_PATH ??
        null,
    );
    const proxySettings = resolveProxySettings(input);
    if (!executablePath) {
      throw createWorkerError(
        500,
        "chatgpt_web_browser_not_found",
        "Unable to locate a Chromium-compatible browser for the ChatGPT Web session worker.",
      );
    }

    const accessToken =
      normalizeString(input?.authToken) ??
      normalizeString(input?.apiKey) ??
      normalizeString(input?.accessToken) ??
      null;
    currentStage = "resolve_profile";
    const profileSource = resolveProfileSource(input, authSeed);
    if (profileSource.mode === "clone") {
      currentStage = "clone_browser_profile";
      clonedUserDataDir = await cloneBrowserProfile(
        profileSource.userDataDir,
        profileSource.profileDirectory,
      );
    } else {
      currentStage = "create_fresh_profile";
      clonedUserDataDir = await createFreshBrowserProfile(profileSource.profileDirectory);
    }

    currentStage = "launch_browser";
    context = await chromium.launchPersistentContext(clonedUserDataDir, {
      executablePath,
      headless: resolveHeadlessMode(input),
      proxy: proxySettings?.launchOptions,
      locale:
        normalizeString(input?.languageCode) ??
        normalizeString(input?.language) ??
        DEFAULT_LANGUAGE_CODE,
      timezoneId: normalizeString(input?.timezone) ?? DEFAULT_TIMEZONE,
      userAgent: normalizeString(input?.userAgent) ?? DEFAULT_USER_AGENT,
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--disable-features=OptimizationGuideModelDownloading,Translate",
        "--no-first-run",
        "--no-default-browser-check",
        `--profile-directory=${profileSource.profileDirectory}`,
      ],
    });

    browser = context.browser();
    const page = context.pages()[0] ?? (await context.newPage());
    await page.setExtraHTTPHeaders({
      "Accept-Language":
        normalizeString(input?.acceptLanguage) ??
        normalizeString(input?.language) ??
        DEFAULT_ACCEPT_LANGUAGE,
    });

    currentStage = "prime_imported_cookies";
    await primeBrowserWithImportedCookies(context, baseUrl, input);
    currentStage = "navigate_home";
    await navigateWithChallengeSettle(page, `${baseUrl}/`, timeoutMs);

    currentStage = "probe_initial";
    let probe = await runChatGptBrowserProbe(page, {
      baseUrl,
      modelsPath,
      accessToken,
      languageCode:
        normalizeString(input?.languageCode) ??
        normalizeString(input?.oaiLanguage) ??
        DEFAULT_LANGUAGE_CODE,
      clientVersion:
        normalizeString(input?.clientVersion) ?? DEFAULT_CLIENT_VERSION,
      clientBuildNumber:
        normalizeString(input?.clientBuildNumber) ?? DEFAULT_CLIENT_BUILD_NUMBER,
      deviceId:
        normalizeString(input?.deviceId) ??
        normalizeString(input?.oaiDeviceId) ??
        null,
      sessionId:
        normalizeString(input?.sessionId) ??
        normalizeString(input?.oaiSessionId) ??
        null,
      timeoutMs,
    });

    let oauthBootstrap = null;
    if (!probe.ok) {
      currentStage = "oauth_bootstrap";
      oauthBootstrap = await bootstrapChatGptOauthSession(page, context, {
        baseUrl,
        timeoutMs,
        deviceId:
          normalizeString(input?.deviceId) ??
          normalizeString(input?.oaiDeviceId) ??
          normalizeString(probe?.deviceId) ??
          null,
      }).catch(() => null);
      if (oauthBootstrap?.ok) {
        currentStage = "navigate_home_after_bootstrap";
        await navigateWithChallengeSettle(page, `${baseUrl}/`, timeoutMs);
        currentStage = "probe_after_bootstrap";
        probe = await runChatGptBrowserProbe(page, {
          baseUrl,
          modelsPath,
          accessToken,
          languageCode:
            normalizeString(input?.languageCode) ??
            normalizeString(input?.oaiLanguage) ??
            DEFAULT_LANGUAGE_CODE,
          clientVersion:
            normalizeString(input?.clientVersion) ?? DEFAULT_CLIENT_VERSION,
          clientBuildNumber:
            normalizeString(input?.clientBuildNumber) ?? DEFAULT_CLIENT_BUILD_NUMBER,
          deviceId:
            normalizeString(input?.deviceId) ??
            normalizeString(input?.oaiDeviceId) ??
            null,
          sessionId:
            normalizeString(input?.sessionId) ??
            normalizeString(input?.oaiSessionId) ??
            null,
          timeoutMs,
        });
      }
    }

    if (!probe.ok && authSeed?.email && authSeed?.password) {
      currentStage = "login_existing_account";
      await loginExistingAccount(
        page,
        baseUrl,
        authUrl ?? oauthBootstrap?.authUrl ?? null,
        authSeed,
        timeoutMs,
        (stage) => {
          currentStage = stage;
        },
        mailboxContext,
      );
      currentStage = "navigate_home_after_login";
      await navigateWithChallengeSettle(page, `${baseUrl}/`, timeoutMs);
      currentStage = "probe_after_login";
      probe = await runChatGptBrowserProbe(page, {
        baseUrl,
        modelsPath,
        accessToken,
        languageCode:
          normalizeString(input?.languageCode) ??
          normalizeString(input?.oaiLanguage) ??
          DEFAULT_LANGUAGE_CODE,
        clientVersion:
          normalizeString(input?.clientVersion) ?? DEFAULT_CLIENT_VERSION,
        clientBuildNumber:
          normalizeString(input?.clientBuildNumber) ?? DEFAULT_CLIENT_BUILD_NUMBER,
        deviceId:
          normalizeString(input?.deviceId) ??
          normalizeString(input?.oaiDeviceId) ??
          null,
        sessionId:
          normalizeString(input?.sessionId) ??
          normalizeString(input?.oaiSessionId) ??
          null,
        timeoutMs,
      });
    }

    currentStage = "collect_runtime_material";
    const html = await page.content().catch(() => "");
    const cookieHeader = await collectCookieHeader(context, baseUrl);
    const extractedBootstrap = extractBootstrapArtifacts(html);
    const effectiveAuthToken =
      normalizeString(probe?.sessionAccessToken) ??
      normalizeString(extractedBootstrap.accessToken) ??
      accessToken ??
      null;
    const effectiveDeviceId =
      normalizeString(probe?.deviceId) ??
      normalizeString(extractedBootstrap.deviceId) ??
      normalizeString(input?.deviceId) ??
      normalizeString(input?.oaiDeviceId) ??
      findCookieValue(cookieHeader, "oai-did") ??
      null;
    const effectiveSessionId =
      normalizeString(probe?.sessionId) ??
      normalizeString(extractedBootstrap.sessionId) ??
      normalizeString(input?.sessionId) ??
      normalizeString(input?.oaiSessionId) ??
      null;
    const effectiveClientVersion =
      normalizeString(probe?.clientVersion) ??
      normalizeString(input?.clientVersion) ??
      DEFAULT_CLIENT_VERSION;
    const effectiveClientBuildNumber =
      normalizeString(probe?.clientBuildNumber) ??
      normalizeString(extractedBootstrap.clientBuildNumber) ??
      normalizeString(input?.clientBuildNumber) ??
      DEFAULT_CLIENT_BUILD_NUMBER;
    const expiresAt =
      normalizeString(probe?.sessionExpiresAt) ??
      normalizeString(probe?.expiresAt) ??
      decodeJwtExpIso(effectiveAuthToken) ??
      null;
    const powSources = dedupeStrings([
      ...(Array.isArray(probe?.powSources) ? probe.powSources : []),
      ...extractedBootstrap.powSources,
      DEFAULT_POW_SCRIPT,
    ]);
    const powDataBuild =
      normalizeString(probe?.powDataBuild) ??
      normalizeString(extractedBootstrap.powDataBuild) ??
      null;
    const accountName =
      normalizeString(probe?.email) ??
      normalizeString(authSeed?.email) ??
      null;
    const currentUrl = safePageUrl(page);
    const challengePresent =
      detectBrowserChallenge(currentUrl) ||
      detectBrowserChallenge(html) ||
      detectBrowserChallenge(probe?.bodyPreview ?? "");

    let relayResponse = null;
    if (input?.relayRequest && probe?.ok) {
      currentStage = "browser_relay_request";
      relayResponse = await runChatGptBrowserRelay(page, {
        baseUrl,
        requestBody: input.relayRequest.body,
        stream: input.relayRequest.stream === true,
        authUrl: normalizeString(input?.authUrl) ?? null,
        authSeed: input?.authSeed ?? null,
        mailboxContext,
        timeoutMs,
        userAgent:
          normalizeString(input?.userAgent) ??
          DEFAULT_USER_AGENT,
        acceptLanguage:
          normalizeString(input?.acceptLanguage) ??
          normalizeString(input?.language) ??
          DEFAULT_ACCEPT_LANGUAGE,
        languageCode:
          normalizeString(input?.languageCode) ??
          normalizeString(input?.oaiLanguage) ??
          DEFAULT_LANGUAGE_CODE,
        clientVersion:
          normalizeString(probe?.clientVersion) ??
          normalizeString(input?.clientVersion) ??
          DEFAULT_CLIENT_VERSION,
        clientBuildNumber:
          normalizeString(probe?.clientBuildNumber) ??
          normalizeString(input?.clientBuildNumber) ??
          DEFAULT_CLIENT_BUILD_NUMBER,
        deviceId: effectiveDeviceId ?? randomUUID(),
        sessionId:
          normalizeString(probe?.sessionId) ??
          normalizeString(input?.sessionId) ??
          normalizeString(input?.oaiSessionId) ??
          randomUUID(),
      });
    }

    if (!probe?.ok) {
      const preview = normalizeString(probe?.bodyPreview)?.slice(0, 400) ?? "";
      throw createWorkerError(
        503,
        challengePresent
          ? "chatgpt_web_browser_challenge_required"
          : "chatgpt_web_browser_probe_failed",
        preview
          ? `ChatGPT Web browser probe did not reach a usable authenticated backend state. status=${probe?.status ?? "unknown"} preview=${preview}`
          : "ChatGPT Web browser probe did not reach a usable authenticated backend state.",
      );
    }

    if (!effectiveAuthToken && !cookieHeader) {
      throw createWorkerError(
        503,
        "chatgpt_web_session_worker_missing_runtime_material",
        "ChatGPT Web session worker could not materialize any usable auth token or Cookie header from the browser session.",
      );
    }

    const result = {
      ok: true,
      authToken: effectiveAuthToken,
      expiresAt,
      cookieHeader,
      deviceId: effectiveDeviceId,
      sessionId: effectiveSessionId,
      clientVersion: effectiveClientVersion,
      clientBuildNumber: effectiveClientBuildNumber,
      userAgent:
        normalizeString(await page.evaluate(() => navigator.userAgent).catch(() => null)) ??
        normalizeString(input?.userAgent) ??
        DEFAULT_USER_AGENT,
      language:
        normalizeString(input?.language) ??
        normalizeString(input?.acceptLanguage) ??
        DEFAULT_ACCEPT_LANGUAGE,
      languageCode:
        normalizeString(input?.languageCode) ??
        normalizeString(input?.oaiLanguage) ??
        DEFAULT_LANGUAGE_CODE,
      timezone: normalizeString(input?.timezone) ?? DEFAULT_TIMEZONE,
      chatgptPowSources: powSources,
      chatgptPowDataBuild: powDataBuild,
      accountName,
      credentialMaterialKey: accountName ? `chatgpt-web-email:${accountName}` : null,
      authProbe: {
        status: probe?.status ?? null,
        ok: probe?.ok ?? false,
        modelCount: probe?.modelCount ?? 0,
        email: accountName,
      },
      oauthBootstrap: oauthBootstrap ?? null,
      profileSource: {
        mode: profileSource.mode,
        browser: profileSource.browser,
        profileDirectory: profileSource.profileDirectory,
      },
      localStorageKeys: Array.isArray(probe?.localStorageKeys) ? probe.localStorageKeys : [],
      currentUrl,
      challengePresent,
      bodyPreview: probe?.bodyPreview ?? null,
      relayResponse,
      proxyConfigured: Boolean(proxySettings),
      proxyServer: proxySettings?.launchOptions?.server ?? null,
    };
    const credentialFile = await maybeWriteCredentialFile(input, result);
    if (credentialFile) {
      result.credentialFile = credentialFile;
    }
    return result;
  } catch (error) {
    return {
      ok: false,
      error: serializeError(error),
    };
  } finally {
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
    if (clonedUserDataDir) {
      await rm(clonedUserDataDir, { recursive: true, force: true }).catch(() => {});
    }
    if (killTimer) {
      clearTimeout(killTimer);
    }
  }
}

function normalizePath(value) {
  const text = normalizeString(value);
  if (!text) {
    return null;
  }
  return text.startsWith("/") ? text : `/${text}`;
}

async function readStdin() {
  return new Promise((resolve, reject) => {
    let buffer = null;
    let bytes = 0;
    const finish = (error) => {
      clearTimeout(timer);
      process.stdin.off("data", onData);
      process.stdin.off("end", onEnd);
      process.stdin.off("error", onError);
      process.stdin.off("close", onClose);
      process.stdin.pause();
      if (error) reject(error);
      else resolve(buffer?.toString("utf8", 0, bytes).trim() || "{}");
      buffer = null;
    };
    const onData = (chunk) => {
      const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, "utf8");
      if (bytes + data.length > 16 * 1024 * 1024) {
        finish(createWorkerError(413, "chatgpt_web_input_too_large", "ChatGPT worker input exceeds 16 MiB."));
        return;
      }
      // Fixed storage also bounds bookkeeping for arbitrarily tiny pipe chunks.
      buffer ??= Buffer.allocUnsafe(16 * 1024 * 1024);
      data.copy(buffer, bytes);
      bytes += data.length;
    };
    const onEnd = () => finish();
    const onError = (error) => finish(error);
    const onClose = () => finish(createWorkerError(400, "chatgpt_web_input_closed", "ChatGPT worker input closed before EOF."));
    // The browser deadline starts after parsing; bound the preceding pipe read too.
    const timer = setTimeout(() => finish(createWorkerError(
      408, "chatgpt_web_input_timeout", "ChatGPT worker input did not finish within 30 seconds.",
    )), 30_000);
    process.stdin.on("data", onData);
    process.stdin.once("end", onEnd);
    process.stdin.once("error", onError);
    process.stdin.once("close", onClose);
  });
}

function printAndExit(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exit(exitCode);
}

// Normal completion must finish owned resource cleanup before process.exit.
const result = await main();
printAndExit(result, result.ok ? 0 : 1);
