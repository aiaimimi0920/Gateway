import { chromium } from "playwright-core";
import process from "node:process";
import { existsSync } from "node:fs";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_WAIT_TIMEOUT_MS = 150_000;
const DEFAULT_POLL_INTERVAL_MS = 3_000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_NAVIGATION_TIMEOUT_MS = 45_000;

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

function validateInput(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw createError(400, "suno_browser_invalid_input", "Suno browser worker input must be an object.");
  }
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeBaseUrl(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw createError(400, "suno_browser_missing_base_url", "baseUrl is required.");
  }
  return normalized.replace(/\/+$/, "");
}

async function resolveBrowserExecutionTarget(options) {
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

async function releaseEasyBrowserLease(lease) {
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

function normalizeLocale(value) {
  const normalized = normalizeString(value);
  return normalized ? normalized.split(",")[0]?.trim() || null : null;
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
  throw createError(
    400,
    "suno_browser_invalid_target_asset_kind",
    "targetAssetKind must be image, audio, or video.",
  );
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

function extractPrompt(requestBody) {
  if (!requestBody || typeof requestBody !== "object") {
    return null;
  }
  const direct =
    normalizeString(requestBody.prompt) ??
    normalizeString(requestBody.input) ??
    normalizeString(requestBody.lyrics);
  if (direct) {
    return direct;
  }
  if (Array.isArray(requestBody.parts)) {
    for (const part of requestBody.parts) {
      const value = normalizeString(part?.content) ?? normalizeString(part?.text);
      if (value) {
        return value;
      }
    }
  }
  return null;
}

function readClips(body) {
  return Array.isArray(body?.clips) ? body.clips : [];
}

function collectClipIds(clips) {
  return clips
    .map((clip) => normalizeString(clip?.id))
    .filter(Boolean);
}

function clipHasTargetAsset(clip, targetAssetKind) {
  if (!clip || typeof clip !== "object") {
    return false;
  }
  const field =
    targetAssetKind === "image"
      ? "image_url"
      : targetAssetKind === "video"
        ? "video_url"
        : "audio_url";
  return Boolean(normalizeString(clip[field]) ?? normalizeString(clip[toCamel(field)]));
}

function clipTerminalStatus(clip) {
  const status = normalizeString(clip?.status)?.toLowerCase();
  return Boolean(status && ["complete", "completed", "error", "failed"].includes(status));
}

function clipCompletedStatus(clip) {
  const status = normalizeString(clip?.status)?.toLowerCase();
  return status === "complete" || status === "completed";
}

function clipsTerminal(clips) {
  return Array.isArray(clips) && clips.length > 0 && clips.every((clip) => clipTerminalStatus(clip));
}

function clipsReadyForTarget(clips, targetAssetKind) {
  return (
    Array.isArray(clips) &&
    clips.length > 0 &&
    clips.every(
      (clip) => clipCompletedStatus(clip) && clipHasTargetAsset(clip, targetAssetKind),
    )
  );
}

function toCamel(value) {
  return value.replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
}

function classifyChallengeLikeBody(bodyText) {
  const normalized = String(bodyText ?? "").toLowerCase();
  return (
    normalized.includes("challenge") ||
    normalized.includes("captcha") ||
    normalized.includes("cloudflare") ||
    normalized.includes("just a moment") ||
    normalized.includes("cf-chl")
  );
}

async function pageLooksChallenged(page) {
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

async function pageLooksSignedOut(page) {
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

async function runCreateThroughPageUi(page, options) {
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

async function maybeClassifyVideoSubscriptionRequired(page) {
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

async function dismissKnownBlockingOverlays(page) {
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

async function resolveWorkerPage(context, options) {
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

function parseCookieHeader(cookieHeader) {
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

function resolveExecutablePath(explicitPath) {
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

function createError(status, code, message, body) {
  return Object.assign(new Error(message), {
    status,
    code,
    body: normalizeString(body) ?? undefined,
  });
}

function normalizeError(error) {
  return {
    status: Number.isFinite(error?.status) ? Number(error.status) : 500,
    code: normalizeString(error?.code) ?? "suno_browser_worker_failed",
    message: normalizeString(error?.message) ?? "Suno browser worker failed.",
    body: normalizeString(error?.body) ?? undefined,
  };
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
