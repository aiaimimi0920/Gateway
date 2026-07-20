import { chromium } from "playwright-core";
import { createHash, randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  isRelayAuthRecoveryState,
  selectNewAssistantTextFromValues,
} from "./chatgpt-web-session-worker-helpers.mjs";

// This worker only refreshes or re-materializes an existing ChatGPT Web session.
// It may use an existing browser profile or best-effort browser login seed, but
// it does not register accounts, solve mailbox/OTP flows, or own EasyProtocol's
// registration pipeline.

const DEFAULT_BASE_URL = "https://chatgpt.com";
const DEFAULT_PROFILE_DIRECTORY = "Default";
const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_ACCEPT_LANGUAGE = "zh-CN,zh;q=0.9,en;q=0.8,en-US;q=0.7";
const DEFAULT_LANGUAGE_CODE = "zh-CN";
const DEFAULT_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";
const DEFAULT_CLIENT_VERSION = "prod-be885abbfcfe7b1f511e88b3003d9ee44757fbad";
const DEFAULT_CLIENT_BUILD_NUMBER = "5955942";
const DEFAULT_TIMEZONE = "Asia/Shanghai";
const DEFAULT_MODELS_PATH = "/backend-api/models?history_and_training_disabled=false";
const DEFAULT_REQUIREMENTS_PATH = "/backend-api/sentinel/chat-requirements";
const DEFAULT_POW_SCRIPT = "https://chatgpt.com/backend-api/sentinel/sdk.js";
const CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH = DEFAULT_REQUIREMENTS_PATH;
const CHATGPT_WEB_DEFAULT_CONVERSATION_PATH = "/backend-api/conversation";
const AUTH_BASE = "https://auth.openai.com";
const PLATFORM_OPENAI_LOGIN_URL = "https://platform.openai.com/login";
const CHATGPT_NEXTAUTH_CSRF_PATH = "/api/auth/csrf";
const CHATGPT_NEXTAUTH_SIGNIN_OPENAI_PATH = "/api/auth/signin/openai";
const LOGIN_OR_CREATE_ACCOUNT_URL = `${AUTH_BASE}/log-in-or-create-account`;
const OPENAI_LOGIN_URL = `${AUTH_BASE}/log-in`;
const DEFAULT_CREDENTIAL_FAMILY_DIR = path.join(
  "chatgpt-platform",
  "chatgpt-web-reverse",
  "session-auth",
);
const WINDOWS_EDGE_EXECUTABLES = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const WINDOWS_CHROME_EXECUTABLES = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
];
const LINUX_CHROMIUM_EXECUTABLES = [
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];
const WINDOWS_EDGE_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Microsoft",
  "Edge",
  "User Data",
);
const WINDOWS_CHROME_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Google",
  "Chrome",
  "User Data",
);
const ROOT_FILES_TO_COPY = ["Local State"];
const PROFILE_FILES_TO_COPY = ["Preferences", "Secure Preferences"];
const PROFILE_DIRS_TO_COPY = [
  "Network",
  "Local Storage",
  "Session Storage",
  "IndexedDB",
  "WebStorage",
  "Service Worker",
  "Shared Dictionary",
];

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
    clearTimeout(killTimer);
    printAndExit(result);
  } catch (error) {
    clearTimeout(killTimer);
    printAndExit(
      {
        ok: false,
        error: serializeError(error),
      },
      1,
    );
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

async function primeBrowserWithImportedCookies(context, baseUrl, input) {
  const cookies = [];
  const fromHeader = parseCookieHeader(
    normalizeString(input?.cookieHeader) ?? normalizeString(input?.headers?.Cookie),
    new URL(baseUrl).hostname,
  );
  cookies.push(...fromHeader);
  if (Array.isArray(input?.sessionCookies)) {
    for (const entry of input.sessionCookies) {
      const normalized = normalizeCookie(entry, new URL(baseUrl).hostname);
      if (normalized) {
        cookies.push(normalized);
      }
    }
  }
  if (cookies.length > 0) {
    await context.addCookies(dedupeCookies(cookies)).catch(() => {});
  }
}

async function navigateWithChallengeSettle(page, url, timeoutMs) {
  await page.goto(url, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  });
  await page.waitForTimeout(2_500);

  for (let index = 0; index < 2; index += 1) {
    const html = await page.content().catch(() => "");
    if (!detectBrowserChallenge(html) && !detectBrowserChallenge(safePageUrl(page))) {
      return;
    }
    await page.waitForTimeout(4_000);
    await page.reload({
      waitUntil: "domcontentloaded",
      timeout: Math.min(timeoutMs, 60_000),
    }).catch(() => {});
  }
}

async function runChatGptBrowserProbe(
  page,
  {
    baseUrl,
    modelsPath,
    accessToken,
    languageCode,
    clientVersion,
    clientBuildNumber,
    deviceId,
    sessionId,
    timeoutMs,
  },
) {
  await page.waitForTimeout(1_000);
  return page.evaluate(
    async ({
      baseUrl,
      modelsPath,
      accessToken,
      languageCode,
      clientVersion,
      clientBuildNumber,
      deviceId,
      sessionId,
      timeoutMs,
    }) => {
      const trimString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const decodeJwtExpIso = (token) => {
        try {
          const [, payload] = String(token ?? "").split(".");
          if (!payload) {
            return null;
          }
          const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
          const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
          const jsonText = atob(padded);
          const decoded = JSON.parse(jsonText);
          return typeof decoded.exp === "number"
            ? new Date(decoded.exp * 1000).toISOString()
            : null;
        } catch {
          return null;
        }
      };
      const detectChallenge = (text) => {
        const lower = String(text ?? "").toLowerCase();
        return (
          lower.includes("cloudflare") ||
          lower.includes("captcha") ||
          lower.includes("challenge") ||
          lower.includes("verify you are human") ||
          lower.includes("just a moment") ||
          lower.includes("attention required")
        );
      };
      const request = async (path, init = {}) => {
        const url = path.startsWith("http://") || path.startsWith("https://")
          ? path
          : `${baseUrl}${path.startsWith("/") ? path : `/${path}`}`;
        const response = await fetch(url, init);
        const text = await response.text();
        let json = null;
        try {
          json = text ? JSON.parse(text) : null;
        } catch {
          json = null;
        }
        return {
          status: response.status,
          ok: response.ok,
          contentType: response.headers.get("content-type"),
          text,
          json,
          challengePresent: detectChallenge(text),
        };
      };

      let sessionProbe = null;
      try {
        sessionProbe = await request("/api/auth/session", {
          method: "GET",
          headers: {
            Accept: "application/json",
          },
          signal: AbortSignal.timeout(Math.min(timeoutMs, 30_000)),
        });
      } catch {
        sessionProbe = null;
      }

      const sessionAccessToken =
        trimString(sessionProbe?.json?.accessToken) ??
        trimString(sessionProbe?.json?.access_token) ??
        null;

      const authHeaders = {};
      if (sessionAccessToken || trimString(accessToken)) {
        authHeaders.Authorization = `Bearer ${sessionAccessToken ?? trimString(accessToken)}`;
      }
      if (trimString(languageCode)) {
        authHeaders["OAI-Language"] = trimString(languageCode);
      }
      if (trimString(clientVersion)) {
        authHeaders["OAI-Client-Version"] = trimString(clientVersion);
      }
      if (trimString(clientBuildNumber)) {
        authHeaders["OAI-Client-Build-Number"] = trimString(clientBuildNumber);
      }
      if (trimString(deviceId)) {
        authHeaders["OAI-Device-Id"] = trimString(deviceId);
      }
      if (trimString(sessionId)) {
        authHeaders["OAI-Session-Id"] = trimString(sessionId);
      }

      const modelsProbe = await request(modelsPath, {
        method: "GET",
        headers: authHeaders,
        signal: AbortSignal.timeout(Math.min(timeoutMs, 30_000)),
      });
      let initProbe = null;
      try {
        initProbe = await request("/backend-api/conversation/init", {
          method: "POST",
          headers: {
            ...authHeaders,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            gizmo_id: null,
            requested_default_model: null,
            conversation_id: null,
            timezone_offset_min: -480,
          }),
          signal: AbortSignal.timeout(Math.min(timeoutMs, 30_000)),
        });
      } catch {
        initProbe = null;
      }

      const pageHtml = document.documentElement?.outerHTML ?? "";
      const scriptSources = Array.from(document.querySelectorAll("script[src]"))
        .map((node) => node.getAttribute("src"))
        .filter((value) => typeof value === "string" && value.includes("/backend-api/sentinel/"));
      const buildCandidates = [];
      const htmlBuild = document.documentElement?.getAttribute("data-build");
      if (trimString(htmlBuild)) {
        buildCandidates.push(trimString(htmlBuild));
      }
      const nextData = document.querySelector("#__NEXT_DATA__");
      if (nextData?.textContent) {
        try {
          const parsed = JSON.parse(nextData.textContent);
          const buildId = trimString(parsed?.buildId);
          if (buildId) {
            buildCandidates.push(buildId);
          }
        } catch {}
      }

      return {
        ok: modelsProbe.ok,
        status: modelsProbe.status,
        bodyPreview: String(modelsProbe.text ?? "").slice(0, 1500),
        contentType: modelsProbe.contentType,
        challengePresent:
          modelsProbe.challengePresent || detectChallenge(pageHtml) || detectChallenge(location.href),
        modelCount: Array.isArray(modelsProbe.json?.categories)
          ? modelsProbe.json.categories.length
          : Array.isArray(modelsProbe.json?.models)
            ? modelsProbe.json.models.length
            : Array.isArray(modelsProbe.json?.items)
              ? modelsProbe.json.items.length
              : Array.isArray(modelsProbe.json)
                ? modelsProbe.json.length
                : 0,
        modelSlugs: Array.isArray(modelsProbe.json?.models)
          ? modelsProbe.json.models
              .map((item) => trimString(item?.slug))
              .filter(Boolean)
          : [],
        conversationInitStatus: initProbe?.status ?? null,
        defaultModelSlug: trimString(initProbe?.json?.default_model_slug),
        conversationInitPreview: String(initProbe?.text ?? "").slice(0, 800),
        sessionStatus: sessionProbe?.status ?? null,
        sessionPreview: String(sessionProbe?.text ?? "").slice(0, 800),
        sessionAccessToken,
        sessionExpiresAt:
          trimString(sessionProbe?.json?.expires) ??
          trimString(sessionProbe?.json?.expires_at) ??
          null,
        email:
          trimString(sessionProbe?.json?.user?.email) ??
          trimString(sessionProbe?.json?.email) ??
          null,
        deviceId:
          trimString(deviceId) ??
          trimString(localStorage.getItem("oai/device_id")) ??
          trimString(localStorage.getItem("oaiDeviceId")),
        sessionId:
          trimString(sessionId) ??
          trimString(localStorage.getItem("oai/session_id")) ??
          trimString(localStorage.getItem("oaiSessionId")),
        clientVersion,
        clientBuildNumber,
        expiresAt: decodeJwtExpIso(accessToken),
        powSources: scriptSources,
        powDataBuild: buildCandidates.find(Boolean) ?? null,
        localStorageKeys: Object.keys(localStorage),
      };
    },
    {
      baseUrl,
      modelsPath,
      accessToken,
      languageCode,
      clientVersion,
      clientBuildNumber,
      deviceId,
      sessionId,
      timeoutMs,
    },
  );
}

