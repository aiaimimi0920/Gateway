import { existsSync } from "node:fs";
import { normalizeString, createError } from "./pure.mjs";

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

export async function pageLooksChallenged(page) {
  const title = String(await page.title().catch(() => "")).toLowerCase();
  if (title.includes("just a moment") || title.includes("attention required")) {
    return true;
  }
  return await page
    .evaluate(() => {
      const selectors = [
        "#challenge-form",
        ".cf-turnstile",
        "[name='cf-turnstile-response']",
        "[data-translate='challenge-running']",
        "iframe[src*='challenges.cloudflare.com']",
        "iframe[src*='turnstile']",
      ];
      return selectors.some((selector) =>
        Array.from(document.querySelectorAll(selector)).some(
          (element) => element.getClientRects().length > 0,
        ),
      );
    })
    .catch(() => false);
}

export async function pageLooksSignedOut(page) {
  const text = String(await page.locator("body").innerText().catch(() => "")).toLowerCase();
  const hasSignInSignal =
    text.includes("sign in") ||
    text.includes("log in") ||
    text.includes("登录") ||
    text.includes("登入");
  const hasCreateSignal =
    text.includes("create song") ||
    text.includes("create") ||
    text.includes("创作歌曲") ||
    text.includes("创作");
  return hasSignInSignal && !hasCreateSignal;
}

export async function runCreateThroughPageUi(page, options) {
  const { prompt, timeoutMs } = options;
  await dismissKnownBlockingOverlays(page);
  let createButton = page
    .locator('button:visible[aria-label="创作歌曲"], button:visible[aria-label="Create song"]')
    .first();
  if (!(await createButton.count())) {
    createButton = page.locator('button:visible:has-text("创作"), button:visible:has-text("Create")').last();
  }
  await createButton.waitFor({ state: "visible", timeout: timeoutMs });

  await fillCreateInputs(page, {
    prompt,
    createButton,
    timeoutMs,
  });
  await expectButtonEnabled(createButton, timeoutMs);

  const createResponsePromise = page.waitForResponse(
    (response) =>
      /\/api\/generate\/v2-web\/?$/.test(response.url()) &&
      response.request().method().toUpperCase() === "POST",
    { timeout: timeoutMs },
  );
  await createButton.click();
  try {
    const createResponse = await createResponsePromise;
    return {
      status: createResponse.status(),
      bodyText: await createResponse.text(),
    };
  } catch (error) {
    if (await pageLooksChallenged(page)) {
      throw createError(
        429,
        "suno_browser_challenge_required",
        "Complete the visible Suno security check before generation can continue.",
        error?.message,
      );
    }
    throw error;
  }
}

async function fillCreateInputs(page, options) {
  const { prompt, createButton, timeoutMs } = options;
  const promptArea = page.locator('textarea:visible[maxlength="3000"]').first();
  await promptArea.waitFor({ state: "visible", timeout: timeoutMs });
  await promptArea.fill(prompt);
  await page.waitForFunction(
    ({ expected }) =>
      Array.from(document.querySelectorAll('textarea[maxlength="3000"]')).some(
        (textarea) => textarea.getClientRects().length > 0 && textarea.value === expected,
      ),
    { expected: prompt },
    { timeout: Math.min(timeoutMs, 10_000) },
  );
  await expectButtonEnabled(createButton, timeoutMs);
}

export async function maybeClassifyVideoSubscriptionRequired(page) {
  await dismissKnownBlockingOverlays(page);
  const openedPublishDialog = await clickButtonByText(page, "Publish").catch(() => false);
  if (!openedPublishDialog) {
    return false;
  }
  await page.waitForTimeout(750);

  const openedAnimateDialog = await clickButtonByText(page, "Animate").catch(() => false);
  if (!openedAnimateDialog) {
    return false;
  }
  await page.waitForTimeout(1_500);

  const bodyText = String(await page.locator("body").innerText().catch(() => ""));
  return /subscribe to generate/i.test(bodyText);
}

