import { normalizeString, scopeGeminiUrlToAuthUser } from "./gemini-canvas-browser-pool-input.mjs";

export async function hasPromptTextbox(page) {
  try {
    const textbox = page
      .locator(
        '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
      )
      .first();
    return (await textbox.count()) > 0;
  } catch {
    return false;
  }
}

export async function clickFirstVisible(candidates) {
  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) > 0) {
        await candidate.waitFor({ state: "visible", timeout: 4000 });
        await candidate.click({ timeout: 12000, force: true });
        return true;
      }
    } catch {
      // try next selector
    }
  }
  return false;
}

function bodyIndicatesGoogleConsent(text, url = null) {
  const normalizedText = String(text || "");
  const normalizedUrl = String(url || "").toLowerCase();
  return (
    normalizedUrl.includes("consent.google.com") ||
    /Before you continue to Google|We use cookies and data to|Accept all|Reject all|More options|在您继续使用 Google 之前|我们会使用 Cookie 和数据|接受全部|全部接受|拒绝全部|全部拒绝|更多选项|g\.co\/privacytools/i.test(
      normalizedText,
    )
  );
}

export function bodyIndicatesGeminiSignedOutLanding(text) {
  const normalizedText = String(text || "");
  return (
    /Sign in|登录|登入/i.test(normalizedText) &&
    (/Meet Gemini, your personal AI assistant|personal AI assistant/i.test(normalizedText) ||
      (/认识\s*Gemini/i.test(normalizedText) && /私人\s*AI/i.test(normalizedText)))
  );
}

export function bodyTextIndicatesGeminiAppSurface(text) {
  return /与 Gemini 对话|Talk to Gemini|Conversation with Gemini|发起新对话|New chat|快速|Fast|制作图片|Create image|Make image|创作音乐|Create music|创作视频|Create video/i.test(
    String(text || ""),
  );
}

export function shouldTreatGeminiPageAsAuthBlocked(bodyText, options = {}) {
  const normalizedText = String(bodyText || "");
  const hasPromptTextbox = options?.hasPromptTextbox === true;
  const hasGeminiAppSurface =
    options?.hasGeminiAppSurface === true || bodyTextIndicatesGeminiAppSurface(normalizedText);
  if (hasPromptTextbox && hasGeminiAppSurface) {
    return false;
  }
  return (
    bodyIndicatesGeminiSignedOutLanding(normalizedText) ||
    (/sign in|登录|登入|继续登录/i.test(normalizedText) && !hasGeminiAppSurface)
  );
}

export function pageLooksLikeReusableGeminiAppSurface(pageUrl, bodyText, baseUrl) {
  const normalizedBaseUrl = normalizeString(baseUrl)?.replace(/\/+$/, "") ?? "";
  const normalizedPageUrl = normalizeString(pageUrl) ?? "";
  if (!normalizedBaseUrl || !normalizedPageUrl.startsWith(normalizedBaseUrl)) {
    return false;
  }
  if (!/\/app(?:\/|$|\?)/i.test(normalizedPageUrl)) {
    return false;
  }
  return (
    bodyTextIndicatesGeminiAppSurface(bodyText) &&
    !bodyIndicatesGeminiSignedOutLanding(bodyText)
  );
}

export async function findAttachedGeminiAppPage(entry, baseUrl) {
  if (!entry?.attachedCdp || !entry?.context || typeof entry.context.pages !== "function") {
    return null;
  }
  const pages = entry.context.pages().filter((page) => !page.isClosed());
  for (const page of pages) {
    const pageUrl = normalizeString(page.url()) ?? "";
    const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    if (pageLooksLikeReusableGeminiAppSurface(pageUrl, bodyText, baseUrl)) {
      return page;
    }
  }
  return null;
}

