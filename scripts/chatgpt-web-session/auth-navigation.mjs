import { randomUUID } from "node:crypto";
import { normalizeString, dedupeStrings } from "./configuration.mjs";
import { collectPageState, pageStateRequiresAuthRecovery, pageStateIsCloudflareWait, safePageUrl } from "./page-state.mjs";

const AUTH_BASE = "https://auth.openai.com";

const PLATFORM_OPENAI_LOGIN_URL = "https://platform.openai.com/login";

const CHATGPT_NEXTAUTH_CSRF_PATH = "/api/auth/csrf";

const CHATGPT_NEXTAUTH_SIGNIN_OPENAI_PATH = "/api/auth/signin/openai";

const LOGIN_OR_CREATE_ACCOUNT_URL = `${AUTH_BASE}/log-in-or-create-account`;

const OPENAI_LOGIN_URL = `${AUTH_BASE}/log-in`;

export async function navigateToEmailSurface(page, { baseUrl, authUrl, timeoutMs }) {
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

export async function clickPreLoginButtons(page) {
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

export async function clickButtonByExactText(page, texts) {
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

export async function bootstrapChatGptOauthSession(page, context, { baseUrl, timeoutMs, deviceId }) {
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

export async function waitForAuthCookieState(
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
