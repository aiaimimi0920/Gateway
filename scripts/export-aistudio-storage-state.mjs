/**
 * One-shot manual helper for AI Studio credential refresh.
 *
 * Opens a visible isolated Edge/Chrome window, waits for a manual AI Studio
 * login, exports Playwright storageState into the gateway object storage, and
 * prints the resulting runtimeStateObjectKey for probe-aistudio-live-request.
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { S3Client, PutObjectCommand } from "@aws-sdk/client-s3";

const DEFAULT_TARGET_URL = "https://ai.studio/apps/fa9cb8e6-4d92-4fb6-a2b1-b947405c22ae";
const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const DEFAULT_ACCOUNT_LABEL = "manual-refresh";
const DEFAULT_LOCALE = "zh-CN";
const HELP_FLAGS = new Set(["--help", "-h"]);

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

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
];
const MACOS_BROWSER_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];
const LINUX_BROWSER_PATHS = [
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];

let objectStorageClient = null;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function isHelpRequested(argv = []) {
  return Array.isArray(argv) && argv.some((arg) => HELP_FLAGS.has(arg));
}

function buildUsageText() {
  return [
    "Usage: node scripts/export-aistudio-storage-state.mjs",
    "",
    "Opens an isolated visible Edge/Chrome window, waits for manual AI Studio login,",
    "and writes a Playwright storageState JSON object into gateway object storage.",
    "",
    "Environment variables:",
    "  AISTUDIO_EXPORT_BROWSER_EXECUTABLE_PATH  Browser executable override.",
    "  AISTUDIO_BROWSER_EXECUTABLE_PATH         Shared AIStudio browser executable override.",
    "  AISTUDIO_EXPORT_TARGET_URL               AIStudio app URL to open.",
    "  AISTUDIO_APP_URL                         Shared AIStudio app URL fallback.",
    "  AISTUDIO_EXPORT_TIMEOUT_MS               Manual-login wait timeout in milliseconds.",
    "  AISTUDIO_EXPORT_LOCALE                   Browser locale, default zh-CN.",
    "  AISTUDIO_EXPORT_ACCOUNT_LABEL            Label used in generated object key.",
    "  AISTUDIO_EXPORT_OBJECT_KEY                Explicit object key or object-key prefix.",
    "",
    "Object storage uses AI_GATEWAY_OBJECT_STORAGE_* or OBJECT_STORAGE_* env vars;",
    "without remote settings it writes under .runtime/ai-gateway-objects.",
  ].join("\n");
}

function timestampTag(now = new Date()) {
  const date = now instanceof Date ? now : new Date(now);
  const yyyy = date.getUTCFullYear();
  const mm = String(date.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(date.getUTCDate()).padStart(2, "0");
  const hh = String(date.getUTCHours()).padStart(2, "0");
  const mi = String(date.getUTCMinutes()).padStart(2, "0");
  const ss = String(date.getUTCSeconds()).padStart(2, "0");
  return `${yyyy}${mm}${dd}-${hh}${mi}${ss}`;
}

function slugify(value) {
  return String(value || "")
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

function assertSafeObjectKey(objectKey) {
  if (
    objectKey.includes("\\") ||
    path.posix.isAbsolute(objectKey) ||
    path.win32.isAbsolute(objectKey) ||
    /^[a-zA-Z]:/.test(objectKey)
  ) {
    throw new Error(`Unsafe AIStudio storage-state object key: ${objectKey}`);
  }
  const segments = objectKey.split("/");
  if (
    segments.some(
      (segment) => !segment || segment === "." || segment === "..",
    )
  ) {
    throw new Error(`Unsafe AIStudio storage-state object key: ${objectKey}`);
  }
  return objectKey;
}

function buildAistudioStorageStateObjectKey({
  accountLabel = DEFAULT_ACCOUNT_LABEL,
  now = new Date(),
  objectKey = null,
} = {}) {
  const explicitObjectKey = normalizeString(objectKey);
  if (explicitObjectKey) {
    const resolvedObjectKey = explicitObjectKey.endsWith("/storage-state.json")
      ? explicitObjectKey
      : `${explicitObjectKey.replace(/\/+$/, "")}/storage-state.json`;
    return assertSafeObjectKey(resolvedObjectKey);
  }
  const label = slugify(accountLabel) || DEFAULT_ACCOUNT_LABEL;
  return assertSafeObjectKey(
    `credential-runtime/aistudio-web/${label}-${timestampTag(now)}/storage-state.json`,
  );
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

function getStorageRoot(config) {
  return path.resolve(process.cwd(), config.localDir);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("Remote object storage is not fully configured.");
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

async function putObject(objectKey, body) {
  const config = resolveObjectStorageConfig();
  if (config.driver === "local") {
    const absolutePath = path.join(getStorageRoot(config), ...objectKey.split("/"));
    await mkdir(path.dirname(absolutePath), { recursive: true });
    await writeFile(absolutePath, body);
    return absolutePath;
  }

  const client = getS3Client(config);
  await client.send(
    new PutObjectCommand({
      Bucket: config.bucket,
      Key: objectKey,
      Body: body,
      ContentType: "application/json",
    }),
  );
  return null;
}

function isGoogleCookieDomain(domain) {
  const normalized = normalizeString(domain)?.toLowerCase().replace(/^\./, "");
  return Boolean(
    normalized &&
      (normalized === "google.com" || normalized.endsWith(".google.com")),
  );
}

function hasGoogleAuthCookies(cookies) {
  return Array.isArray(cookies) && cookies.some(
    (cookie) =>
      AUTH_COOKIE_NAMES.has(cookie.name) &&
      isGoogleCookieDomain(cookie.domain),
  );
}

function isAistudioSurfaceUrl(url) {
  const normalized = normalizeString(url)?.toLowerCase() ?? "";
  return (
    normalized.startsWith("https://ai.studio/") ||
    normalized.startsWith("https://aistudio.google.com/") ||
    normalized.startsWith("https://makersuite.google.com/")
  );
}

function isAistudioAuthenticatedSignal({ url, cookies, pageSignal } = {}) {
  return Boolean(
    isAistudioSurfaceUrl(url) &&
      hasGoogleAuthCookies(cookies) &&
      pageSignal &&
      !pageSignal.loginVisible &&
      (pageSignal.hasAistudioSurface || pageSignal.hasPromptInput),
  );
}

async function collectAistudioPageAuthSignal(page) {
  return await page.evaluate(() => {
    const bodyText = document.body?.innerText ?? "";
    const loweredBody = bodyText.toLowerCase();
    const hasPromptInput = Boolean(
      document.querySelector("textarea, [role='textbox'], rich-textarea, input[type='text']"),
    );
    const loginVisible =
      /(^|\n)\s*(login|log in|sign in|登录|登入)(\s|$)/i.test(bodyText) ||
      Boolean(
        document.querySelector(
          "a[href*='accounts.google.com'], a[href*='signin'], button[aria-label*='登录'], button[aria-label*='Sign in']",
        ),
      );
    const hasAistudioSurface =
      loweredBody.includes("ai studio") ||
      loweredBody.includes("google ai studio") ||
      loweredBody.includes("build with gemini") ||
      loweredBody.includes("makersuite") ||
      location.hostname === "ai.studio" ||
      location.hostname === "aistudio.google.com" ||
      location.hostname === "makersuite.google.com";
    return {
      bodyText,
      hasPromptInput,
      hasAistudioSurface,
      loginVisible,
      url: location.href,
      title: document.title,
    };
  });
}

function isTransientNavigationError(error) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    message.includes("Execution context was destroyed") ||
    message.includes("Target page, context or browser has been closed") ||
    message.includes("Cannot find context with specified id")
  );
}

async function waitForManualLogin({ page, context, timeoutMs, targetUrl }) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const url = page.url();
      const cookies = await context.cookies([
        "https://ai.studio",
        "https://aistudio.google.com",
        "https://makersuite.google.com",
        "https://accounts.google.com",
        "https://google.com",
        "https://www.google.com",
      ]);
      const pageSignal = await collectAistudioPageAuthSignal(page);

      if (isAistudioAuthenticatedSignal({ url, cookies, pageSignal })) {
        return { url, cookies, pageSignal };
      }

      if (isAistudioSurfaceUrl(url) && normalizeString(targetUrl) && url !== targetUrl) {
        try {
          await page.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: 30_000 });
        } catch {
          // Ignore and keep polling.
        }
      }
    } catch (error) {
      if (!isTransientNavigationError(error)) {
        throw error;
      }
    }

    await page.waitForTimeout(2_000).catch(() => undefined);
  }

  throw new Error(
    "Timed out waiting for manual AI Studio login. Complete login in the opened browser window and keep the page on ai.studio.",
  );
}

function isMainModule() {
  return process.argv[1]
    ? import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
    : false;
}

async function main() {
  if (isHelpRequested(process.argv.slice(2))) {
    console.log(buildUsageText());
    return;
  }

  const executablePath = resolveExecutablePath(
    process.env.AISTUDIO_EXPORT_BROWSER_EXECUTABLE_PATH ??
      process.env.AISTUDIO_BROWSER_EXECUTABLE_PATH ??
      null,
  );
  if (!executablePath) {
    throw new Error(
      "Unable to locate Edge/Chrome automatically. Set AISTUDIO_EXPORT_BROWSER_EXECUTABLE_PATH.",
    );
  }

  const targetUrl =
    normalizeString(process.env.AISTUDIO_EXPORT_TARGET_URL) ??
    normalizeString(process.env.AISTUDIO_APP_URL) ??
    DEFAULT_TARGET_URL;
  const timeoutMs = Number(process.env.AISTUDIO_EXPORT_TIMEOUT_MS || DEFAULT_TIMEOUT_MS);
  const locale = normalizeString(process.env.AISTUDIO_EXPORT_LOCALE) ?? DEFAULT_LOCALE;
  const accountLabel =
    normalizeString(process.env.AISTUDIO_EXPORT_ACCOUNT_LABEL) ?? DEFAULT_ACCOUNT_LABEL;
  const objectKey = buildAistudioStorageStateObjectKey({
    accountLabel,
    objectKey: process.env.AISTUDIO_EXPORT_OBJECT_KEY,
  });

  console.log(`[aistudio-export] Opening browser at ${targetUrl}`);
  console.log("[aistudio-export] Complete AI Studio login in the opened browser window.");
  console.log("[aistudio-export] The script will auto-detect success and export storageState.");

  const browser = await chromium.launch({
    executablePath,
    headless: false,
    args: [
      "--disable-blink-features=AutomationControlled",
      "--disable-dev-shm-usage",
      "--no-first-run",
      "--no-default-browser-check",
      "--start-maximized",
    ],
  });

  try {
    const context = await browser.newContext({
      viewport: null,
      locale,
      ignoreHTTPSErrors: true,
      bypassCSP: true,
    });
    const page = await context.newPage();
    await page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });

    const state = await waitForManualLogin({
      page,
      context,
      timeoutMs,
      targetUrl,
    });

    const storageState = await context.storageState();
    const storageBuffer = Buffer.from(JSON.stringify(storageState, null, 2), "utf8");
    const absolutePath = await putObject(objectKey, storageBuffer);

    const output = {
      ok: true,
      runtimeStateObjectKey: objectKey,
      absolutePath,
      currentUrl: state.url,
      cookieCount: state.cookies.length,
      pageTitle: state.pageSignal?.title ?? null,
      hasPromptInput: state.pageSignal?.hasPromptInput ?? false,
      hasAistudioSurface: state.pageSignal?.hasAistudioSurface ?? false,
      targetUrl,
      note: "Generated AI Studio storageState for probe-aistudio-live-request.",
    };

    console.log(JSON.stringify(output, null, 2));
  } finally {
    await browser.close().catch(() => undefined);
  }
}

export {
  buildAistudioStorageStateObjectKey,
  buildUsageText,
  hasGoogleAuthCookies,
  isHelpRequested,
  isAistudioAuthenticatedSignal,
  isAistudioSurfaceUrl,
  slugify,
  timestampTag,
};

if (isMainModule()) {
  main().catch((error) => {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        },
        null,
        2,
      ),
    );
    process.exit(1);
  });
}