async function loginExistingAccount(
  page,
  baseUrl,
  authUrl,
  authSeed,
  timeoutMs,
  setStage = () => {},
  mailboxContext = null,
) {
  if (!authSeed?.email || !authSeed?.password) {
    throw createWorkerError(
      401,
      "chatgpt_web_browser_login_seed_missing",
      "Browser-backed ChatGPT Web login requires an existing account email and plain password.",
    );
  }

  const emailSelectors = 'input[type="email"], input[name="email"], input[autocomplete="username"]';
  setStage("login_navigate_email_surface");
  const emailPageState = await navigateToEmailSurface(page, {
    baseUrl,
    authUrl,
    timeoutMs,
  });

  let emailInput = page.locator(emailSelectors).first();
  for (let attempt = 0; attempt < 8; attempt += 1) {
    if ((await emailInput.count()) > 0) {
      break;
    }
    await clickPreLoginButtons(page).catch(() => {});
    await page.waitForTimeout(1_000);
    emailInput = page.locator(emailSelectors).first();
  }
  if ((await emailInput.count()) === 0) {
    const domSubmitted = await submitAuthEmailViaDom(page, authSeed.email);
    if (domSubmitted) {
      await page.waitForTimeout(2_000);
    } else {
      throw createWorkerError(
        500,
        "chatgpt_web_browser_login_email_missing",
        `Could not locate the ChatGPT/OpenAI email input while refreshing an existing account session. url=${safePageUrl(page)} state=${compactPageState(emailPageState)}`,
      );
    }
  } else {
    setStage("login_fill_email");
    const filledEmail = await fillVisibleInputBySelectors(page, emailSelectors, authSeed.email);
    if (!filledEmail) {
      const domSubmitted = await submitAuthEmailViaDom(page, authSeed.email);
      if (!domSubmitted) {
        const emailState = await collectPageState(page);
        throw createWorkerError(
          500,
          "chatgpt_web_browser_login_email_fill_failed",
          `ChatGPT/OpenAI login found an email input node but could not interact with it. state=${compactPageState(emailState)}`,
        );
      }
      await page.waitForTimeout(2_000);
    } else {
      setStage("login_submit_email");
      for (let attempt = 0; attempt < 5; attempt += 1) {
        const clicked = await clickButtonByExactText(page, [
          "Continue with email",
          "使用电子邮件继续",
          "Continue",
          "继续",
        ]);
        if (clicked) {
          break;
        }
        await page.keyboard.press("Enter").catch(() => {});
        await page.waitForTimeout(1_000);
      }
      await page.waitForTimeout(2_500);
    }
  }
  let emailVerificationHandled = await maybeCompleteEmailVerification(
    page,
    mailboxContext,
    timeoutMs,
    setStage,
    authSeed.email,
  );

  const passwordInput = page.locator('input[type="password"]').first();
  let lastBodyText = "";
  setStage("login_wait_password_surface");
  const passwordRecoveryTimeoutMs = Math.min(timeoutMs, 3_000);
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const loopState = await collectPageState(page);
    if (pageStateRequiresEmailVerification(loopState) && !emailVerificationHandled) {
      emailVerificationHandled = await maybeCompleteEmailVerification(
        page,
        mailboxContext,
        timeoutMs,
        setStage,
        authSeed.email,
      );
    }
    lastBodyText = await page.locator("body").innerText().catch(() => "");
    if (lastBodyText.includes("Operation timed out") || lastBodyText.includes("重试")) {
      const retryButton = page.locator('button:has-text("重试"), button:has-text("Retry")').first();
      if ((await retryButton.count()) > 0) {
        await retryButton.click().catch(() => {});
      } else {
        await page.reload({
          waitUntil: "domcontentloaded",
          timeout: passwordRecoveryTimeoutMs,
        }).catch(() => {});
      }
    }
    await page.waitForTimeout(1_500);
    if ((await passwordInput.count()) > 0) {
      break;
    }
  }
  if ((await passwordInput.count()) === 0) {
    const passwordState = await collectPageState(page);
    if (pageStateRequiresEmailVerification(passwordState) && !emailVerificationHandled) {
      emailVerificationHandled = await maybeCompleteEmailVerification(
        page,
        mailboxContext,
        timeoutMs,
        setStage,
        authSeed.email,
      );
      await page.waitForTimeout(2_000);
    }
  }
  if ((await passwordInput.count()) === 0) {
    const passwordState = await collectPageState(page);
    const currentHref = String(passwordState?.href ?? "").toLowerCase();
    const normalizedBaseUrl = String(baseUrl ?? "").toLowerCase();
    const homeRecovered =
      normalizedBaseUrl &&
      currentHref.startsWith(normalizedBaseUrl) &&
      !pageStateRequiresAuthRecovery(passwordState) &&
      !passwordState?.hasEmail &&
      !passwordState?.hasPassword;
    if (homeRecovered) {
      return;
    }
    if (pageStateRequiresEmailVerification(passwordState) && emailVerificationHandled) {
      throw createWorkerError(
        500,
        "chatgpt_web_email_otp_submit_stuck",
        `ChatGPT/OpenAI email verification remained on the OTP page after one repo-owned OTP/password fallback attempt. state=${compactPageState(passwordState)}`,
      );
    }
    if (lastBodyText.includes("Operation timed out") || lastBodyText.includes("重试")) {
      throw createWorkerError(
        503,
        "chatgpt_web_browser_login_timed_out",
        `ChatGPT/OpenAI browser login stalled on the auth page with \`Operation timed out\`, so the worker could not reach the password step. state=${compactPageState(passwordState)}`,
      );
    }
    throw createWorkerError(
      500,
      "chatgpt_web_browser_login_password_missing",
      `Could not locate the ChatGPT/OpenAI password input while refreshing an existing account session. state=${compactPageState(passwordState)}`,
    );
  }
  setStage("login_fill_password");
  const filledPassword = await fillVisibleInputBySelectors(
    page,
    'input[type="password"], input[name="password"], input[name="new-password"]',
    authSeed.password,
  );
  if (!filledPassword) {
    const passwordState = await collectPageState(page);
    throw createWorkerError(
      500,
      "chatgpt_web_browser_login_password_fill_failed",
      `ChatGPT/OpenAI password field existed but could not be filled. state=${compactPageState(passwordState)}`,
    );
  }

  const submitButton = page
    .locator(
      'button:has-text("Log in"), button:has-text("登录"), button:has-text("Continue"), button:has-text("继续"), button[type="submit"]',
    )
    .first();
  setStage("login_submit_password");
  const clickedSubmitButton = await clickButtonByExactText(page, [
    "Log in",
    "登录",
    "Continue",
    "继续",
  ]);
  if (clickedSubmitButton) {
    // already clicked above
  } else if ((await submitButton.count()) > 0) {
    await submitButton.click({ timeout: 15_000 }).catch(() => {});
  } else {
    await page.keyboard.press("Enter").catch(() => {});
  }

  setStage("login_post_submit_wait");
  await page.waitForTimeout(8_000);
}

async function maybeCompleteEmailVerification(
  page,
  mailboxContext,
  timeoutMs,
  setStage = () => {},
  emailAddress = null,
) {
  const verificationStartedAt = Date.now();
  const verificationMarkerFloor = verificationStartedAt - 5_000;
  const verificationState = await collectPageState(page);
  if (!pageStateRequiresEmailVerification(verificationState)) {
    return false;
  }
  setStage("login_email_verification_choose_password");
  const usedPasswordFallback = await clickButtonByExactText(page, [
    "使用密码继续",
    "Continue with password",
    "Use password instead",
  ]);
  if (usedPasswordFallback) {
    await page.waitForTimeout(3_000);
    const passwordFallbackState = await collectPageState(page);
    if (passwordFallbackState?.hasEmail && emailAddress) {
      await submitEmailAddress(page, emailAddress);
      await page.waitForTimeout(2_500);
    }
    return true;
  }
  if (!mailboxContext?.mailboxRef || !mailboxContext?.mailboxSessionId) {
    throw createWorkerError(
      412,
      "chatgpt_web_mailbox_context_missing",
      `ChatGPT/OpenAI login reached email verification, but no mailboxRef/mailboxSessionId was available. state=${compactPageState(verificationState)}`,
    );
  }
  if (!mailboxContext?.serviceBaseUrl || !mailboxContext?.apiKey) {
    throw createWorkerError(
      412,
      "chatgpt_web_mailbox_service_missing",
      `ChatGPT/OpenAI login reached email verification, but mailbox service configuration is missing. state=${compactPageState(verificationState)}`,
    );
  }

  const effectiveMailboxContext = await ensureRecoveredMailboxContext(mailboxContext);
  setStage("login_wait_email_otp");
  let code = await fetchImmediateMailboxCode(effectiveMailboxContext, {
    minMarker: verificationMarkerFloor,
  }).catch(() => "");
  if (!code) {
    const minMarker = await fetchMailboxLatestMarker(effectiveMailboxContext).catch(() =>
      fetchMailboxSnapshotLatestMarker(effectiveMailboxContext).catch(() => 0),
    );
    await clickOtpResendIfAvailable(page);
    code = await waitForMailboxOpenAiCode(effectiveMailboxContext, {
      timeoutMs: Math.min(Math.max(timeoutMs, 15_000), 30_000),
      minMarker: Math.max(Number(minMarker || 0), verificationMarkerFloor),
    });
  }
  setStage("login_fill_email_otp");
  await fillEmailVerificationCode(page, code);
  setStage("login_submit_email_otp");
  const continueButton = page
    .locator('button:has-text("继续"), button:has-text("Continue"), button[type="submit"]')
    .first();
  if ((await continueButton.count()) > 0) {
    await continueButton.click({ timeout: 10_000 }).catch(() => {});
  } else {
    await page.keyboard.press("Enter").catch(() => {});
  }
  await page.waitForTimeout(4_000);
  const postOtpState = await collectPageState(page);
  if (pageStateRequiresEmailVerification(postOtpState)) {
    throw createWorkerError(
      500,
      "chatgpt_web_email_otp_submit_stuck",
      `ChatGPT/OpenAI email verification remained on the OTP page after submitting a recovered code. state=${compactPageState(postOtpState)}`,
    );
  }
  return true;
}

async function clickOtpResendIfAvailable(page) {
  const retryButton = page
    .locator('button:has-text("重新发送电子邮件"), button:has-text("Resend email"), button:has-text("Resend")')
    .first();
  if ((await retryButton.count()) > 0) {
    await retryButton.click({ timeout: 5_000 }).catch(() => {});
    await page.waitForTimeout(1_500);
    return true;
  }
  return false;
}

async function fillEmailVerificationCode(page, code) {
  const digits = String(code ?? "").trim();
  if (!/^\d{6}$/.test(digits)) {
    throw createWorkerError(
      500,
      "chatgpt_web_mailbox_code_invalid",
      "Mailbox OTP helper did not produce a valid 6-digit OpenAI verification code.",
    );
  }
  const inputs = page.locator(
    'input[autocomplete="one-time-code"], input[inputmode="numeric"], input[name*="otp" i], input[name*="code" i], input[type="tel"], input[type="number"]',
  );
  const count = await inputs.count().catch(() => 0);
  if (count <= 0) {
    throw createWorkerError(
      500,
      "chatgpt_web_email_verification_input_missing",
      "ChatGPT/OpenAI email verification page did not expose any OTP input fields.",
    );
  }
  if (count === 1) {
    await inputs.first().fill(digits);
    return;
  }
  for (let index = 0; index < Math.min(6, count); index += 1) {
    await inputs.nth(index).fill(digits[index] ?? "");
  }
}

async function submitEmailAddress(page, emailAddress) {
  const emailSelectors = 'input[type="email"], input[name="email"], input[autocomplete="username"]';
  const emailInput = page.locator(emailSelectors).first();
  if ((await emailInput.count()) <= 0) {
    return false;
  }
  const filled = await fillVisibleInputBySelectors(page, emailSelectors, String(emailAddress ?? ""));
  if (!filled) {
    return false;
  }
  await page.keyboard.press("Tab").catch(() => {});
  if (
    await clickButtonByExactText(page, [
      "Continue with email",
      "使用电子邮件继续",
      "Continue",
      "继续",
    ])
  ) {
    return true;
  }
  await page.keyboard.press("Enter").catch(() => {});
  return true;
}

async function fillVisibleInputBySelectors(page, selectors, value) {
  const locator = page.locator(selectors);
  const count = await locator.count().catch(() => 0);
  for (let index = 0; index < count; index += 1) {
    const input = locator.nth(index);
    const visible = await input.isVisible().catch(() => false);
    const enabled = await input.isEnabled().catch(() => false);
    if (!visible || !enabled) {
      continue;
    }
    const filled = await input
      .click({ timeout: 5_000 })
      .then(async () => {
        await input.fill("");
        await input.pressSequentially(String(value ?? ""), { delay: 40 });
        return true;
      })
      .catch(() => false);
    if (filled) {
      return true;
    }
  }

  return page
    .evaluate(
      ({ selectors, value }) => {
        const candidates = Array.from(document.querySelectorAll(selectors));
        const visibleCandidate =
          candidates.find((element) => {
            const style = window.getComputedStyle(element);
            const rect = element.getBoundingClientRect();
            return (
              rect.width > 0 &&
              rect.height > 0 &&
              style.display !== "none" &&
              style.visibility !== "hidden" &&
              !element.disabled
            );
          }) ?? candidates[0];
        if (!visibleCandidate) {
          return false;
        }
        visibleCandidate.focus();
        visibleCandidate.value = "";
        visibleCandidate.dispatchEvent(new Event("input", { bubbles: true }));
        visibleCandidate.value = String(value ?? "");
        visibleCandidate.dispatchEvent(new Event("input", { bubbles: true }));
        visibleCandidate.dispatchEvent(new Event("change", { bubbles: true }));
        return true;
      },
      { selectors, value: String(value ?? "") },
    )
    .catch(() => false);
}

async function submitAuthEmailViaDom(page, email) {
  return page
    .evaluate((value) => {
      const input = document.querySelector(
        'input[type="email"], input[name="email"], input[autocomplete="username"]',
      );
      if (!input) {
        return false;
      }
      const buttons = Array.from(document.querySelectorAll("button"));
      const continueButton = buttons.find((button) => {
        const text = String(button.innerText || button.textContent || "").trim();
        return (
          text === "Continue" ||
          text === "继续" ||
          text === "Continue with email" ||
          text === "使用电子邮件继续"
        );
      });
      const nativeSetter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set;
      input.focus();
      if (nativeSetter) {
        nativeSetter.call(input, "");
        nativeSetter.call(input, String(value || ""));
      } else {
        input.value = "";
        input.value = String(value || "");
      }
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      continueButton?.click();
      return true;
    }, email)
    .catch(() => false);
}

async function navigateToEmailSurface(page, { baseUrl, authUrl, timeoutMs }) {
  const normalizedAuthUrl = normalizeString(authUrl);
  const authAuthorizeCandidate =
    normalizedAuthUrl && normalizedAuthUrl.includes("/api/accounts/authorize")
      ? normalizedAuthUrl
      : null;
  const authLoginCandidate =
    normalizedAuthUrl && !authAuthorizeCandidate ? normalizedAuthUrl : null;
  const candidateUrls = dedupeStrings([
    authLoginCandidate,
    `${baseUrl}/auth/login_with?callback_path=/`,
    LOGIN_OR_CREATE_ACCOUNT_URL,
    OPENAI_LOGIN_URL,
    `${baseUrl}/auth/login`,
    `${baseUrl}/auth/login_with`,
    authAuthorizeCandidate,
  ]);
  const navTimeoutMs = Math.min(timeoutMs, 12_000);
  let lastState = null;
  for (const url of candidateUrls) {
    await page.goto(url, {
      waitUntil: "domcontentloaded",
      timeout: navTimeoutMs,
    }).catch(() => {});
    for (let attempt = 0; attempt < 2; attempt += 1) {
      await page.waitForTimeout(attempt === 0 ? 2_000 : 1_500);
      lastState = await collectPageState(page);
      if (lastState?.hasEmail) {
        return lastState;
      }
      if (String(lastState?.href ?? "").toLowerCase().includes("/auth/error")) {
        break;
      }
      if (
        pageStateRequiresAuthRecovery(lastState) ||
        (Array.isArray(lastState?.authLinks) && lastState.authLinks.length > 0)
      ) {
        await clickPreLoginButtons(page);
      }
      lastState = await collectPageState(page);
      if (lastState?.hasEmail) {
        return lastState;
      }
      if (String(lastState?.href ?? "").toLowerCase().includes("/auth/error")) {
        break;
      }
      if (pageStateIsCloudflareWait(lastState)) {
        await page.reload({
          waitUntil: "domcontentloaded",
          timeout: navTimeoutMs,
        }).catch(() => {});
      }
    }
  }
  return lastState;
}