export async function dismissKnownBlockingOverlays(page) {
  const hasVisibleSecurityChallenge = await page
    .evaluate(() =>
      Array.from(
        document.querySelectorAll(
          'iframe[src*="challenges.cloudflare.com"], iframe[src*="turnstile"], iframe[src*="hcaptcha"]',
        ),
      ).some((frame) => frame.getClientRects().length > 0),
    )
    .catch(() => false);
  if (!hasVisibleSecurityChallenge) {
    await page.keyboard.press("Escape").catch(() => undefined);
    await page.waitForTimeout(150).catch(() => undefined);
    await page.keyboard.press("Escape").catch(() => undefined);
    await page.waitForTimeout(150).catch(() => undefined);
  }

  await page
    .evaluate(() => {
      const clickByText = (texts) => {
        for (const button of Array.from(document.querySelectorAll("button"))) {
          const text = (button.innerText || "").trim();
          if (texts.includes(text) && !button.disabled) {
            button.click();
            return true;
          }
        }
        return false;
      };

      clickByText(["Accept All Cookies", "Accept All", "Reject All", "Only Necessary Cookies"]);
      clickByText(["关闭", "Close", "Done", "Cancel"]);
    })
    .catch(() => undefined);

  await page.waitForTimeout(300).catch(() => undefined);
}

async function expectButtonEnabled(locator, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await locator.isEnabled().catch(() => false)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("Create button did not become enabled.");
}

async function clickButtonByText(page, text) {
  return await page.evaluate((buttonText) => {
    const button = Array.from(document.querySelectorAll("button")).find(
      (element) => (element.innerText || "").trim() === buttonText,
    );
    if (!button || button.disabled) {
      return false;
    }
    button.click();
    return true;
  }, text);
}

export async function resolveWorkerPage(context, options) {
  const { targetUrl, fallbackUrl, navigationTimeoutMs, forceNavigate, borrowedContext } = options;
  const target = safeUrl(targetUrl);
  const existingPage = context
    .pages()
    .find((page) => pageMatchesTarget(page, target) || pageMatchesTarget(page, safeUrl(fallbackUrl)));
  if (borrowedContext && !existingPage) {
    throw new Error("Suno CDP session does not contain the expected create page.");
  }
  const page = existingPage ?? (await context.newPage());

  if (!borrowedContext && (forceNavigate || !existingPage || !pageMatchesTarget(page, target))) {
    await page.goto(target?.toString() ?? fallbackUrl, {
      waitUntil: "domcontentloaded",
      timeout: navigationTimeoutMs,
    });
  } else {
    await page.waitForLoadState("domcontentloaded", { timeout: navigationTimeoutMs }).catch(() => undefined);
  }
  await page.waitForTimeout(1500);
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

export function parseCookieHeader(cookieHeader) {
  return cookieHeader
    .split(/;\s*/)
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const separator = entry.indexOf("=");
      if (separator <= 0) {
        return null;
      }
      const name = entry.slice(0, separator).trim();
      const value = entry.slice(separator + 1).trim();
      if (!name || !value) {
        return null;
      }
      return {
        name,
        value,
        domain: ".suno.com",
        path: "/",
        httpOnly: false,
        secure: true,
        sameSite: "Lax",
      };
    })
    .filter(Boolean);
}

