import { normalizeString } from "./configuration.mjs";

export async function collectPageState(page) {
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

export function pageStateIsCloudflareWait(pageState) {
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

export function compactPageState(pageState) {
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

export function pageStateRequiresEmailVerification(pageState) {
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

export function pageStateRequiresAuthRecovery(pageState) {
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

export function safePageUrl(page) {
  try {
    return normalizeString(page.url()) ?? "";
  } catch {
    return "";
  }
}