export function createAppPageOwner({ log, inferGoogleAuthUser, syncCookieHeaderIntoContext, collectButtonSnapshot }) {
  async function ensureAppPage(entry, baseUrl, timeoutMs, options = {}) {
    if (entry.page.isClosed()) {
      throw Object.assign(new Error("Gemini Canvas browser page was unexpectedly closed."), {
        status: 500,
        code: "gemini_canvas_page_closed",
      });
    }

    const normalizedBaseUrl = baseUrl.replace(/\/+$/, "");
    const currentPageUrl = normalizeString(entry.page.url());
    const hasExplicitAuthUser =
      typeof currentPageUrl === "string" &&
      (/[?&]authuser=\d+\b/i.test(currentPageUrl) || /\/u\/\d+\b/i.test(currentPageUrl));
    const configuredAuthUser = (() => {
      try {
        return new URL(normalizedBaseUrl).pathname.match(/^\/u\/(\d+)(?:\/|$)/i)?.[1] ?? null;
      } catch {
        return null;
      }
    })();
    const authUser = hasExplicitAuthUser
      ? inferGoogleAuthUser(currentPageUrl)
      : configuredAuthUser;
    const unscopedAppUrl = (() => {
      try {
        return `${new URL(normalizedBaseUrl).origin}/app`;
      } catch {
        return `${normalizedBaseUrl}/app`;
      }
    })();
    const appUrl = scopeGeminiUrlToAuthUser(unscopedAppUrl, authUser) ?? unscopedAppUrl;
    let shouldNavigate = true;
    let currentBodyText = "";
    try {
      const current = new URL(entry.page.url() || appUrl);
      const target = new URL(appUrl);
      shouldNavigate =
        current.origin !== target.origin ||
        current.pathname.replace(/\/+$/, "") !== target.pathname.replace(/\/+$/, "");
    } catch {
      shouldNavigate = !entry.page.url().startsWith(appUrl);
    }

    if (options?.skipInitialNavigationWhenAppSurfaceReady === true) {
      currentBodyText = await entry.page
        .evaluate(() => document.body?.innerText ?? "")
        .catch(() => "");
      if (
        pageLooksLikeReusableGeminiAppSurface(
          currentPageUrl,
          currentBodyText,
          normalizedBaseUrl,
        )
      ) {
        shouldNavigate = false;
        log(
          "ensureAppPage reusing attached app surface without hard reload",
          JSON.stringify({
            appUrl,
            currentPageUrl,
            runtimeStatePath: entry.runtimeStatePath,
          }),
        );
      }
    }

    if (shouldNavigate) {
      log("ensureAppPage navigating", JSON.stringify({ appUrl, runtimeStatePath: entry.runtimeStatePath }));
      await entry.page.goto(appUrl, {
        waitUntil: "domcontentloaded",
        timeout: timeoutMs,
      });
    }

    await entry.page.waitForTimeout(2000);
    const consentResolved = await tryResolveGoogleConsent(entry.page, timeoutMs).catch(() => false);
    log("ensureAppPage consent pass", JSON.stringify({ consentResolved, finalUrl: entry.page.url() }));
    if (!consentResolved) {
      const consentUrl = entry.page.url();
      const consentBodyText = await entry.page
        .evaluate(() => document.body?.innerText ?? "")
        .catch(() => "");
      if (bodyIndicatesGoogleConsent(consentBodyText, consentUrl)) {
        const consentButtons = await collectButtonSnapshot(entry.page).catch(() => []);
        throw Object.assign(new Error("Google consent surface is still blocking Gemini app interaction."), {
          status: 409,
          code: "gemini_canvas_google_consent_unresolved",
          bodyText: JSON.stringify(
            {
              pageUrl: consentUrl,
              bodyText: consentBodyText,
              buttons: consentButtons.slice(0, 120),
              runtimeStatePath: entry.runtimeStatePath,
              runtimeStateMode: entry.runtimeStateMode,
              launchClonedProfile: entry.launchClonedProfile === true,
            },
            null,
            2,
          ),
        });
      }
    }
    const finalUrl = entry.page.url();
    const loweredUrl = finalUrl.toLowerCase();
    if (
      loweredUrl.includes("signin") ||
      loweredUrl.includes("servicelogin") ||
      loweredUrl.includes("accounts.google.com")
    ) {
      throw Object.assign(new Error(`Gemini Canvas navigation redirected to ${finalUrl}.`), {
        status: 401,
        code: "gemini_canvas_auth_redirect",
      });
    }

    let bodyText = await entry.page
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
    const evaluateAuthBlockedSurface = async (text) => {
      const hasPromptTextboxVisible = await hasPromptTextbox(entry.page);
      return {
        hasPromptTextbox: hasPromptTextboxVisible,
        authBlocked: shouldTreatGeminiPageAsAuthBlocked(text, {
          hasPromptTextbox: hasPromptTextboxVisible,
        }),
      };
    };
    let authBlockedSurface = await evaluateAuthBlockedSurface(bodyText);
    if (
      authBlockedSurface.authBlocked &&
      normalizeString(options.cookieHeader) &&
      options.cookieRehydrateAttempted !== true
    ) {
      const forcedCookieSyncCount = await syncCookieHeaderIntoContext(
        entry.context,
        options.cookieHeader,
        normalizedBaseUrl,
      ).catch((error) => {
        log(
          "ensureAppPage forced cookie rehydrate failed",
          JSON.stringify({
            runtimeStatePath: entry.runtimeStatePath,
            message: error instanceof Error ? error.message : String(error),
          }),
        );
        return 0;
      });
      if (forcedCookieSyncCount > 0) {
        log(
          "ensureAppPage forced cookie rehydrate",
          JSON.stringify({
            runtimeStatePath: entry.runtimeStatePath,
            runtimeStateMode: entry.runtimeStateMode,
            launchClonedProfile: entry.launchClonedProfile === true,
            forcedCookieSyncCount,
          }),
        );
        await entry.page.goto(appUrl, {
          waitUntil: "domcontentloaded",
          timeout: timeoutMs,
        });
        await entry.page.waitForTimeout(2000);
        const postRehydrateConsentResolved = await tryResolveGoogleConsent(entry.page, timeoutMs).catch(() => false);
        log(
          "ensureAppPage post-rehydrate consent pass",
          JSON.stringify({ postRehydrateConsentResolved, finalUrl: entry.page.url() }),
        );
        bodyText = await entry.page
          .evaluate(() => document.body?.innerText ?? "")
          .catch(() => "");
        authBlockedSurface = await evaluateAuthBlockedSurface(bodyText);
      }
    }
    if (authBlockedSurface.authBlocked) {
      const authGateGraceDeadline = Date.now() + Math.min(timeoutMs, 8_000);
      while (Date.now() < authGateGraceDeadline) {
        await entry.page.waitForTimeout(1_000);
        const graceConsentResolved = await tryResolveGoogleConsent(entry.page, timeoutMs).catch(() => false);
        if (graceConsentResolved) {
          log("ensureAppPage grace consent resolved", JSON.stringify({ pageUrl: entry.page.url() }));
        }
        const reloadedUrl = entry.page.url();
        const loweredReloadedUrl = reloadedUrl.toLowerCase();
        if (
          loweredReloadedUrl.includes("signin") ||
          loweredReloadedUrl.includes("servicelogin") ||
          loweredReloadedUrl.includes("accounts.google.com")
        ) {
          throw Object.assign(new Error(`Gemini Canvas navigation redirected to ${reloadedUrl}.`), {
            status: 401,
            code: "gemini_canvas_auth_redirect",
          });
        }
        bodyText = await entry.page
          .evaluate(() => document.body?.innerText ?? "")
          .catch(() => "");
        authBlockedSurface = await evaluateAuthBlockedSurface(bodyText);
        if (!authBlockedSurface.authBlocked) {
          return;
        }
      }
      log(
        "ensureAppPage auth gate",
        JSON.stringify({
          finalUrl,
          hasPromptTextbox: authBlockedSurface.hasPromptTextbox,
          bodyPreview: String(bodyText || "").slice(0, 600),
          runtimeStatePath: entry.runtimeStatePath,
          runtimeStateMode: entry.runtimeStateMode,
        }),
      );
      throw Object.assign(
        new Error("Gemini Canvas browser context appears to have fallen back to a sign-in page."),
        {
          status: 401,
          code: "gemini_canvas_auth_required",
        },
      );
    }
  }

  async function tryResolveGoogleConsent(page, timeoutMs) {
    const currentUrl = page.url();
    const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    if (!bodyIndicatesGoogleConsent(bodyText, currentUrl)) {
      return false;
    }

    log(
      "google consent surface detected",
      JSON.stringify({
        currentUrl,
        bodyPreview: String(bodyText || "").slice(0, 400),
      }),
    );

    const clickConsentAction = async (labelMatchers, actionLabel) => {
      const candidates = [];
      for (const matcher of labelMatchers) {
        candidates.push(
          page.getByRole("button", { name: matcher }).first(),
          page.locator('button,[role="button"],a').filter({ hasText: matcher }).first(),
        );
      }
      let clicked = await clickFirstVisible(candidates);
      if (!clicked) {
        clicked = await page
          .evaluate((rawMatchers) => {
            const labels = rawMatchers.map((entry) => new RegExp(entry.pattern, entry.flags));
            const isVisible = (node) => {
              if (!(node instanceof HTMLElement)) {
                return false;
              }
              const style = window.getComputedStyle(node);
              if (style.display === "none" || style.visibility === "hidden") {
                return false;
              }
              const rect = node.getBoundingClientRect();
              return rect.width > 0 && rect.height > 0;
            };
            const clickNode = (node) => {
              node.scrollIntoView({ block: "center", inline: "center" });
              node.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true }));
              node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
              node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
              node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
            };
            const nodes = Array.from(document.querySelectorAll("button,[role='button'],a,input[type='button'],input[type='submit']"));
            for (const matcher of labels) {
              for (const node of nodes) {
                const text = (node.innerText || node.getAttribute("value") || node.getAttribute("aria-label") || "").trim();
                if (!text || !matcher.test(text) || !isVisible(node)) {
                  continue;
                }
                clickNode(node);
                return {
                  clicked: true,
                  text,
                  tag: node.tagName || null,
                };
              }
            }
            return { clicked: false };
          }, labelMatchers.map((matcher) => ({ pattern: matcher.source, flags: matcher.flags })))
          .then((result) => {
            if (result?.clicked) {
              log("google consent dom fallback clicked", JSON.stringify({ actionLabel, ...result }));
              return true;
            }
            return false;
          })
          .catch(() => false);
      }
      if (clicked) {
        log("google consent action clicked", actionLabel);
      }
      return clicked;
    };

    const attempts = [
      { label: "reject_all", matchers: [/Reject all|拒绝全部|全部拒绝/i] },
      { label: "accept_all", matchers: [/Accept all|接受全部|全部接受/i] },
      { label: "agree", matchers: [/I agree|我同意/i] },
    ];

    for (const attempt of attempts) {
      const clicked = await clickConsentAction(attempt.matchers, attempt.label);
      if (!clicked) {
        continue;
      }
      const deadline = Date.now() + Math.min(timeoutMs, 8_000);
      while (Date.now() < deadline) {
        await page.waitForTimeout(800);
        const nextUrl = page.url();
        const nextBodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
        if (!bodyIndicatesGoogleConsent(nextBodyText, nextUrl)) {
          log(
            "google consent surface resolved",
            JSON.stringify({
              actionLabel: attempt.label,
              nextUrl,
              bodyPreview: String(nextBodyText || "").slice(0, 300),
            }),
          );
          return true;
        }
      }
    }

    log("google consent surface unresolved", "timed out waiting for consent page to dismiss");
    return false;
  }

  return { ensureAppPage, tryResolveGoogleConsent };
}