async function clickPreLoginButtons(page) {
  const selectors = [
    "a[href*=\"/auth/login_with\"]",
    "a[href*=\"auth.openai.com\"]",
    "button:has-text(\"Log in\")",
    "button:has-text(\"登录\")",
    "a:has-text(\"Log in\")",
    "a:has-text(\"登录\")",
    "button:has-text(\"Continue with email\")",
    "button:has-text(\"使用电子邮件继续\")",
    "a:has-text(\"Continue with email\")",
    "a:has-text(\"使用电子邮件继续\")",
    "button:has-text(\"Continue\")",
    "button:has-text(\"继续\")",
    "a:has-text(\"Continue\")",
    "a:has-text(\"继续\")",
    "button:has-text(\"Try again\")",
    "button:has-text(\"Retry\")",
    "button:has-text(\"重试\")",
    "button:has-text(\"返回\")",
    "button:has-text(\"Back\")",
    "button:has-text(\"Return\")",
    "a:has-text(\"Try again\")",
    "a:has-text(\"Retry\")",
    "a:has-text(\"重试\")",
    "a:has-text(\"返回\")",
    "a:has-text(\"Back\")",
    "a:has-text(\"Return\")",
  ];
  for (const selector of selectors) {
    const button = await page.$(selector);
    if (!button) {
      continue;
    }
    await button.click().catch(() => {});
    await page.waitForTimeout(1_000);
    return true;
  }
  return false;
}

async function clickButtonByExactText(page, texts) {
  const normalizedTargets = new Set(
    (Array.isArray(texts) ? texts : [])
      .map((value) => String(value ?? "").trim())
      .filter(Boolean),
  );
  if (normalizedTargets.size === 0) {
    return false;
  }
  const buttons = await page.locator("button").elementHandles().catch(() => []);
  for (const button of buttons) {
    const text = await button.innerText().catch(() => "");
    const normalizedText = String(text ?? "").replace(/\s+/g, " ").trim();
    if (!normalizedTargets.has(normalizedText)) {
      continue;
    }
    const clicked = await button
      .click({ timeout: 3_000, force: true })
      .then(() => true)
      .catch(async () => {
        return button
          .evaluate((element) => {
            element.click();
            return true;
          })
          .catch(() => false);
      });
    if (!clicked) {
      continue;
    }
    await page.waitForTimeout(1_000);
    return true;
  }
  return false;
}

async function bootstrapChatGptOauthSession(page, context, { baseUrl, timeoutMs, deviceId }) {
  const effectiveDeviceId = normalizeString(deviceId) ?? randomUUID();
  const authSessionId = randomUUID();

  await page.goto(PLATFORM_OPENAI_LOGIN_URL, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  }).catch(() => {});
  await page.goto(`${baseUrl}/auth/login`, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  }).catch(() => {});

  const bootstrap = await page.evaluate(
    async ({
      callbackUrl,
      csrfPath,
      signinPath,
      deviceId,
      authSessionId,
    }) => {
      const fail = (error) => ({ ok: false, error: String(error || "unknown") });
      try {
        const csrfResponse = await fetch(csrfPath, {
          credentials: "include",
          headers: {
            Accept: "application/json",
          },
        });
        if (!csrfResponse.ok) {
          return fail(`csrf_status_${csrfResponse.status}`);
        }
        const csrfPayload = await csrfResponse.json().catch(() => null);
        const csrfToken = String(csrfPayload?.csrfToken || "").trim();
        if (!csrfToken) {
          return fail("chatgpt_nextauth_csrf_missing_token");
        }
        const query = new URLSearchParams({
          prompt: "login",
          screen_hint: "login_or_signup",
          device_id: deviceId,
          "ext-oai-did": deviceId,
          auth_session_logging_id: authSessionId,
        });
        const signinResponse = await fetch(`${signinPath}?${query.toString()}`, {
          method: "POST",
          credentials: "include",
          headers: {
            Accept: "application/json",
            "Content-Type": "application/x-www-form-urlencoded",
          },
          body: new URLSearchParams({
            csrfToken,
            callbackUrl,
            json: "true",
          }).toString(),
        });
        if (!signinResponse.ok) {
          return fail(`signin_status_${signinResponse.status}`);
        }
        const signinPayload = await signinResponse.json().catch(() => null);
        const authUrl = String(signinPayload?.url || "").trim();
        if (!authUrl) {
          return fail("chatgpt_nextauth_signin_missing_url");
        }
        const state = new URL(authUrl).searchParams.get("state") || "";
        if (!state) {
          return fail("chatgpt_nextauth_signin_missing_state");
        }
        return {
          ok: true,
          authUrl,
          authState: String(state),
          cookieNames: document.cookie
            .split(";")
            .map((entry) => String(entry || "").split("=")[0].trim())
            .filter(Boolean)
            .slice(0, 100),
        };
      } catch (error) {
        return fail(error);
      }
    },
    {
      callbackUrl: `${baseUrl}/`,
      csrfPath: CHATGPT_NEXTAUTH_CSRF_PATH,
      signinPath: CHATGPT_NEXTAUTH_SIGNIN_OPENAI_PATH,
      deviceId: effectiveDeviceId,
      authSessionId,
    },
  );
  if (!bootstrap?.ok || !bootstrap?.authUrl) {
    return bootstrap ?? { ok: false, error: "chatgpt_nextauth_bootstrap_failed" };
  }

  await page.goto(bootstrap.authUrl, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  }).catch(() => {});
  const authReady = await waitForAuthCookieState(context, page, {
    urls: [bootstrap.authUrl, `${baseUrl}/`, LOGIN_OR_CREATE_ACCOUNT_URL],
    timeoutMs: Math.min(timeoutMs, 25_000),
    cookieNames: ["login_session", "oai-client-auth-session", "hydra_redirect"],
    requireRgContextStb: false,
  });

  await page.goto(LOGIN_OR_CREATE_ACCOUNT_URL, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  }).catch(() => {});
  const entryReady = await waitForAuthCookieState(context, page, {
    urls: [LOGIN_OR_CREATE_ACCOUNT_URL, bootstrap.authUrl, `${baseUrl}/`],
    timeoutMs: Math.min(timeoutMs, 30_000),
    cookieNames: ["login_session", "oai-client-auth-session", "hydra_redirect", "oai-sc", "rg_context"],
    requireRgContextStb: true,
  });

  return {
    ok: Boolean(authReady.ready || entryReady.ready),
    authUrl: bootstrap.authUrl,
    authState: bootstrap.authState,
    deviceId: effectiveDeviceId,
    authCookieReady: authReady.ready,
    entryCookieReady: entryReady.ready,
    entryRgContext: entryReady.rgContext ?? null,
    currentUrl: safePageUrl(page),
    pageState: entryReady.pageState ?? authReady.pageState ?? null,
    cookieNames: entryReady.cookieNames ?? authReady.cookieNames ?? bootstrap.cookieNames ?? [],
  };
}

async function waitForAuthCookieState(
  context,
  page,
  { urls, timeoutMs, cookieNames, requireRgContextStb },
) {
  const deadline = Date.now() + Math.max(5_000, timeoutMs);
  let lastState = null;
  while (Date.now() < deadline) {
    lastState = await collectPageState(page);
    const cookies = await context.cookies(urls).catch(() => []);
    const cookieMap = new Map();
    for (const cookie of cookies) {
      const name = normalizeString(cookie?.name);
      if (!name || cookieMap.has(name)) {
        continue;
      }
      cookieMap.set(name, String(cookie?.value ?? ""));
    }
    const names = Array.from(cookieMap.keys());
    const ready = cookieNames.every((name) => cookieMap.has(name));
    const rgContext = normalizeString(cookieMap.get("rg_context"));
    const challengePresent = pageStateIsCloudflareWait(lastState);
    const stateReady =
      ready &&
      (!requireRgContextStb || rgContext === "stb") &&
      !challengePresent;
    if (stateReady) {
      return {
        ready: true,
        cookieNames: names,
        rgContext,
        pageState: lastState,
      };
    }
    await page.waitForTimeout(2_000);
  }
  return {
    ready: false,
    cookieNames: [],
    rgContext: null,
    pageState: lastState,
  };
}

async function collectPageState(page) {
  return page.evaluate(() => ({
    href: location.href,
    title: document.title,
    readyState: document.readyState,
    bodyText: String((document.body && document.body.innerText) || "").slice(0, 400),
    hasEmail: !!document.querySelector('input[type="email"], input[name="email"], input[autocomplete="username"]'),
    hasPassword: !!document.querySelector('input[type="password"], input[name="password"], input[name="new-password"]'),
    buttonTexts: Array.from(document.querySelectorAll("button"))
      .map((element) => String((element.innerText || element.textContent || "")).trim())
      .filter(Boolean)
      .slice(0, 12),
    anchorTexts: Array.from(document.querySelectorAll("a"))
      .map((element) => String((element.innerText || element.textContent || "")).trim())
      .filter(Boolean)
      .slice(0, 12),
    authLinks: Array.from(document.querySelectorAll('a[href]'))
      .map((element) => String(element.getAttribute("href") || "").trim())
      .filter(Boolean)
      .filter((href) => href.includes("auth.openai.com") || href.includes("/auth/login_with"))
      .slice(0, 8),
  })).catch(() => ({}));
}

function pageStateIsCloudflareWait(pageState) {
  const href = String(pageState?.href ?? "").toLowerCase();
  const title = String(pageState?.title ?? "").toLowerCase();
  const body = String(pageState?.bodyText ?? "").toLowerCase();
  const combined = [href, title, body].join("\n");
  return [
    "just a moment",
    "attention required",
    "verify you are human",
    "cdn-cgi/challenge-platform",
    "performing security verification",
  ].some((marker) => combined.includes(marker));
}

function compactPageState(pageState) {
  if (!pageState || typeof pageState !== "object") {
    return "<none>";
  }
  const buttonTexts = Array.isArray(pageState.buttonTexts)
    ? pageState.buttonTexts.slice(0, 6).join("|")
    : "";
  const anchorTexts = Array.isArray(pageState.anchorTexts)
    ? pageState.anchorTexts.slice(0, 4).join("|")
    : "";
  const authLinks = Array.isArray(pageState.authLinks)
    ? pageState.authLinks.slice(0, 3).join("|")
    : "";
  const href = String(pageState.href ?? "").slice(0, 120);
  const title = String(pageState.title ?? "").slice(0, 80);
  const bodyText = String(pageState.bodyText ?? "").replace(/\s+/g, " ").slice(0, 160);
  return `href=${href} title=${title} hasEmail=${Boolean(pageState.hasEmail)} hasPassword=${Boolean(pageState.hasPassword)} buttons=${buttonTexts} anchors=${anchorTexts} authLinks=${authLinks} body=${bodyText}`;
}

function pageStateRequiresEmailVerification(pageState) {
  const href = String(pageState?.href ?? "").toLowerCase();
  const body = String(pageState?.bodyText ?? "").toLowerCase();
  const title = String(pageState?.title ?? "").toLowerCase();
  return (
    href.includes("email-verification") ||
    body.includes("检查您的收件箱") ||
    body.includes("验证码") ||
    body.includes("verification code") ||
    body.includes("check your inbox") ||
    title.includes("检查您的收件箱")
  );
}

function pageStateRequiresAuthRecovery(pageState) {
  const href = String(pageState?.href ?? "").toLowerCase();
  const title = String(pageState?.title ?? "").toLowerCase();
  const body = String(pageState?.bodyText ?? "").toLowerCase();
  const links = Array.isArray(pageState?.authLinks)
    ? pageState.authLinks.map((value) => String(value).toLowerCase()).join("\n")
    : "";
  const combined = [href, title, body, links].join("\n");
  return [
    "auth.openai.com",
    "/auth/login_with",
    "/api/auth/error",
    "/auth/error",
    "your session has ended",
    "你的会话已结束",
    "log in to continue",
    "登录以继续",
    "continue to chatgpt",
    "log-in-or-create-account",
  ].some((marker) => combined.includes(marker));
}

function resolveMailboxContext(input, authSeed) {
  const mailboxRef = normalizeString(input?.mailboxRef);
  const mailboxSessionId =
    normalizeString(input?.mailboxSessionId) ?? decodeMailboxSessionId(mailboxRef);
  const mailboxProviderKey =
    normalizeString(input?.mailboxProviderKey) ?? decodeMailboxProviderKey(mailboxRef);
  const mailboxProviderInstanceId = decodeMailboxProviderInstanceId(mailboxRef);
  const decodedMailboxAddress = decodeMailboxAddress(mailboxRef);
  const configuredBaseUrl =
    normalizeString(process.env.CHATGPT_WEB_MAILBOX_SERVICE_BASE_URL) ??
    normalizeString(process.env.MAILBOX_SERVICE_BASE_URL) ??
    (process.platform === "win32" ? "http://127.0.0.1:18081" : "http://easy-email:8080");
  const apiKey =
    normalizeString(process.env.CHATGPT_WEB_MAILBOX_SERVICE_API_KEY) ??
    normalizeString(process.env.MAILBOX_SERVICE_API_KEY);
  return {
    mailboxRef,
    mailboxSessionId,
    mailboxProviderKey,
    mailboxProviderInstanceId,
    mailboxAddress: decodedMailboxAddress,
    email: normalizeString(authSeed?.email),
    hostId:
      normalizeString(input?.mailboxHostId) ??
      normalizeString(process.env.CHATGPT_WEB_MAILBOX_HOST_ID) ??
      "python-register-orchestration",
    serviceBaseUrl: configuredBaseUrl,
    apiKey,
  };
}

