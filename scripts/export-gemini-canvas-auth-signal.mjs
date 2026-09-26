import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

const AUTH_COOKIE_NAMES = new Set([
  "SID",
  "HSID",
  "SSID",
  "APISID",
  "SAPISID",
  "__Secure-1PSID",
  "__Secure-3PSID",
  "__Secure-1PSIDTS",
  "__Secure-3PSIDTS",
]);

function hasGoogleAuthCookies(cookies) {
  return Array.isArray(cookies) && cookies.some(
    (cookie) =>
      AUTH_COOKIE_NAMES.has(cookie.name) &&
      isGoogleCookieDomain(cookie.domain),
  );
}

function isGoogleCookieDomain(domain) {
  const normalized = normalizeString(domain)?.replace(/^\./, "").toLowerCase() ?? "";
  return Boolean(
    normalized &&
      (normalized === "google.com" || normalized.endsWith(".google.com")),
  );
}

function isGoogleVerificationChallengeVisible({ bodyText, title } = {}) {
  const body = String(bodyText ?? "");
  const heading = String(title ?? "");
  return [
    /为了您的账号安全.*请验证后登录/i,
    /选择所有符合描述的图片/i,
    /包含文字[:：""]?\s*["“”][^"“”]+["“”]/i,
    /verify (?:to continue|it's you|you can continue)/i,
    /select all images that match/i,
    /choose all images/i,
  ].some((pattern) => pattern.test(body) || pattern.test(heading));
}

function isGeminiSurfaceUrl(url) {
  const normalized = normalizeString(url)?.toLowerCase() ?? "";
  return normalized.startsWith("https://gemini.google.com/");
}

async function collectGeminiPageAuthSignal(page) {
  const pageSignal = await page.evaluate(() => {
    const bodyText = document.body?.innerText ?? "";
    const loweredBody = bodyText.toLowerCase();
    const title = document.title ?? "";
    const loweredTitle = title.toLowerCase();
    const images = Array.from(document.images || []).map((entry) => ({
      alt: entry.alt || "",
      src: entry.currentSrc || entry.src || "",
    }));
    const avatarSources = images
      .filter((entry) => /个人资料照片|profile photo|account/i.test(entry.alt))
      .map((entry) => entry.src);
    const hasNonDefaultAvatar = avatarSources.some(
      (src) => typeof src === "string" && src.includes("googleusercontent.com") && !src.includes("default-user"),
    );
    const hasConversationInput = Boolean(
      document.querySelector("textarea, [role='textbox'], rich-textarea"),
    );
    const hasAccountMenuButton = Boolean(
      document.querySelector(
        "[aria-label*='Google Account'], [aria-label*='Google 帐号'], [aria-label*='Google 账号'], [aria-label*='Account']",
      ),
    );
    const loginVisible =
      /(^|\n)\s*(登录|登入|sign in)(\s|$)/i.test(bodyText) ||
      Boolean(
        document.querySelector(
          "a[href*='accounts.google.com'], a[href*='signin'], button[aria-label*='登录'], button[aria-label*='Sign in']",
        ),
      );
    const marketingVisible =
      loweredBody.includes("认识 gemini") ||
      loweredBody.includes("your personal ai assistant") ||
      loweredBody.includes("私人 ai 助理") ||
      loweredBody.includes("about gemini") ||
      loweredBody.includes("企业应用场景");
    const hasGeminiSurface =
      loweredTitle.includes("gemini") ||
      loweredBody.includes("gemini") ||
      location.hostname === "gemini.google.com";
    return {
      bodyText,
      avatarSources,
      hasAccountMenuButton,
      hasGeminiSurface,
      hasNonDefaultAvatar,
      hasConversationInput,
      loginVisible,
      marketingVisible,
      url: location.href,
      title,
    };
  });
  return {
    ...pageSignal,
    verificationChallengeVisible: isGoogleVerificationChallengeVisible(pageSignal),
  };
}

function isGeminiAuthenticatedSignal({ url, cookies, pageSignal } = {}) {
  return Boolean(
    isGeminiSurfaceUrl(url) &&
      hasGoogleAuthCookies(cookies) &&
      pageSignal &&
      !pageSignal.loginVisible &&
      !pageSignal.verificationChallengeVisible &&
      (
        pageSignal.hasConversationInput ||
        pageSignal.hasNonDefaultAvatar ||
        pageSignal.hasAccountMenuButton ||
        pageSignal.hasGeminiSurface
      ),
  );
}

function summarizeGeminiPageSignal(pageSignal) {
  return {
    title: normalizeString(pageSignal?.title) ?? null,
    url: normalizeString(pageSignal?.url) ?? null,
    hasGeminiSurface: Boolean(pageSignal?.hasGeminiSurface),
    hasConversationInput: Boolean(pageSignal?.hasConversationInput),
    hasNonDefaultAvatar: Boolean(pageSignal?.hasNonDefaultAvatar),
    hasAccountMenuButton: Boolean(pageSignal?.hasAccountMenuButton),
    loginVisible: Boolean(pageSignal?.loginVisible),
    verificationChallengeVisible: Boolean(pageSignal?.verificationChallengeVisible),
    marketingVisible: Boolean(pageSignal?.marketingVisible),
  };
}

function isConcreteGeminiAppUrl(url) {
  const normalized = normalizeString(url);
  if (!normalized) {
    return false;
  }
  try {
    const parsed = new URL(normalized);
    const pathname = parsed.pathname.replace(/\/+$/, "").toLowerCase();
    return /^\/(?:u\/\d+\/)?app\/[^/?#]+$/.test(pathname);
  } catch {
    return false;
  }
}

function canForceCompleteGeminiCapture({ url, cookies, pageSignal } = {}) {
  if (isGeminiAuthenticatedSignal({ url, cookies, pageSignal })) {
    return true;
  }
  return Boolean(
    pageSignal &&
      hasGoogleAuthCookies(cookies) &&
      (isGeminiSurfaceUrl(url) || pageSignal.hasGeminiSurface) &&
      !pageSignal.verificationChallengeVisible &&
      (
        pageSignal.hasNonDefaultAvatar ||
        pageSignal.hasAccountMenuButton ||
        isConcreteGeminiAppUrl(url)
      ),
  );
}

function pickGeminiCandidatePage(entries = []) {
  const ranked = [...entries].sort((left, right) => {
    const leftScore =
      (left.pageSignal?.hasConversationInput ? 8 : 0) +
      (left.pageSignal?.hasNonDefaultAvatar ? 4 : 0) +
      (left.pageSignal?.hasAccountMenuButton ? 2 : 0) +
      (left.pageSignal?.hasGeminiSurface ? 1 : 0);
    const rightScore =
      (right.pageSignal?.hasConversationInput ? 8 : 0) +
      (right.pageSignal?.hasNonDefaultAvatar ? 4 : 0) +
      (right.pageSignal?.hasAccountMenuButton ? 2 : 0) +
      (right.pageSignal?.hasGeminiSurface ? 1 : 0);
    return rightScore - leftScore;
  });
  return ranked[0] ?? null;
}

export {
  hasGoogleAuthCookies,
  isGoogleVerificationChallengeVisible,
  isGeminiSurfaceUrl,
  collectGeminiPageAuthSignal,
  isGeminiAuthenticatedSignal,
  summarizeGeminiPageSignal,
  canForceCompleteGeminiCapture,
  pickGeminiCandidatePage,
};
