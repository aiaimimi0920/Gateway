import { existsSync } from "node:fs";
import process from "node:process";
import { normalizeString, normalizePathname } from "./request.mjs";

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

export function safePageUrl(page) {
  try {
    return page?.url?.() ?? null;
  } catch {
    return null;
  }
}

export function resolveBorrowedContext(browser, baseUrl, targetUrl, fallbackUrl) {
  const base = safeUrl(baseUrl);
  const targets = [safeUrl(targetUrl), safeUrl(fallbackUrl)]
    .filter((target) => target && base && target.origin === base.origin);
  const context = browser.contexts().find((candidate) => candidate.pages()
    .some((page) => targets.some((target) => pageMatchesTarget(page, target))));
  if (!context) {
    throw new Error("Udio CDP session does not contain the expected create page.");
  }
  // Never create a context or fall back to an unrelated shared account.
  return context;
}

export async function createWorkerPage(context, referer, navigationTimeoutMs) {
  const page = await context.newPage();
  await gotoUdio(page, referer, navigationTimeoutMs);
  await page.waitForTimeout(1_500);
  return page;
}

export async function resolveWorkerPage(context, options) {
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

export function safeUrl(value) {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

export function pageMatchesTarget(page, targetUrl) {
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

export function resolveExecutablePath(overridePath) {
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

export async function gotoUdio(page, url, timeoutMs) {
  await page.goto(url, {
    waitUntil: "domcontentloaded",
    timeout: timeoutMs,
  });
  await page.waitForLoadState("networkidle", {
    timeout: Math.min(timeoutMs, 10_000),
  }).catch(() => undefined);
}

export async function openChallengeSurface(page, baseUrl, referer, timeoutMs, options = {}) {
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