function decodeMailboxSessionId(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const index = text.indexOf(":");
  if (index < 0) {
    return text;
  }
  return text.slice(index + 1).trim() || null;
}

function decodeMailboxProviderKey(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const index = text.indexOf(":");
  if (index < 0) {
    return null;
  }
  return text.slice(0, index).trim() || null;
}

function decodeMailboxProviderInstanceId(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const first = text.indexOf(":");
  if (first < 0) {
    return null;
  }
  const second = text.indexOf(":", first + 1);
  if (second < 0) {
    return null;
  }
  return text.slice(first + 1, second).trim() || null;
}

function decodeMailboxAddress(mailboxRef) {
  const payload = decodeMailboxRefPayload(mailboxRef);
  const address =
    normalizeString(payload?.address) ??
    normalizeString(payload?.email) ??
    normalizeString(payload?.mailbox);
  return address?.toLowerCase() ?? null;
}

function decodeMailboxRefPayload(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const first = text.indexOf(":");
  if (first < 0) {
    return null;
  }
  const second = text.indexOf(":", first + 1);
  if (second < 0) {
    return null;
  }
  const encoded = text.slice(second + 1).trim();
  if (!encoded) {
    return null;
  }
  try {
    const payload = JSON.parse(decodeURIComponent(encoded));
    return payload && typeof payload === "object" ? payload : null;
  } catch {
    return null;
  }
}

async function ensureRecoveredMailboxContext(mailboxContext) {
  if (!mailboxContext?.email || !mailboxContext?.mailboxProviderKey) {
    return mailboxContext;
  }
  try {
    const recovered = await recoverMailboxByEmail(mailboxContext, {
      emailAddress: mailboxContext.email,
      providerTypeKey: mailboxContext.mailboxProviderKey,
      hostId: mailboxContext.hostId,
    });
    if (recovered?.mailboxSessionId) {
      return {
        ...mailboxContext,
        mailboxRef: recovered.mailboxRef ?? mailboxContext.mailboxRef,
        mailboxSessionId: recovered.mailboxSessionId,
      };
    }
  } catch {
    // best-effort only; keep the original mailbox session if recovery fails
  }
  return mailboxContext;
}

async function fetchMailboxLatestMarker(mailboxContext) {
  const payload = await fetchMailboxCodePayload(mailboxContext);
  const codeObject = payload?.code;
  return extractMailboxCodeMarker(codeObject);
}

async function fetchMailboxSnapshotLatestMarker(mailboxContext) {
  const snapshot = await fetchMailboxSnapshot(mailboxContext, { markerOnly: true });
  return Number(snapshot?.marker || 0);
}

async function fetchImmediateMailboxCode(mailboxContext, options = {}) {
  const minMarker = Number(options?.minMarker || 0);
  const payload = await fetchMailboxCodePayload(mailboxContext);
  const directCode = selectOpenAiVerificationCode(payload?.code);
  const directMarker = extractMailboxCodeMarker(payload?.code);
  if (directCode && (!minMarker || directMarker >= minMarker)) {
    return directCode;
  }
  const snapshot = await fetchMailboxSnapshot(mailboxContext).catch(() => null);
  if (snapshot?.code && (!minMarker || Number(snapshot.marker || 0) >= minMarker)) {
    return snapshot.code;
  }
  const providerCode = await fetchProviderDirectMailboxCode(mailboxContext).catch(() => null);
  if (providerCode?.code && (!minMarker || Number(providerCode.marker || 0) >= minMarker)) {
    return providerCode.code;
  }
  return "";
}

async function recoverMailboxByEmail(mailboxContext, payload) {
  const response = await fetch(
    `${mailboxContext.serviceBaseUrl.replace(/\/+$/, "")}/mail/mailboxes/recover-by-email`,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${mailboxContext.apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    },
  );
  if (!response.ok) {
    throw new Error(`mailbox_recover_status_${response.status}`);
  }
  const raw = await response.json().catch(() => ({}));
  const result = raw?.result && typeof raw.result === "object" ? raw.result : raw;
  const session = result?.session && typeof result.session === "object" ? result.session : null;
  return session
    ? {
        mailboxRef: normalizeString(session.mailboxRef),
        mailboxSessionId: normalizeString(session.id),
      }
    : null;
}

async function waitForMailboxOpenAiCode(mailboxContext, { timeoutMs, minMarker = 0 }) {
  const deadline = Date.now() + Math.max(5_000, timeoutMs);
  let lastError = null;
  let lastSeenMarker = Number(minMarker || 0);
  while (Date.now() < deadline) {
    let snapshot = null;
    try {
      const payload = await fetchMailboxCodePayload(mailboxContext);
      const codeObject = payload?.code;
      const code = selectOpenAiVerificationCode(codeObject);
      const marker = extractMailboxCodeMarker(codeObject);
      if (code && (!lastSeenMarker || marker > lastSeenMarker)) {
        return code;
      }
    } catch (error) {
      lastError = error;
    }
    try {
      snapshot = await fetchMailboxSnapshot(mailboxContext);
      const snapshotCode = snapshot?.code;
      if (snapshotCode && (!lastSeenMarker || Number(snapshot.marker || 0) > lastSeenMarker)) {
        return snapshotCode;
      }
    } catch (error) {
      lastError ??= error;
    }
    try {
      const providerCode = await fetchProviderDirectMailboxCode(mailboxContext);
      if (
        providerCode?.code &&
        (!lastSeenMarker || Number(providerCode.marker || 0) > lastSeenMarker)
      ) {
        return providerCode.code;
      }
    } catch (error) {
      lastError ??= error;
    }
    await sleep(4_000);
  }
  throw createWorkerError(
    504,
    "chatgpt_web_mailbox_code_timeout",
    lastError
      ? `Timed out waiting for ChatGPT/OpenAI email verification code. err=${String(lastError)}`
      : "Timed out waiting for ChatGPT/OpenAI email verification code.",
  );
}

async function fetchMailboxCodePayload(mailboxContext) {
  const response = await fetch(
    `${mailboxContext.serviceBaseUrl.replace(/\/+$/, "")}/mail/mailboxes/${encodeURIComponent(mailboxContext.mailboxSessionId)}/code`,
    {
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${mailboxContext.apiKey}`,
      },
    },
  );
  if (!response.ok) {
    throw new Error(`mailbox_code_status_${response.status}`);
  }
  return response.json().catch(() => ({}));
}

async function fetchMailboxSnapshot(mailboxContext, options = {}) {
  const root = await fetchMailboxSnapshotRoot(mailboxContext);
  const candidateSessionIds = resolveMailboxSnapshotSessionIds(root, mailboxContext);
  const messages = Array.isArray(root?.messages) ? root.messages : [];
  let bestCode = null;
  let bestMarker = 0;
  for (const message of messages) {
    if (!message || typeof message !== "object") {
      continue;
    }
    const messageSessionId = String(message.sessionId ?? "").trim();
    if (!candidateSessionIds.has(messageSessionId)) {
      continue;
    }
    const marker = Math.max(
      parseMailTimestamp(message.observedAt),
      parseMailTimestamp(message.receivedAt),
    );
    if (marker <= 0 || marker < bestMarker) {
      continue;
    }
    if (options.markerOnly === true) {
      bestMarker = marker;
      continue;
    }
    const code = extractOpenAiCodeFromMessage(message);
    if (code) {
      bestCode = code;
      bestMarker = marker;
    }
  }
  if (bestCode) {
    return { code: bestCode, marker: bestMarker };
  }
  if (options.markerOnly === true && bestMarker > 0) {
    return { marker: bestMarker };
  }
  return null;
}

function resolveMailboxSnapshotSessionIds(root, mailboxContext) {
  const ids = new Set();
  const expectedSessionId = normalizeString(mailboxContext?.mailboxSessionId);
  if (expectedSessionId) {
    ids.add(expectedSessionId);
  }
  const emailCandidates = dedupeStrings([
    mailboxContext?.email,
    mailboxContext?.mailboxAddress,
  ]).map((value) => value.toLowerCase());
  if (emailCandidates.length === 0) {
    return ids;
  }
  const expectedProviderType = normalizeString(mailboxContext?.mailboxProviderKey)?.toLowerCase();
  const expectedInstanceId = normalizeString(mailboxContext?.mailboxProviderInstanceId);
  const sessions = Array.isArray(root?.sessions) ? root.sessions : [];
  for (const session of sessions) {
    if (!session || typeof session !== "object") {
      continue;
    }
    const emailAddress = normalizeString(session.emailAddress)?.toLowerCase();
    if (!emailAddress || !emailCandidates.includes(emailAddress)) {
      continue;
    }
    const providerTypeKey = normalizeString(session.providerTypeKey)?.toLowerCase();
    if (expectedProviderType && providerTypeKey && providerTypeKey !== expectedProviderType) {
      continue;
    }
    const providerInstanceId = normalizeString(session.providerInstanceId);
    if (expectedInstanceId && providerInstanceId && providerInstanceId !== expectedInstanceId) {
      continue;
    }
    const sessionId = normalizeString(session.id);
    if (sessionId) {
      ids.add(sessionId);
    }
  }
  return ids;
}

async function fetchProviderDirectMailboxCode(mailboxContext) {
  const providerKey = normalizeString(mailboxContext?.mailboxProviderKey)?.toLowerCase();
  if (providerKey !== "im215") {
    return null;
  }
  const address = (
    normalizeString(mailboxContext?.email) ?? normalizeString(mailboxContext?.mailboxAddress)
  )?.toLowerCase();
  if (!address || !address.includes("@")) {
    return null;
  }
  const root = await fetchMailboxSnapshotRoot(mailboxContext);
  const config = resolveIm215ConfigFromSnapshot(root, mailboxContext);
  if (!config?.apiKey || !config?.baseUrl) {
    return null;
  }
  return fetchIm215DirectMailboxCode(config, address);
}

async function fetchMailboxSnapshotRoot(mailboxContext) {
  const response = await fetch(
    `${mailboxContext.serviceBaseUrl.replace(/\/+$/, "")}/mail/snapshot`,
    {
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${mailboxContext.apiKey}`,
      },
    },
  );
  if (!response.ok) {
    throw new Error(`mailbox_snapshot_status_${response.status}`);
  }
  const payload = await response.json().catch(() => ({}));
  return payload?.snapshot && typeof payload.snapshot === "object"
    ? payload.snapshot
    : payload?.result && typeof payload.result === "object"
      ? payload.result
      : payload;
}

function resolveIm215ConfigFromSnapshot(root, mailboxContext) {
  const expectedInstanceId =
    normalizeString(mailboxContext?.mailboxProviderInstanceId) ??
    normalizeString(decodeMailboxProviderInstanceId(mailboxContext?.mailboxRef));
  if (!expectedInstanceId) {
    return null;
  }
  const instances = Array.isArray(root?.instances) ? root.instances : [];
  const instance = instances.find(
    (item) => normalizeString(item?.id) === expectedInstanceId,
  );
  if (!instance || typeof instance !== "object") {
    return null;
  }
  const metadata =
    instance.metadata && typeof instance.metadata === "object" ? instance.metadata : {};
  const baseUrl =
    normalizeString(metadata.apiBase) ??
    normalizeString(metadata.baseUrl) ??
    "https://maliapi.215.im/v1";
  const directApiKey = normalizeString(metadata.apiKey);
  if (directApiKey) {
    return { baseUrl, apiKey: directApiKey };
  }
  const credentialSetsJson = normalizeString(metadata.credentialSetsJson);
  if (!credentialSetsJson) {
    return null;
  }
  try {
    const sets = JSON.parse(credentialSetsJson);
    if (!Array.isArray(sets)) {
      return null;
    }
    for (const set of sets) {
      const items = Array.isArray(set?.items) ? set.items : [];
      for (const item of items) {
        const value = normalizeString(item?.value);
        if (value) {
          return { baseUrl, apiKey: value };
        }
      }
    }
  } catch {
    return null;
  }
  return null;
}

async function fetchIm215DirectMailboxCode(config, address) {
  const listResult = await im215Request(config, "GET", "/messages", {
    query: { address },
  });
  if (listResult.status === 404) {
    return null;
  }
  if (listResult.status !== 200) {
    throw new Error(`im215_list_status_${listResult.status}`);
  }
  const rows = extractIm215MessageList(listResult.data);
  let bestCode = null;
  let bestMarker = 0;
  for (const row of rows) {
    const record = readValueRecord(row);
    const sender = readIm215MessageSender(record) || "";
    const subject = readIm215MessageSubject(record) || "";
    const summaryText = readIm215MessageText(record) || "";
    const summaryHtml = readIm215MessageHtml(record) || "";
    const summaryCode = extractOpenAiCodeFromMessage({
      sender,
      subject,
      textBody: summaryText,
      htmlBody: summaryHtml,
    });
    const observedAt = readIm215ObservedAt(record) || new Date().toISOString();
    const marker = parseMailTimestamp(observedAt);
    if (summaryCode && marker >= bestMarker) {
      bestCode = summaryCode;
      bestMarker = marker;
      continue;
    }
    const messageId = readIm215MessageId(record);
    if (!messageId) {
      continue;
    }
    const detailResult = await im215Request(
      config,
      "GET",
      `/messages/${encodeURIComponent(messageId)}`,
      { query: { address } },
    );
    if (detailResult.status !== 200 && detailResult.status !== 404) {
      throw new Error(`im215_detail_status_${detailResult.status}`);
    }
    const detail = detailResult.status === 200 ? unwrapIm215MessageRecord(detailResult.data) : record;
    const code = extractOpenAiCodeFromMessage({
      sender: readIm215MessageSender(detail) || sender,
      subject: readIm215MessageSubject(detail) || subject,
      textBody: readIm215MessageText(detail) || summaryText,
      htmlBody: readIm215MessageHtml(detail) || summaryHtml,
    });
    const detailMarker = parseMailTimestamp(readIm215ObservedAt(detail) || observedAt);
    if (code && detailMarker >= bestMarker) {
      bestCode = code;
      bestMarker = detailMarker;
    }
  }
  return bestCode ? { code: bestCode, marker: bestMarker } : null;
}