export function resolveExecutablePath(explicitPath) {
  const direct = normalizeString(explicitPath);
  if (direct && existsSync(direct)) {
    return direct;
  }
  const candidates =
    process.platform === "win32"
      ? WINDOWS_BROWSER_PATHS
      : process.platform === "darwin"
        ? MACOS_BROWSER_PATHS
        : LINUX_BROWSER_PATHS;
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

export async function resolveBrowserExecutionTarget(options) {
  const { input, browserCdpUrl, browserCdpTargetUrl, referer, timeoutMs } = options;
  if (browserCdpUrl) {
    return {
      browserCdpUrl,
      browserCdpTargetUrl,
      lease: null,
    };
  }

  const easyBrowserBaseUrl = normalizeString(
    input.easyBrowserBaseUrl ?? process.env.SUNO_EASYBROWSER_BASE_URL ?? null,
  );
  if (!easyBrowserBaseUrl) {
    return {
      browserCdpUrl: null,
      browserCdpTargetUrl,
      lease: null,
    };
  }

  const easyBrowserBearerToken = normalizeString(
    input.easyBrowserBearerToken ?? process.env.SUNO_EASYBROWSER_BEARER_TOKEN ?? null,
  );
  const easyBrowserProvider =
    normalizeString(input.easyBrowserProvider ?? process.env.SUNO_EASYBROWSER_PROVIDER ?? null) ??
    "chrome";
  const easyBrowserRuntimeReuse =
    normalizeString(
      input.easyBrowserRuntimeReuse ?? process.env.SUNO_EASYBROWSER_RUNTIME_REUSE ?? null,
    ) ?? "require_reuse";
  const easyBrowserStartupUrl =
    normalizeString(input.easyBrowserStartupUrl ?? process.env.SUNO_EASYBROWSER_STARTUP_URL ?? null) ??
    browserCdpTargetUrl ??
    referer;

  const lease = await acquireEasyBrowserSession({
    baseUrl: easyBrowserBaseUrl,
    bearerToken: easyBrowserBearerToken,
    provider: easyBrowserProvider,
    runtimeReuse: easyBrowserRuntimeReuse,
    startupUrl: easyBrowserStartupUrl,
    timeoutMs,
  });

  return {
    browserCdpUrl: lease.browserCdpUrl,
    browserCdpTargetUrl: lease.pageUrl ?? browserCdpTargetUrl,
    lease,
  };
}

async function acquireEasyBrowserSession(options) {
  const { baseUrl, bearerToken, provider, runtimeReuse, startupUrl, timeoutMs } = options;
  const response = await fetch(`${baseUrl.replace(/\/+$/, "")}/v1/browser/sessions/acquire`, {
    method: "POST",
    headers: buildEasyBrowserHeaders(bearerToken),
    body: JSON.stringify({
      request_id: `suno-easybrowser-${Date.now()}`,
      mode: "direct",
      provider_hint: provider,
      startup_url: startupUrl,
      runtime_reuse: runtimeReuse,
      timeout_ms: timeoutMs,
    }),
  }).catch((error) => {
    throw createError(
      502,
      "suno_easybrowser_unavailable",
      `Failed to contact EasyBrowser: ${error.message}`,
    );
  });

  const rawText = await response.text();
  let payload;
  try {
    payload = rawText ? JSON.parse(rawText) : {};
  } catch (error) {
    throw createError(
      response.status || 500,
      "suno_easybrowser_invalid_json",
      `EasyBrowser returned invalid JSON: ${error.message}`,
      rawText,
    );
  }

  if (!response.ok || payload?.success === false) {
    const message =
      payload?.error?.message || payload?.message || `EasyBrowser returned HTTP ${response.status}.`;
    throw createError(
      response.status || payload?.error?.status || 500,
      "suno_easybrowser_acquire_failed",
      message,
      rawText,
    );
  }

  const session = payload?.data?.session;
  const attach = session?.attach;
  const browserCdpUrl = normalizeString(attach?.endpoint);
  if (!browserCdpUrl) {
    throw createError(
      500,
      "suno_easybrowser_missing_cdp",
      "EasyBrowser session did not return attach.endpoint for CDP attach.",
      rawText,
    );
  }

  return {
    baseUrl: baseUrl.replace(/\/+$/, ""),
    bearerToken,
    sessionId: normalizeString(session?.session_id),
    browserCdpUrl,
    pageUrl: normalizeString(attach?.page_url) ?? normalizeString(session?.current_url),
  };
}

export async function releaseEasyBrowserLease(lease) {
  if (!lease?.sessionId || !lease?.baseUrl) {
    return;
  }
  await fetch(`${lease.baseUrl}/v1/browser/sessions/${encodeURIComponent(lease.sessionId)}/release`, {
    method: "POST",
    headers: buildEasyBrowserHeaders(lease.bearerToken),
  });
}

function buildEasyBrowserHeaders(bearerToken) {
  const headers = {
    "content-type": "application/json",
  };
  if (bearerToken) {
    headers.authorization = `Bearer ${bearerToken}`;
  }
  return headers;
}