async function im215Request(config, method, requestPath, options = {}) {
  const baseUrl = normalizeBaseUrl(config.baseUrl);
  const url = new URL(
    requestPath.replace(/^\//, ""),
    baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`,
  );
  for (const [key, value] of Object.entries(options.query || {})) {
    if (value !== undefined && value !== null && String(value) !== "") {
      url.searchParams.set(key, String(value));
    }
  }
  const headers = {
    Accept: "application/json",
    ...(options.body ? { "Content-Type": "application/json" } : {}),
  };
  const apiKey = String(config.apiKey || "").trim();
  if (apiKey) {
    if (/^AC-/i.test(apiKey)) {
      headers["X-API-Key"] = apiKey;
    } else {
      headers.Authorization = `Bearer ${apiKey}`;
    }
  }
  const response = await fetch(url.toString(), {
    method,
    headers,
    body: options.body ? JSON.stringify(options.body) : undefined,
  });
  const text = await response.text().catch(() => "");
  let data = text;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = text;
    }
  }
  return { status: response.status, data };
}

function extractIm215MessageList(body) {
  const record = readValueRecord(body);
  const sources = [
    body,
    record.data,
    record.items,
    record.messages,
    record.list,
    readValueRecord(record.data).items,
    readValueRecord(record.data).messages,
    readValueRecord(record.result).items,
    readValueRecord(record.result).messages,
  ];
  for (const source of sources) {
    const items = readValueRecordList(source);
    if (items.length > 0) {
      return items;
    }
  }
  return [];
}

function unwrapIm215MessageRecord(body) {
  const root = readValueRecord(body);
  const nested = [
    readValueRecord(root.data),
    readValueRecord(root.result),
    readValueRecord(root.message),
  ].find((item) => Object.keys(item).length);
  return Object.keys(nested || {}).length ? nested : root;
}

function readIm215MessageId(record) {
  return (
    readValueString(record.id) ||
    readValueString(record.messageId) ||
    readValueString(record.mailId) ||
    readValueString(record.uuid) ||
    readValueString(record._id) ||
    ""
  );
}

function readIm215ObservedAt(record) {
  return (
    readValueString(record.receivedAt) ||
    readValueString(record.createdAt) ||
    readValueString(record.updatedAt) ||
    readValueString(record.timestamp) ||
    ""
  );
}

function readIm215MessageSubject(record) {
  return readValueString(record.subject) || readValueString(record.title) || "";
}

function readIm215MessageSender(record) {
  return (
    readSender(record.from) ||
    readSender(record.sender) ||
    readValueString(record.from_address) ||
    readValueString(record.mailFrom) ||
    ""
  );
}

function readIm215MessageText(record) {
  return (
    readValueString(record.text) ||
    readValueString(record.textBody) ||
    readValueString(record.body) ||
    readValueString(record.content) ||
    readValueString(record.snippet) ||
    readValueString(record.preview) ||
    readValueString(record.intro) ||
    ""
  );
}

function readIm215MessageHtml(record) {
  return (
    readValueString(record.html) ||
    readValueString(record.htmlBody) ||
    readValueString(record.html_content) ||
    readValueString(record.raw_content) ||
    ""
  );
}

function readValueRecord(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

function readValueRecordList(value) {
  return Array.isArray(value)
    ? value.filter((item) => item && typeof item === "object" && !Array.isArray(item))
    : [];
}

function readValueString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : "";
}

function readSender(value) {
  if (typeof value === "string" && value.trim()) {
    return value.trim();
  }
  if (!value || typeof value !== "object") {
    return "";
  }
  return (
    readValueString(value.address) ||
    readValueString(value.email) ||
    readValueString(value.name) ||
    ""
  );
}

function extractMailboxCodeMarker(codeObject) {
  if (!codeObject || typeof codeObject !== "object") {
    return 0;
  }
  return Math.max(
    parseMailTimestamp(codeObject.observedAt),
    parseMailTimestamp(codeObject.receivedAt),
    Number(codeObject.messageId ?? 0) || 0,
  );
}

function selectOpenAiVerificationCode(codeObject) {
  if (!codeObject || typeof codeObject !== "object") {
    return "";
  }
  const direct = extractSixDigitCode(codeObject.extractedCode ?? codeObject.code);
  if (direct) {
    return direct;
  }
  if (Array.isArray(codeObject.extractedCandidates)) {
    for (const item of codeObject.extractedCandidates) {
      const candidate = extractSixDigitCode(item);
      if (candidate) {
        return candidate;
      }
    }
  }
  return extractOpenAiCodeFromMessage(codeObject);
}

function extractOpenAiCodeFromMessage(message) {
  if (!message || typeof message !== "object") {
    return "";
  }
  for (const key of ["subject", "textBody", "htmlBody"]) {
    let text = String(message[key] ?? "");
    if (key === "htmlBody") {
      text = text.replace(/<[^>]+>/g, " ");
    }
    text = text.replace(/https?:\/\/\S+/gi, " ");
    text = text.replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/gi, " ");
    text = text.replace(/\s+/g, " ").trim();
    const contextual = text.match(
      /(?:verification\s*code|verify\s*code|security\s*code|one[-\s]*time\s*(?:pass)?code|login\s*code|sign[\s-]*in\s*code|confirmation\s*code|email\s*code|otp|passcode|验证码|校验码|动态码|动态密码|口令|代码为|代码是|enter\s+this\s+temporary\s+verification\s+code)[^0-9]{0,80}(\d{6})(?!\d)/i,
    );
    if (contextual?.[1]) {
      return contextual[1];
    }
  }
  for (const key of ["subject", "textBody", "htmlBody"]) {
    let text = String(message[key] ?? "");
    if (key === "htmlBody") {
      text = text.replace(/<[^>]+>/g, " ");
    }
    const candidate = extractSixDigitCode(text);
    if (candidate) {
      return candidate;
    }
  }
  return "";
}

function extractSixDigitCode(value) {
  const match = String(value ?? "").match(/(?<!\d)(\d{6})(?!\d)/);
  return match?.[1] ?? "";
}

function parseMailTimestamp(value) {
  const text = normalizeString(value);
  if (!text) {
    return 0;
  }
  const epoch = Date.parse(String(text).replace("Z", "+00:00"));
  return Number.isFinite(epoch) ? epoch : 0;
}

function sleep(timeoutMs) {
  return new Promise((resolve) => setTimeout(resolve, timeoutMs));
}

async function collectCookieHeader(context, baseUrl) {
  const cookies = await context.cookies([baseUrl]).catch(() => []);
  if (!Array.isArray(cookies) || cookies.length === 0) {
    return null;
  }
  return cookies
    .filter((cookie) => normalizeString(cookie?.name))
    .map((cookie) => `${cookie.name}=${cookie.value ?? ""}`)
    .join("; ");
}

function extractBootstrapArtifacts(html) {
  const text = String(html ?? "");
  const accessToken =
    firstRegexCapture(text, /"accessToken"\s*:\s*"([^"]+)"/i) ??
    firstRegexCapture(text, /\\"accessToken\\"\s*:\s*\\"([^\\]+)\\"/i);
  const sessionId =
    firstRegexCapture(text, /"session_id"\s*:\s*"([^"]+)"/i) ??
    firstRegexCapture(text, /\\"session_id\\"\s*:\s*\\"([^\\]+)\\"/i);
  const deviceId =
    firstRegexCapture(text, /"deviceId"\s*:\s*"([0-9a-f-]{20,})"/i) ??
    firstRegexCapture(text, /"oai-did"\s*[:=]\s*"([0-9a-f-]{20,})"/i);
  const clientBuildNumber =
    firstRegexCapture(text, /"clientBuildNumber"\s*:\s*"([^"]+)"/i) ??
    firstRegexCapture(text, /\\"clientBuildNumber\\"\s*:\s*\\"([^\\]+)\\"/i);
  const powDataBuild =
    firstRegexCapture(text, /data-build="([^"]+)"/i) ??
    firstRegexCapture(text, /"buildId"\s*:\s*"([^"]+)"/i);
  const powSources = Array.from(
    text.matchAll(/https:\/\/chatgpt\.com\/backend-api\/sentinel\/[^"'\\\s<]+/gi),
    (match) => normalizeString(match[0]),
  ).filter(Boolean);
  return {
    accessToken: normalizeString(accessToken),
    sessionId: normalizeString(sessionId),
    deviceId: normalizeString(deviceId),
    clientBuildNumber: normalizeString(clientBuildNumber),
    powDataBuild: normalizeString(powDataBuild),
    powSources: dedupeStrings(powSources),
  };
}

async function runChatGptBrowserRelay(page, input) {
  const html = await page.content().catch(() => "");
  const bootstrap = mergeChatGptPowBootstrap(
    extractChatGptPowBootstrapFromHtml(html),
    input,
  );
  const legacyToken = buildLegacyRequirementsToken(bootstrap, input.userAgent);
  const authHeaderValue = normalizeString(input.accessToken);
  const requirementsHeaders = {
    Accept: "application/json",
    "Accept-Language": input.acceptLanguage,
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    Pragma: "no-cache",
    Priority: "u=1, i",
    "OAI-Client-Build-Number": input.clientBuildNumber,
    "OAI-Client-Version": input.clientVersion,
    "OAI-Device-Id": input.deviceId,
    "OAI-Language": input.languageCode,
    "OAI-Session-Id": input.sessionId,
    "X-OpenAI-Target-Path": CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
    "X-OpenAI-Target-Route": CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
  };
  if (authHeaderValue) {
    requirementsHeaders.Authorization = `Bearer ${authHeaderValue}`;
  }
  const requirementsResponse = await page.evaluate(
    async ({ url, headers, body }) => {
      const response = await fetch(url, {
        method: "POST",
        credentials: "include",
        headers,
        body: JSON.stringify(body),
      });
      const text = await response.text();
      return {
        status: response.status,
        contentType: response.headers.get("content-type"),
        bodyText: text,
      };
    },
    {
      url: `${input.baseUrl}${CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH}`,
      headers: requirementsHeaders,
      body: { p: legacyToken },
    },
  );
  const requirementsValue = safeParseJson(requirementsResponse.bodyText);
  let proofToken = null;
  if (
    requirementsValue?.proofofwork?.required === true &&
    requirementsValue?.proofofwork?.seed &&
    requirementsValue?.proofofwork?.difficulty
  ) {
    proofToken = buildProofToken(
      bootstrap,
      input.userAgent,
      requirementsValue.proofofwork.seed,
      requirementsValue.proofofwork.difficulty,
    );
  }
  const conversationHeaders = {
    Accept: "text/event-stream",
    "Accept-Language": input.acceptLanguage,
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    Pragma: "no-cache",
    Priority: "u=1, i",
    "OAI-Client-Build-Number": input.clientBuildNumber,
    "OAI-Client-Version": input.clientVersion,
    "OAI-Device-Id": input.deviceId,
    "OAI-Language": input.languageCode,
    "OAI-Session-Id": input.sessionId,
    "OpenAI-Sentinel-Chat-Requirements-Token": normalizeString(requirementsValue?.token) ?? "",
    "X-OpenAI-Target-Path": CHATGPT_WEB_DEFAULT_CONVERSATION_PATH,
    "X-OpenAI-Target-Route": CHATGPT_WEB_DEFAULT_CONVERSATION_PATH,
  };
  if (authHeaderValue) {
    conversationHeaders.Authorization = `Bearer ${authHeaderValue}`;
  }
  if (proofToken) {
    conversationHeaders["OpenAI-Sentinel-Proof-Token"] = proofToken;
  }
  if (normalizeString(requirementsValue?.turnstile?.token)) {
    conversationHeaders["OpenAI-Sentinel-Turnstile-Token"] = normalizeString(
      requirementsValue.turnstile.token,
    );
  }
  if (normalizeString(requirementsValue?.so_token)) {
    conversationHeaders["OpenAI-Sentinel-SO-Token"] = normalizeString(
      requirementsValue.so_token,
    );
  }
  const relayResponse = await page.evaluate(
    async ({ url, headers, body }) => {
      const response = await fetch(url, {
        method: "POST",
        credentials: "include",
        headers,
        body: JSON.stringify(body),
      });
      const text = await response.text();
      return {
        status: response.status,
        contentType: response.headers.get("content-type"),
        bodyText: text,
      };
    },
    {
      url: `${input.baseUrl}${CHATGPT_WEB_DEFAULT_CONVERSATION_PATH}`,
      headers: conversationHeaders,
      body: input.requestBody,
    },
  );
  const result = {
    requirementsStatus: requirementsResponse.status,
    requirementsContentType: requirementsResponse.contentType,
    requirementsPreview: String(requirementsResponse.bodyText ?? "").slice(0, 500),
    status: relayResponse.status,
    contentType: relayResponse.contentType,
    bodyText: relayResponse.bodyText,
    bodyPreview: String(relayResponse.bodyText ?? "").slice(0, 1000),
  };
  if (shouldFallbackToUiRelay(result)) {
    return runChatGptBrowserUiRelay(page, input);
  }
  return result;
}

function shouldFallbackToUiRelay(relayResponse) {
  const requirementsStatus = Number(relayResponse?.requirementsStatus ?? 0);
  const relayStatus = Number(relayResponse?.status ?? 0);
  const preview = String(
    relayResponse?.bodyText ??
      relayResponse?.bodyPreview ??
      relayResponse?.requirementsPreview ??
      "",
  ).toLowerCase();
  if (!(requirementsStatus >= 200 && requirementsStatus < 300)) {
    return true;
  }
  if (!(relayStatus >= 200 && relayStatus < 300)) {
    return true;
  }
  return (
    preview.includes("unusual activity") ||
    preview.includes("unauthorized") ||
    preview.includes("internal server error") ||
    preview.includes("your session has ended") ||
    preview.includes("登录以继续") ||
    preview.includes("log in to continue") ||
    preview.includes("login_with") ||
    preview.includes("/api/auth/error") ||
    preview.includes("/auth/error") ||
    preview.includes("captcha") ||
    preview.includes("challenge")
  );
}

function extractRelayPrompt(requestBody) {
  const messages = Array.isArray(requestBody?.messages) ? requestBody.messages : [];
  const firstMessage = messages.find((item) => item?.author?.role === "user") ?? messages[0];
  const parts = firstMessage?.content?.parts;
  if (Array.isArray(parts)) {
    const text = parts
      .map((part) => (typeof part === "string" ? part.trim() : ""))
      .filter(Boolean)
      .join("\n");
    if (text) {
      return text;
    }
  }
  return null;
}

async function waitForVisibleChatGptEditor(page) {
  await page.waitForFunction(
    () => {
      const selectors = [
        "#prompt-textarea",
        'div.ProseMirror[contenteditable="true"]',
        '[contenteditable="true"]',
        "textarea",
      ];
      return selectors.some((selector) =>
        Array.from(document.querySelectorAll(selector)).some((node) => {
          const style = window.getComputedStyle(node);
          const rect = node.getBoundingClientRect();
          return (
            style.display !== "none" &&
            style.visibility !== "hidden" &&
            rect.width > 0 &&
            rect.height > 0
          );
        }),
      );
    },
    undefined,
    { timeout: 120_000 },
  );
  return page.locator(
    '#prompt-textarea:visible, div.ProseMirror[contenteditable="true"]:visible, [contenteditable="true"]:visible, textarea:visible',
  );
}

async function dismissChatGptBlockingOverlays(page) {
  await page.keyboard.press("Escape").catch(() => {});
  const selectors = [
    '#modal-onboarding button[aria-label*="Close"]',
    '#modal-onboarding button[aria-label*="关闭"]',
    '#modal-onboarding button:has-text("开始")',
    '#modal-onboarding button:has-text("继续")',
    '#modal-onboarding button:has-text("稍后")',
    '#modal-onboarding button:has-text("跳过")',
    '#modal-onboarding button:has-text("Done")',
    '#modal-onboarding button:has-text("Continue")',
    '#modal-onboarding button:has-text("Skip")',
    '#modal-onboarding button:has-text("Maybe later")',
    '[data-testid="modal-onboarding"] button[aria-label*="Close"]',
    '[data-testid="modal-onboarding"] button:has-text("开始")',
    '[data-testid="modal-onboarding"] button:has-text("继续")',
    '[data-testid="modal-onboarding"] button:has-text("稍后")',
    '[data-testid="modal-onboarding"] button:has-text("跳过")',
    '[data-testid="modal-onboarding"] button:has-text("Continue")',
    '[data-testid="modal-onboarding"] button:has-text("Skip")',
    '[data-testid="modal-onboarding"] button:has-text("Done")',
  ];
  for (const selector of selectors) {
    const button = page.locator(selector).first();
    if ((await button.count().catch(() => 0)) <= 0) {
      continue;
    }
    await button.click({ timeout: 2_000, force: true }).catch(() => {});
    await page.waitForTimeout(500);
  }
  await page
    .evaluate(() => {
      const overlays = Array.from(
        document.querySelectorAll('#modal-onboarding, [data-testid="modal-onboarding"]'),
      );
      for (const overlay of overlays) {
        overlay.remove();
      }
    })
    .catch(() => {});
}

async function collectChatGptAssistantTexts(page) {
  return page.evaluate(() => {
    const selectors = [
      '[data-message-author-role="assistant"]',
      'article [data-message-author-role="assistant"]',
      '[data-testid^="conversation-turn-"] [data-message-author-role="assistant"]',
    ];
    const values = [];
    for (const selector of selectors) {
      for (const node of document.querySelectorAll(selector)) {
        const text = String(node.innerText || node.textContent || "").trim();
        if (text) {
          values.push(text);
        }
      }
    }
    return Array.from(new Set(values));
  });
}

async function recoverChatGptUiRelayAuthIfNeeded(page, relayInput) {
  const pageState = await collectPageState(page).catch(() => null);
  if (!isRelayAuthRecoveryState(pageState)) {
    return false;
  }
  if (!relayInput?.authSeed?.email || !relayInput?.authSeed?.password) {
    return false;
  }
  await loginExistingAccount(
    page,
    relayInput.baseUrl,
    relayInput.authUrl,
    relayInput.authSeed,
    relayInput.timeoutMs ?? 120_000,
    () => {},
    relayInput.mailboxContext ?? null,
  );
  await navigateWithChallengeSettle(
    page,
    `${relayInput.baseUrl}/`,
    relayInput.timeoutMs ?? 120_000,
  );
  await page.waitForTimeout(3_000);
  await dismissChatGptBlockingOverlays(page);
  return true;
}

async function prepareChatGptUiRelayPage(page, relayInput) {
  await page.bringToFront().catch(() => {});
  await page.waitForLoadState("domcontentloaded").catch(() => {});
  await page.waitForTimeout(2_000);
  await dismissChatGptBlockingOverlays(page);
  await recoverChatGptUiRelayAuthIfNeeded(page, relayInput);
  await dismissChatGptBlockingOverlays(page);
}

async function submitChatGptUiPrompt(page, prompt) {
  await dismissChatGptBlockingOverlays(page);
  const editor = await waitForVisibleChatGptEditor(page);
  const editorTag = await editor
    .first()
    .evaluate((node) => ({
      tagName: node.tagName,
      contentEditable: node.getAttribute("contenteditable"),
    }));
  if (
    String(editorTag?.tagName || "").toLowerCase() === "textarea" ||
    String(editorTag?.contentEditable || "").toLowerCase() !== "true"
  ) {
    await editor.first().fill(prompt);
  } else {
    await editor.first().click({ force: true });
    await page.keyboard.press("Control+A").catch(() => {});
    await page.keyboard.type(prompt, { delay: 5 });
  }
  const sendButton = page.locator(
    'button[data-testid="send-button"]:visible, button[aria-label*="Send"]:visible, button[aria-label*="发送"]:visible, button[aria-label*="送信"]:visible',
  );
  if (
    (await sendButton.count().catch(() => 0)) > 0 &&
    (await sendButton.first().isEnabled().catch(() => false))
  ) {
    await sendButton.first().click();
  } else {
    await editor.first().press("Enter");
  }
  await page.waitForLoadState("domcontentloaded").catch(() => {});
}

async function runChatGptBrowserUiRelay(page, relayInput) {
  const prompt = extractRelayPrompt(relayInput?.requestBody);
  if (!prompt) {
    return {
      requirementsStatus: 200,
      requirementsContentType: "application/json",
      requirementsPreview: "{\"detail\":\"ui relay skipped: missing prompt\"}",
      status: 500,
      contentType: "application/json",
      bodyText: "{\"detail\":\"ui relay missing prompt\"}",
      bodyPreview: "{\"detail\":\"ui relay missing prompt\"}",
    };
  }

  await prepareChatGptUiRelayPage(page, relayInput);
  const beforeAssistantTexts = await collectChatGptAssistantTexts(page);
  const uiRequestCapture = createChatGptUiRequestCapture(page);

  await submitChatGptUiPrompt(page, prompt);

  const startedAt = Date.now();
  let stableCount = 0;
  let lastText = "";
  let recoveryAttempted = false;
  while (Date.now() - startedAt < 90_000) {
    await page.waitForTimeout(1_500);
    if (!recoveryAttempted) {
      const pageState = await collectPageState(page).catch(() => null);
      const authPage = isRelayAuthRecoveryState(pageState);
      if (
        authPage &&
        relayInput?.authSeed?.email &&
        relayInput?.authSeed?.password
      ) {
        recoveryAttempted = true;
        await recoverChatGptUiRelayAuthIfNeeded(page, relayInput);
        await submitChatGptUiPrompt(page, prompt);
        continue;
      }
    }
    let snapshot;
    try {
      snapshot = await page.evaluate(() => {
        const stopVisible = Array.from(document.querySelectorAll("button"))
        .some((node) => {
          const label = String(
            node.getAttribute("aria-label") ||
              node.textContent ||
              "",
          ).toLowerCase();
          return label.includes("stop") || label.includes("停止");
        });
        return {
          stopVisible,
        };
      });
      snapshot.assistantText = selectNewAssistantTextFromValues(
        await collectChatGptAssistantTexts(page),
        beforeAssistantTexts,
      );
    } catch (error) {
      if (String(error?.message || error).toLowerCase().includes("execution context was destroyed")) {
        await page.waitForLoadState("domcontentloaded").catch(() => {});
        continue;
      }
      throw error;
    }

    const candidate = normalizeString(snapshot.assistantText) ?? "";
    if (candidate && candidate === lastText && !snapshot.stopVisible) {
      stableCount += 1;
    } else if (candidate) {
      stableCount = 1;
      lastText = candidate;
    }
    if (candidate && stableCount >= 2) {
      const syntheticSse = `data: ${candidate}\n\ndata: [DONE]\n\n`;
      return finalizeChatGptUiRelayCapture(uiRequestCapture, {
        requirementsStatus: 200,
        requirementsContentType: "application/json",
        requirementsPreview: "{\"detail\":\"ui relay fallback used\"}",
        status: 200,
        contentType: "text/event-stream",
        bodyText: syntheticSse,
        bodyPreview: syntheticSse.slice(0, 1000),
      });
    }
  }

  const timeoutState = await page
    .evaluate(() => ({
      href: location.href,
      title: document.title,
      bodyPreview: String(document.body?.innerText || "").slice(0, 2000),
    }))
    .catch(() => ({
      href: null,
      title: null,
      bodyPreview: "",
    }));

  return finalizeChatGptUiRelayCapture(uiRequestCapture, {
    requirementsStatus: 200,
    requirementsContentType: "application/json",
    requirementsPreview: "{\"detail\":\"ui relay timeout\"}",
    status: 504,
    contentType: "application/json",
    bodyText: JSON.stringify({
      detail: "ui relay timed out waiting for assistant output",
      state: timeoutState,
    }),
    bodyPreview: JSON.stringify({
      detail: "ui relay timed out waiting for assistant output",
      state: timeoutState,
    }).slice(0, 1000),
  });
}

function createChatGptUiRequestCapture(page) {
  const records = [];
  const rawHeaders = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_HEADERS,
    false,
  );
  const rawResponses = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_RESPONSES,
    false,
  );
  const captureAllPosts = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_ALL_POSTS,
    false,
  );
  const recordByRequest = new Map();
  const onRequest = (request) => {
    try {
      const url = request.url();
      const matchesTarget =
        captureAllPosts ||
        url.includes("/backend-api/conversation") ||
        url.includes("/backend-api/f/conversation") ||
        url.includes("/ces/");
      if (
        request.method().toUpperCase() !== "POST" ||
        !matchesTarget
      ) {
        return;
      }
      const postData = request.postData();
      const record = {
        url,
        method: request.method(),
        headers: rawHeaders ? request.headers() : redactCapturedRequestHeaders(request.headers()),
        postDataJson: safeParseJson(postData),
        postDataPreview: String(postData ?? "").slice(0, 4000),
      };
      records.push(record);
      recordByRequest.set(request, record);
    } catch {
      // Capture is diagnostic-only; do not break the live UI relay.
    }
  };
  const onResponse = async (response) => {
    try {
      const record = recordByRequest.get(response.request());
      if (!record) {
        return;
      }
      const headers = response.headers();
      const bodyText = await response.text().catch(() => null);
      record.response = {
        status: response.status(),
        headers: rawHeaders ? headers : redactCapturedRequestHeaders(headers),
        bodyJson: rawResponses ? safeParseJson(bodyText) : null,
        bodyPreview: rawResponses ? String(bodyText ?? "").slice(0, 4000) : null,
        bodyLength: String(bodyText ?? "").length,
      };
    } catch {
      // Capture is diagnostic-only; do not break the live UI relay.
    }
  };
  page.on("request", onRequest);
  page.on("response", onResponse);
  return { page, onRequest, onResponse, records, rawHeaders, rawResponses };
}

async function finalizeChatGptUiRelayCapture(capture, result) {
  if (!capture) {
    return result;
  }
  capture.page.off("request", capture.onRequest);
  capture.page.off("response", capture.onResponse);
  const summary = {
    capturedAt: new Date().toISOString(),
    rawHeaders: capture.rawHeaders === true,
    rawResponses: capture.rawResponses === true,
    requests: capture.records,
  };
  const targetPath = normalizeString(process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_PATH);
  if (targetPath) {
    await writeFile(targetPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8").catch(() => {});
  }
  return {
    ...result,
    uiRequestCapture: summary,
  };
}

function redactCapturedRequestHeaders(headers) {
  const result = {};
  for (const [name, value] of Object.entries(headers ?? {})) {
    const lower = name.toLowerCase();
    if (lower === "authorization") {
      result[name] = { redacted: true, kind: "authorization", length: String(value ?? "").length };
    } else if (lower === "cookie") {
      result[name] = {
        redacted: true,
        kind: "cookie",
        length: String(value ?? "").length,
        cookieNames: parseCookieNames(value),
      };
    } else if (lower === "x-oai-is") {
      result[name] = { redacted: true, kind: "x-oai-is", length: String(value ?? "").length };
    } else if (lower.includes("token")) {
      result[name] = { redacted: true, kind: "token", length: String(value ?? "").length };
    } else {
      result[name] = value;
    }
  }
  return result;
}

function parseCookieNames(cookieHeader) {
  return String(cookieHeader ?? "")
    .split(";")
    .map((entry) => entry.trim().split("=", 1)[0])
    .filter(Boolean);
}

function extractChatGptPowBootstrapFromHtml(html) {
  const text = String(html ?? "");
  const scriptMatches = Array.from(
    text.matchAll(/<script[^>]+src=["']([^"']+)["']/gi),
    (match) => normalizeString(match[1]),
  ).filter(Boolean);
  const buildMatch =
    text.match(/c\/[^/]*\/_/i)?.[0] ??
    text.match(/<html[^>]*data-build=["']([^"']*)["']/i)?.[1] ??
    "";
  return {
    powScriptSources: scriptMatches.length > 0 ? scriptMatches : [DEFAULT_POW_SCRIPT],
    powDataBuild: normalizeString(buildMatch) ?? "",
  };
}

function mergeChatGptPowBootstrap(bootstrap, input) {
  const fallbackSources = Array.isArray(input?.chatgptPowSources)
    ? input.chatgptPowSources
        .map((value) => normalizeString(value))
        .filter(Boolean)
    : [];
  const mergedSources = dedupeStrings([
    ...(Array.isArray(bootstrap?.powScriptSources) ? bootstrap.powScriptSources : []),
    ...fallbackSources,
    DEFAULT_POW_SCRIPT,
  ]);
  return {
    powScriptSources: mergedSources.length > 0 ? mergedSources : [DEFAULT_POW_SCRIPT],
    powDataBuild:
      normalizeString(bootstrap?.powDataBuild) ??
      normalizeString(input?.chatgptPowDataBuild) ??
      "",
  };
}

function buildLegacyRequirementsToken(bootstrap, userAgent) {
  const seed = Math.random().toFixed(15);
  const config = buildPowConfig(bootstrap, userAgent);
  const [answer] = powGenerate(seed, "0fffff", config, 500000);
  return `gAAAAAC${answer}`;
}

function buildProofToken(bootstrap, userAgent, seed, difficulty) {
  const config = buildPowConfig(bootstrap, userAgent);
  const [answer, solved] = powGenerate(seed, difficulty, config, 500000);
  if (!solved) {
    return null;
  }
  return `gAAAAAB${answer}`;
}

function buildPowConfig(bootstrap, userAgent) {
  const scriptSources =
    Array.isArray(bootstrap?.powScriptSources) && bootstrap.powScriptSources.length > 0
      ? bootstrap.powScriptSources
      : [DEFAULT_POW_SCRIPT];
  const navigatorKeys = [
    "registerProtocolHandler−function registerProtocolHandler() { [native code] }",
    "storage−[object StorageManager]",
    "locks−[object LockManager]",
    "appCodeName−Mozilla",
    "permissions−[object Permissions]",
    "share−function share() { [native code] }",
    "webdriver−false",
    "managed−[object NavigatorManagedData]",
    "canShare−function canShare() { [native code] }",
    "vendor−Google Inc.",
  ];
  const windowKeys = [
    "window",
    "self",
    "document",
    "location",
    "navigator",
    "indexedDB",
    "sessionStorage",
    "localStorage",
    "__NEXT_DATA__",
  ];
  const documentKeys = ["_reactListeningo743lnnpvdg", "location"];
  const cores = [8, 16, 24, 32];
  return [
    3000,
    "Mon Jan 02 2006 15:04:05 GMT-0500 (Eastern Standard Time)",
    4294705152,
    0,
    userAgent,
    pickRandom(scriptSources) ?? DEFAULT_POW_SCRIPT,
    normalizeString(bootstrap?.powDataBuild) ?? "",
    "en-US",
    "en-US,es-US,en,es",
    0,
    pickRandom(navigatorKeys) ?? navigatorKeys[0],
    pickRandom(documentKeys) ?? documentKeys[0],
    pickRandom(windowKeys) ?? windowKeys[0],
    Date.now(),
    randomUUID(),
    "",
    pickRandom(cores) ?? 8,
    0,
  ];
}

function powGenerate(seed, difficulty, config, limit) {
  const target = Buffer.from(String(difficulty || ""), "hex");
  const diffLen = Math.floor(String(difficulty || "").length / 2);
  const part1 = joinJsonSlice(config.slice(0, 3), true, true);
  const part2 = joinJsonSlice(config.slice(4, 9), false, false);
  const part3 = joinJsonSlice(config.slice(10), false, true);
  for (let i = 0; i < limit; i += 1) {
    const finalJson = `${part1}${i},${part2}${i >> 1}${part3}`;
    const encoded = Buffer.from(finalJson, "utf8").toString("base64");
    const hash = createHash("sha3-512")
      .update(String(seed || ""), "utf8")
      .update(encoded, "utf8")
      .digest();
    if (diffLen > 0 && Buffer.compare(hash.subarray(0, diffLen), target.subarray(0, diffLen)) <= 0) {
      return [encoded, true];
    }
  }
  return [
    `wQ8Lk5FbGpA2NcR9dShT6gYjU7VxZ4D${Buffer.from(`\"${seed}\"`, "utf8").toString("base64")}`,
    false,
  ];
}

function joinJsonSlice(items, dropLastBracket, dropFirstBracket) {
  let text = JSON.stringify(items ?? []);
  if (dropFirstBracket && text.startsWith("[")) {
    text = text.slice(1);
  }
  if (dropLastBracket && text.endsWith("]")) {
    text = text.slice(0, -1);
  }
  return text;
}

function pickRandom(values) {
  return Array.isArray(values) && values.length > 0
    ? values[Math.floor(Math.random() * values.length)]
    : null;
}

function safeParseJson(text) {
  try {
    return JSON.parse(String(text ?? ""));
  } catch {
    return null;
  }
}

function firstRegexCapture(text, pattern) {
  const match = pattern.exec(String(text ?? ""));
  return match?.[1] ?? null;
}

function detectBrowserChallenge(value) {
  const lower = String(value ?? "").toLowerCase();
  return (
    lower.includes("cloudflare") ||
    lower.includes("captcha") ||
    lower.includes("challenge") ||
    lower.includes("verify you are human") ||
    lower.includes("just a moment") ||
    lower.includes("attention required")
  );
}

function safePageUrl(page) {
  try {
    return normalizeString(page.url()) ?? "";
  } catch {
    return "";
  }
}

function findCookieValue(cookieHeader, name) {
  const target = String(name ?? "").trim();
  if (!cookieHeader || !target) {
    return null;
  }
  for (const entry of String(cookieHeader).split(";")) {
    const [cookieName, ...rest] = entry.split("=");
    if (cookieName?.trim() === target) {
      return rest.join("=").trim() || null;
    }
  }
  return null;
}

async function maybeWriteCredentialFile(input, result) {
  const shouldWrite =
    input?.writeCredentialFile === true ||
    typeof input?.credentialFilePath === "string" ||
    typeof input?.credentialRootDir === "string" ||
    parseBoolean(process.env.CHATGPT_WEB_WRITE_CREDENTIAL_FILE, false);
  if (!shouldWrite || !result?.ok) {
    return null;
  }

  const targetPath = resolveCredentialFilePath(input, result);
  await mkdir(path.dirname(targetPath), { recursive: true });
  const payload = buildCredentialPayload(result);
  await writeFile(targetPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  return targetPath;
}

function buildCredentialPayload(result) {
  const uiConversationMaterial = extractChatGptUiConversationMaterial(result);
  const payload = {
    adapter: "chatgpt_web_reverse_compatible",
    baseUrl: DEFAULT_BASE_URL,
    sessionAuth: {
      transport: "bearer",
      headerName: "authorization",
    },
    headers: {},
    extraBody: {},
  };
  if (normalizeString(result.authToken)) {
    payload.apiKey = result.authToken;
  }
  if (normalizeString(result.expiresAt)) {
    payload.expiresAt = result.expiresAt;
  }
  if (normalizeString(result.cookieHeader)) {
    payload.headers.Cookie = result.cookieHeader;
  }
  if (normalizeString(result.userAgent)) {
    payload.headers["User-Agent"] = result.userAgent;
    payload.extraBody.userAgent = result.userAgent;
  }
  if (normalizeString(result.language)) {
    payload.extraBody.language = result.language;
  }
  if (normalizeString(result.languageCode)) {
    payload.extraBody.languageCode = result.languageCode;
  }
  if (normalizeString(result.timezone)) {
    payload.extraBody.timezone = result.timezone;
  }
  if (normalizeString(result.clientVersion)) {
    payload.extraBody.clientVersion = result.clientVersion;
  }
  if (normalizeString(result.clientBuildNumber)) {
    payload.extraBody.clientBuildNumber = result.clientBuildNumber;
  }
  if (normalizeString(result.deviceId)) {
    payload.extraBody.deviceId = result.deviceId;
  }
  if (normalizeString(result.sessionId)) {
    payload.extraBody.sessionId = result.sessionId;
  }
  if (normalizeString(uiConversationMaterial.clientVersion)) {
    payload.extraBody.clientVersion = uiConversationMaterial.clientVersion;
  }
  if (normalizeString(uiConversationMaterial.clientBuildNumber)) {
    payload.extraBody.clientBuildNumber = uiConversationMaterial.clientBuildNumber;
  }
  if (normalizeString(uiConversationMaterial.deviceId)) {
    payload.extraBody.deviceId = uiConversationMaterial.deviceId;
  }
  if (normalizeString(uiConversationMaterial.sessionId)) {
    payload.extraBody.sessionId = uiConversationMaterial.sessionId;
  }
  if (normalizeString(uiConversationMaterial.xOaiIs)) {
    payload.headers["X-OAI-IS"] = uiConversationMaterial.xOaiIs;
  }
  if (normalizeString(uiConversationMaterial.xConduitToken)) {
    payload.headers["X-Conduit-Token"] = uiConversationMaterial.xConduitToken;
  }
  if (normalizeString(uiConversationMaterial.oaiTelemetry)) {
    payload.headers["OAI-Telemetry"] = uiConversationMaterial.oaiTelemetry;
  }
  if (normalizeString(uiConversationMaterial.oaiEchoLogs)) {
    payload.headers["OAI-Echo-Logs"] = uiConversationMaterial.oaiEchoLogs;
  }
  if (normalizeString(uiConversationMaterial.turnTraceId)) {
    payload.extraBody.chatgptWebTurnTraceId = uiConversationMaterial.turnTraceId;
  }
  if (normalizeString(uiConversationMaterial.sentinelChatRequirementsToken)) {
    payload.extraBody.chatgptWebSentinelChatRequirementsToken =
      uiConversationMaterial.sentinelChatRequirementsToken;
  }
  if (normalizeString(uiConversationMaterial.sentinelProofToken)) {
    payload.extraBody.chatgptWebSentinelProofToken = uiConversationMaterial.sentinelProofToken;
  }
  if (normalizeString(uiConversationMaterial.sentinelTurnstileToken)) {
    payload.extraBody.chatgptWebSentinelTurnstileToken =
      uiConversationMaterial.sentinelTurnstileToken;
  }
  if (normalizeString(uiConversationMaterial.sentinelSoToken)) {
    payload.extraBody.chatgptWebSentinelSoToken = uiConversationMaterial.sentinelSoToken;
  }
  if (Array.isArray(result.chatgptPowSources) && result.chatgptPowSources.length > 0) {
    payload.extraBody.chatgptPowSources = result.chatgptPowSources;
  }
  if (normalizeString(result.chatgptPowDataBuild)) {
    payload.extraBody.chatgptPowDataBuild = result.chatgptPowDataBuild;
  }
  if (normalizeString(result.accountName)) {
    payload.accountName = result.accountName;
  }
  if (normalizeString(result.credentialMaterialKey)) {
    payload.credentialMaterialKey = result.credentialMaterialKey;
  }
  payload.rawSource = {
    authProbe: result.authProbe ?? null,
    profileSource: result.profileSource ?? null,
    currentUrl: result.currentUrl ?? null,
    challengePresent: result.challengePresent === true,
    localStorageKeys: Array.isArray(result.localStorageKeys) ? result.localStorageKeys : [],
    bodyPreview: result.bodyPreview ?? null,
    uiConversationMaterial: {
      captured: uiConversationMaterial.captured === true,
      rawHeaders: uiConversationMaterial.rawHeaders === true,
      hasXOaiIs: Boolean(uiConversationMaterial.xOaiIs),
      hasXConduitToken: Boolean(uiConversationMaterial.xConduitToken),
      hasOaiTelemetry: Boolean(uiConversationMaterial.oaiTelemetry),
      hasOaiEchoLogs: Boolean(uiConversationMaterial.oaiEchoLogs),
      hasTurnTraceId: Boolean(uiConversationMaterial.turnTraceId),
      hasSentinelChatRequirementsToken: Boolean(
        uiConversationMaterial.sentinelChatRequirementsToken,
      ),
      hasSentinelProofToken: Boolean(uiConversationMaterial.sentinelProofToken),
      hasSentinelTurnstileToken: Boolean(uiConversationMaterial.sentinelTurnstileToken),
      hasSentinelSoToken: Boolean(uiConversationMaterial.sentinelSoToken),
    },
  };
  return payload;
}

function extractChatGptUiConversationMaterial(result) {
  const capture = result?.relayResponse?.uiRequestCapture;
  const requests = Array.isArray(capture?.requests) ? capture.requests : [];
  const request = [...requests]
    .reverse()
    .find((item) => {
      try {
        return new URL(item?.url).pathname === "/backend-api/f/conversation";
      } catch {
        return false;
      }
    });
  const headers = request?.headers && typeof request.headers === "object" ? request.headers : {};
  const readHeader = (name) => {
    const normalized = name.toLowerCase();
    const value = headers[normalized] ?? headers[name];
    return typeof value === "string" ? normalizeString(value) : null;
  };
  return {
    captured: Boolean(request),
    rawHeaders: capture?.rawHeaders === true,
    clientVersion: readHeader("oai-client-version"),
    clientBuildNumber: readHeader("oai-client-build-number"),
    deviceId: readHeader("oai-device-id"),
    sessionId: readHeader("oai-session-id"),
    xOaiIs: readHeader("x-oai-is"),
    xConduitToken: readHeader("x-conduit-token"),
    turnTraceId: readHeader("x-oai-turn-trace-id"),
    oaiTelemetry: readHeader("oai-telemetry"),
    oaiEchoLogs: readHeader("oai-echo-logs"),
    sentinelChatRequirementsToken: readHeader("openai-sentinel-chat-requirements-token"),
    sentinelProofToken: readHeader("openai-sentinel-proof-token"),
    sentinelTurnstileToken: readHeader("openai-sentinel-turnstile-token"),
    sentinelSoToken: readHeader("openai-sentinel-so-token"),
  };
}

function resolveCredentialFilePath(input, result) {
  const explicitPath = normalizeString(input?.credentialFilePath);
  if (explicitPath) {
    return ensureJsonExtension(explicitPath);
  }

  const rootDir =
    normalizeString(input?.credentialRootDir) ??
    normalizeString(process.env.NEURO_PROVIDER_CREDENTIAL_ROOT_DIR) ??
    path.join(os.homedir(), ".neuro");
  const familyDir =
    normalizeString(input?.credentialFamilyDir) ?? DEFAULT_CREDENTIAL_FAMILY_DIR;
  const explicitFileName = normalizeString(input?.credentialFileName);
  const accountHint =
    normalizeString(result?.accountName) ??
    normalizeString(result?.credentialMaterialKey) ??
    `chatgpt-web-${Date.now()}`;
  const inferredFileName =
    explicitFileName ??
    sanitizeFileNameComponent(accountHint) ??
    `chatgpt-web-${Date.now()}.json`;
  return path.join(rootDir, familyDir, ensureJsonExtension(inferredFileName));
}

function ensureJsonExtension(filePath) {
  return String(filePath).toLowerCase().endsWith(".json") ? filePath : `${filePath}.json`;
}

function sanitizeFileNameComponent(value) {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || null;
}

function normalizeBaseUrl(value) {
  const text = normalizeString(value) ?? DEFAULT_BASE_URL;
  return text.replace(/\/+$/, "");
}

function normalizePath(value) {
  const text = normalizeString(value);
  if (!text) {
    return null;
  }
  return text.startsWith("/") ? text : `/${text}`;
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeTimeoutMs(value) {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.min(value, 10 * 60 * 1000);
  }
  return DEFAULT_TIMEOUT_MS;
}

function parseBoolean(value, fallback) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }
  return fallback;
}

function resolveHeadlessMode(input) {
  const explicitHeadless = input?.headless ?? process.env.CHATGPT_WEB_SESSION_WORKER_HEADLESS;
  if (typeof explicitHeadless === "boolean") {
    return explicitHeadless;
  }
  if (typeof explicitHeadless === "string") {
    return parseBoolean(explicitHeadless, false);
  }

  // Containerized Linux workers commonly run without an X server. In that case
  // the browser refresh path must default to headless instead of crashing on a
  // headed launch before the retry can even start.
  if (
    process.platform !== "win32" &&
    !normalizeString(process.env.DISPLAY) &&
    !normalizeString(process.env.WAYLAND_DISPLAY)
  ) {
    return true;
  }

  return false;
}

function resolveProxySettings(input) {
  const rawProxy =
    normalizeString(input?.proxyUrl) ??
    normalizeString(input?.proxy_url) ??
    normalizeString(input?.proxyServer) ??
    normalizeString(input?.proxy_server) ??
    normalizeString(input?.credentialProxyUrl) ??
    normalizeString(input?.credential_proxy_url) ??
    normalizeString(input?.outboundProxy) ??
    normalizeString(input?.outbound_proxy) ??
    normalizeString(process.env.CHATGPT_WEB_BROWSER_PROXY_URL) ??
    normalizeString(process.env.ALL_PROXY) ??
    normalizeString(process.env.all_proxy) ??
    normalizeString(process.env.HTTPS_PROXY) ??
    normalizeString(process.env.https_proxy) ??
    normalizeString(process.env.HTTP_PROXY) ??
    normalizeString(process.env.http_proxy);

  if (!rawProxy || rawProxy.toLowerCase() === "direct") {
    return null;
  }

  try {
    const parsed = new URL(rawProxy);
    const launchOptions = {
      server: `${parsed.protocol}//${parsed.hostname}${parsed.port ? `:${parsed.port}` : ""}`,
    };
    const username = parsed.username ? decodeURIComponent(parsed.username) : null;
    const password = parsed.password ? decodeURIComponent(parsed.password) : null;
    const bypass =
      normalizeString(input?.proxyBypass) ??
      normalizeString(input?.proxy_bypass) ??
      normalizeString(input?.noProxy) ??
      normalizeString(input?.no_proxy) ??
      normalizeString(process.env.CHATGPT_WEB_BROWSER_PROXY_BYPASS) ??
      normalizeString(process.env.NO_PROXY) ??
      normalizeString(process.env.no_proxy);
    if (username) {
      launchOptions.username = username;
    }
    if (password) {
      launchOptions.password = password;
    }
    if (bypass) {
      launchOptions.bypass = bypass;
    }
    return {
      launchOptions,
      rawProxy,
    };
  } catch {
    return null;
  }
}

function resolveExecutablePath(configured) {
  const candidates = [];
  if (normalizeString(configured)) {
    candidates.push(configured);
  }
  if (process.platform === "win32") {
    candidates.push(...WINDOWS_EDGE_EXECUTABLES, ...WINDOWS_CHROME_EXECUTABLES);
  } else {
    candidates.push(...LINUX_CHROMIUM_EXECUTABLES);
  }
  return candidates.find((candidate) => candidate && existsSync(candidate)) ?? null;
}

function normalizeAuthSeed(value) {
  if (!value || typeof value !== "object") {
    return null;
  }
  const email = normalizeString(value.email ?? value.loginEmail ?? value.login_email);
  const password = normalizeString(value.password ?? value.loginPassword ?? value.login_password);
  const passwordSha256 = normalizeString(
    value.passwordSha256 ?? value.password_sha256 ?? value.passwordHash ?? value.password_hash,
  );
  if (!email || (!password && !passwordSha256)) {
    return null;
  }
  return {
    email,
    password,
    passwordSha256,
  };
}

function resolveProfileSource(input, authSeed) {
  const configuredUserDataDir =
    normalizeString(input?.userDataDir) ??
    normalizeString(process.env.CHATGPT_WEB_BROWSER_USER_DATA_DIR);
  const autoCloneLocalProfile = parseBoolean(
    input?.autoCloneLocalProfile ??
      process.env.CHATGPT_WEB_BROWSER_AUTO_CLONE_LOCAL_PROFILE,
    false,
  );
  const profileDirectory =
    normalizeString(input?.profileDirectory) ??
    normalizeString(process.env.CHATGPT_WEB_BROWSER_PROFILE_DIR) ??
    DEFAULT_PROFILE_DIRECTORY;

  if (configuredUserDataDir && existsSync(configuredUserDataDir)) {
    return {
      mode: "clone",
      browser: inferBrowserName(configuredUserDataDir),
      userDataDir: configuredUserDataDir,
      profileDirectory,
    };
  }
  if (autoCloneLocalProfile && process.platform === "win32" && existsSync(WINDOWS_EDGE_USER_DATA_DIR)) {
    return {
      mode: "clone",
      browser: "edge",
      userDataDir: WINDOWS_EDGE_USER_DATA_DIR,
      profileDirectory,
    };
  }
  if (autoCloneLocalProfile && process.platform === "win32" && existsSync(WINDOWS_CHROME_USER_DATA_DIR)) {
    return {
      mode: "clone",
      browser: "chrome",
      userDataDir: WINDOWS_CHROME_USER_DATA_DIR,
      profileDirectory,
    };
  }
  return {
    mode: "fresh",
    browser: authSeed?.password ? "chromium-login" : "chromium",
    userDataDir: null,
    profileDirectory,
  };
}

function inferBrowserName(userDataDir) {
  const lower = userDataDir.toLowerCase();
  if (lower.includes("\\edge\\") || lower.includes("/edge/")) {
    return "edge";
  }
  if (lower.includes("\\chrome\\") || lower.includes("/chrome/")) {
    return "chrome";
  }
  return "chromium";
}

async function cloneBrowserProfile(userDataDir, profileDirectory) {
  const sourceProfileDir = path.join(userDataDir, profileDirectory);
  if (!existsSync(userDataDir) || !existsSync(sourceProfileDir)) {
    throw createWorkerError(
      500,
      "chatgpt_web_profile_missing",
      `Browser profile not found at ${sourceProfileDir}. ChatGPT Web refresh requires an existing profile or a browser login seed.`,
    );
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "chatgpt-web-profile-"));
  await mkdir(tempRoot, { recursive: true });
  for (const fileName of ROOT_FILES_TO_COPY) {
    await copyIfExists(path.join(userDataDir, fileName), path.join(tempRoot, fileName));
  }

  const clonedProfileDir = path.join(tempRoot, profileDirectory);
  await mkdir(clonedProfileDir, { recursive: true });
  for (const fileName of PROFILE_FILES_TO_COPY) {
    await copyIfExists(path.join(sourceProfileDir, fileName), path.join(clonedProfileDir, fileName));
  }
  for (const directoryName of PROFILE_DIRS_TO_COPY) {
    await copyRecursive(
      path.join(sourceProfileDir, directoryName),
      path.join(clonedProfileDir, directoryName),
    );
  }
  return tempRoot;
}

async function createFreshBrowserProfile(profileDirectory) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "chatgpt-web-profile-fresh-"));
  await mkdir(path.join(tempRoot, profileDirectory), { recursive: true });
  return tempRoot;
}

async function copyIfExists(source, destination) {
  if (!existsSync(source)) {
    return;
  }
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    if (shouldIgnoreLockedProfileFile(error)) {
      return;
    }
    throw error;
  }
}

async function copyRecursive(source, destination) {
  if (!existsSync(source)) {
    return;
  }
  const sourceStat = await stat(source);
  if (sourceStat.isDirectory()) {
    await mkdir(destination, { recursive: true });
    const entries = await readdir(source, { withFileTypes: true });
    for (const entry of entries) {
      await copyRecursive(path.join(source, entry.name), path.join(destination, entry.name));
    }
    return;
  }
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    if (shouldIgnoreLockedProfileFile(error)) {
      return;
    }
    throw error;
  }
}

function shouldIgnoreLockedProfileFile(error) {
  const code = normalizeString(error?.code)?.toUpperCase();
  return code === "EBUSY" || code === "EPERM";
}

function parseCookieHeader(cookieHeader, host) {
  const header = normalizeString(cookieHeader);
  if (!header) {
    return [];
  }
  return header
    .split(";")
    .map((entry) => {
      const [name, ...rest] = entry.split("=");
      const trimmedName = name?.trim();
      if (!trimmedName) {
        return null;
      }
      return {
        name: trimmedName,
        value: rest.join("=").trim(),
        domain: host,
        path: "/",
        secure: true,
        httpOnly: false,
        sameSite: "Lax",
      };
    })
    .filter(Boolean);
}

function normalizeCookie(cookie, defaultHost) {
  if (!cookie || typeof cookie !== "object") {
    return null;
  }
  const name = normalizeString(cookie.name);
  if (!name) {
    return null;
  }
  const domain =
    normalizeString(cookie.domain)?.replace(/^\./, "") ??
    normalizeString(defaultHost);
  if (!domain) {
    return null;
  }
  return {
    name,
    value: String(cookie.value ?? ""),
    domain,
    path: normalizeString(cookie.path) ?? "/",
    secure: cookie.secure !== false,
    httpOnly: cookie.httpOnly === true,
    expires:
      typeof cookie.expires === "number" && Number.isFinite(cookie.expires)
        ? cookie.expires
        : undefined,
    sameSite:
      cookie.sameSite === "Strict" || cookie.sameSite === "None"
        ? cookie.sameSite
        : "Lax",
  };
}

function dedupeCookies(cookies) {
  const map = new Map();
  for (const cookie of cookies) {
    const key = `${cookie.name}|${cookie.domain}|${cookie.path}`;
    map.set(key, cookie);
  }
  return [...map.values()];
}

function dedupeStrings(values) {
  const unique = [];
  const seen = new Set();
  for (const value of values) {
    const normalized = normalizeString(value);
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    unique.push(normalized);
  }
  return unique;
}

function decodeJwtExpIso(token) {
  const normalized = normalizeString(token);
  if (!normalized) {
    return null;
  }
  try {
    const [, payload] = normalized.split(".");
    if (!payload) {
      return null;
    }
    const base64 = payload.replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, "=");
    const text = Buffer.from(padded, "base64").toString("utf8");
    const decoded = JSON.parse(text);
    return typeof decoded.exp === "number"
      ? new Date(decoded.exp * 1000).toISOString()
      : null;
  } catch {
    return null;
  }
}

function createWorkerError(status, code, message) {
  const error = new Error(message);
  error.status = status;
  error.code = code;
  return error;
}

function serializeError(error) {
  return {
    status: Number.isFinite(error?.status) ? error.status : 500,
    code: normalizeString(error?.code) ?? "chatgpt_web_session_worker_failed",
    message:
      normalizeString(error?.message) ??
      "ChatGPT Web session worker failed without a structured error message.",
  };
}

async function readStdin() {
  return new Promise((resolve, reject) => {
    let buffer = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      buffer += chunk;
    });
    process.stdin.on("end", () => resolve(buffer.trim() || "{}"));
    process.stdin.on("error", reject);
  });
}

function printAndExit(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exit(exitCode);
}

await main();
