/**
 * One-shot manual helper for Gemini Canvas testing and credential bootstrap.
 *
 * This script opens a visible Chromium/Edge browser, waits for a manual Gemini
 * login, then exports Playwright storageState into the gateway object storage
 * and prints the resulting runtimeStateObjectKey.
 *
 * This is a repo-side operator helper. It exists to bootstrap a
 * test credential for local validation.
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { S3Client, PutObjectCommand } from "@aws-sdk/client-s3";

const DEFAULT_BASE_URL = "https://gemini.google.com";
const DEFAULT_TARGET_PATH = "/app";
const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const DEFAULT_ACCOUNT_LABEL = "manual-test";
const DEFAULT_SHARE_ID = "fe24c455a570";
const DEFAULT_OUTPUT_MODE = "storage_state";
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

const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
];
const MACOS_EDGE_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];
const LINUX_EDGE_PATHS = [
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];

const GOOGLE_API_KEY_REGEX = /AIza[0-9A-Za-z_-]{20,}/g;

let objectStorageClient = null;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function isHelpRequested(argv = []) {
  return Array.isArray(argv) && argv.some((arg) => HELP_FLAGS.has(arg));
}

function buildUsageText() {
  return [
    "Usage: node scripts/export-gemini-canvas-storage-state.mjs",
    "",
    "Opens an isolated visible Edge/Chrome window, waits for manual Gemini login,",
    "and writes a Playwright storageState JSON object into gateway object storage.",
    "",
    "Environment variables:",
    "  GEMINI_CANVAS_CAPTURE_BROWSER_EXECUTABLE_PATH  Browser executable override.",
    "  GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH          Shared Gemini browser executable override.",
    "  GEMINI_CANVAS_CAPTURE_BASE_URL                 Gemini base URL override, default https://gemini.google.com.",
    "  GEMINI_CANVAS_CAPTURE_TIMEOUT_MS               Manual-login wait timeout in milliseconds.",
    "  GEMINI_CANVAS_CAPTURE_ACCOUNT_LABEL            Label used in generated object key.",
    "  GEMINI_CANVAS_CAPTURE_OBJECT_KEY               Explicit object key or object-key prefix.",
    "  GEMINI_CANVAS_CAPTURE_OUTPUT_MODE              storage_state or profile_dir.",
    "  GEMINI_CANVAS_SHARE_ID                         Suggested shareId stored beside the exported state.",
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

function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_EDGE_PATHS
      : process.platform === "darwin"
        ? MACOS_EDGE_PATHS
        : LINUX_EDGE_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

function resolveOptionalRepoFile(candidate) {
  const normalized = normalizeString(candidate);
  if (!normalized) {
    return null;
  }
  return path.isAbsolute(normalized) ? normalized : path.resolve(process.cwd(), normalized);
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

function pushUniqueString(values, candidate) {
  const normalized = normalizeString(candidate);
  if (!normalized || values.includes(normalized)) {
    return;
  }
  values.push(normalized);
}

function extractGoogleApiKeysFromBlob(blob) {
  const text = String(blob ?? "");
  const matches = text.match(GOOGLE_API_KEY_REGEX) ?? [];
  const keys = [];
  for (const candidate of matches) {
    pushUniqueString(keys, candidate);
  }
  return keys;
}

async function collectGeminiPageRuntimeArtifacts(page) {
  const blobs = [];
  try {
    blobs.push(await page.content());
  } catch (error) {
    if (!isTransientNavigationError(error)) {
      throw error;
    }
  }
  try {
    const serializedRuntime = await page.evaluate(() =>
      JSON.stringify({
        firebaseConfig: globalThis.firebaseConfig ?? null,
        WIZ_global_data: globalThis.WIZ_global_data ?? null,
        __NEXT_DATA__: globalThis.__NEXT_DATA__ ?? null,
        APP_INITIALIZATION_STATE: globalThis.APP_INITIALIZATION_STATE ?? null,
      }),
    );
    blobs.push(serializedRuntime);
  } catch (error) {
    if (!isTransientNavigationError(error)) {
      throw error;
    }
  }

  const apiKeys = [];
  for (const blob of blobs) {
    for (const candidate of extractGoogleApiKeysFromBlob(blob)) {
      pushUniqueString(apiKeys, candidate);
    }
  }
  return {
    apiKeys,
  };
}

async function collectGeminiRuntimeMaterial(context, fallbackPage, preferredUrl) {
  const pageEntries = await inspectGeminiPages(context, fallbackPage);
  const preferred = normalizeString(preferredUrl);
  const preferredIndex = pageEntries.findIndex(
    (entry) => normalizeString(entry.url) === preferred,
  );
  const orderedEntries =
    preferredIndex >= 0
      ? [
          pageEntries[preferredIndex],
          ...pageEntries.filter((_, index) => index !== preferredIndex),
        ]
      : pageEntries;
  const apiKeys = [];

  for (const entry of orderedEntries) {
    const artifacts = await collectGeminiPageRuntimeArtifacts(entry.page);
    for (const candidate of artifacts.apiKeys) {
      pushUniqueString(apiKeys, candidate);
    }
  }

  return {
    apiKeys,
  };
}

async function inspectGeminiPages(context, fallbackPage) {
  const pages = context.pages();
  const seen = new Set();
  const entries = [];
  for (const page of [...pages, fallbackPage].filter(Boolean)) {
    if (seen.has(page)) {
      continue;
    }
    seen.add(page);
    try {
      const url = page.url();
      const pageSignal = await collectGeminiPageAuthSignal(page);
      entries.push({
        page,
        url,
        pageSignal,
      });
    } catch (error) {
      if (!isTransientNavigationError(error)) {
        throw error;
      }
    }
  }
  return entries;
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
  let lastObservedPages = [];
  const forceCompleteFile = resolveOptionalRepoFile(
    process.env.GEMINI_CANVAS_CAPTURE_FORCE_COMPLETE_FILE ?? null,
  );
  while (Date.now() < deadline) {
    try {
      const cookies = await context.cookies([
        "https://gemini.google.com",
        "https://accounts.google.com",
        "https://google.com",
        "https://www.google.com",
      ]);
      const pageEntries = await inspectGeminiPages(context, page);
      lastObservedPages = pageEntries.map((entry) => ({
        url: normalizeString(entry.url) ?? null,
        pageSignal: summarizeGeminiPageSignal(entry.pageSignal),
      }));
      const authenticatedEntry = pageEntries.find((entry) =>
        isGeminiAuthenticatedSignal({
          url: entry.url,
          cookies,
          pageSignal: entry.pageSignal,
        }),
      );
      if (authenticatedEntry) {
        return {
          url: authenticatedEntry.url,
          cookies,
          pageSignal: authenticatedEntry.pageSignal,
        };
      }
      const candidateEntry =
        pickGeminiCandidatePage(
          pageEntries.filter((entry) => isGeminiSurfaceUrl(entry.url)),
        ) ??
        pickGeminiCandidatePage(
          pageEntries.filter((entry) => entry.pageSignal?.hasGeminiSurface),
        );
      if (
        forceCompleteFile &&
        existsSync(forceCompleteFile) &&
        candidateEntry &&
        canForceCompleteGeminiCapture({
          url: candidateEntry.url,
          cookies,
          pageSignal: candidateEntry.pageSignal,
        })
      ) {
        return {
          url: candidateEntry.url,
          cookies,
          pageSignal: candidateEntry.pageSignal,
        };
      }
      const navigationPage = candidateEntry?.page ?? page;
      const navigationUrl = normalizeString(candidateEntry?.url ?? navigationPage?.url?.())?.toLowerCase() ?? "";
      if (isGeminiSurfaceUrl(navigationUrl) && !navigationUrl.includes("/app")) {
        try {
          await navigationPage.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: 30_000 });
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

  const challengeDetected = lastObservedPages.some(
    (entry) => entry?.pageSignal?.verificationChallengeVisible,
  );
  if (challengeDetected) {
    throw new Error(
      `Google account verification is still open in the helper browser. Complete the verification challenge first, then keep the page on gemini.google.com. lastObservedPages=${JSON.stringify(lastObservedPages)}`,
    );
  }

  throw new Error(
    `Timed out waiting for manual Gemini login. Please complete login in the opened browser window and keep the page on gemini.google.com. lastObservedPages=${JSON.stringify(lastObservedPages)}`,
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
    process.env.GEMINI_CANVAS_CAPTURE_BROWSER_EXECUTABLE_PATH ??
      process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH ??
      null,
  );
  if (!executablePath) {
    throw new Error(
      "Unable to locate Edge/Chrome automatically. Set GEMINI_CANVAS_CAPTURE_BROWSER_EXECUTABLE_PATH.",
    );
  }

  const baseUrl = normalizeString(process.env.GEMINI_CANVAS_CAPTURE_BASE_URL) ?? DEFAULT_BASE_URL;
  const targetUrl = `${baseUrl.replace(/\/+$/, "")}${DEFAULT_TARGET_PATH}`;
  const timeoutMs = Number(process.env.GEMINI_CANVAS_CAPTURE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS);
  const accountLabel = slugify(
    normalizeString(process.env.GEMINI_CANVAS_CAPTURE_ACCOUNT_LABEL) ?? DEFAULT_ACCOUNT_LABEL,
  );
  const outputMode =
    normalizeString(process.env.GEMINI_CANVAS_CAPTURE_OUTPUT_MODE)?.toLowerCase() ??
    DEFAULT_OUTPUT_MODE;
  if (!["storage_state", "profile_dir"].includes(outputMode)) {
    throw new Error(
      "GEMINI_CANVAS_CAPTURE_OUTPUT_MODE must be either 'storage_state' or 'profile_dir'.",
    );
  }
  const baseObjectKey =
    normalizeString(process.env.GEMINI_CANVAS_CAPTURE_OBJECT_KEY) ??
    `credential-runtime/gemini-canvas/${accountLabel}-${timestampTag()}`;
  const objectKey =
    outputMode === "profile_dir" ? baseObjectKey : `${baseObjectKey}/storage-state.json`;
  const objectStorageConfig = resolveObjectStorageConfig();

  console.log(`[gemini-canvas-export] Opening browser at ${targetUrl}`);
  console.log("[gemini-canvas-export] Complete Gemini login in the opened browser window.");
  console.log("[gemini-canvas-export] The script will auto-detect success and export storageState.");

  if (outputMode === "profile_dir") {
    if (objectStorageConfig.driver !== "local") {
      throw new Error("profile_dir capture mode currently requires local object storage.");
    }
    const profileDir = path.join(getStorageRoot(objectStorageConfig), ...objectKey.split("/"));
    await mkdir(profileDir, { recursive: true });
    const context = await chromium.launchPersistentContext(profileDir, {
      executablePath,
      headless: false,
      locale: "en-US",
      viewport: null,
      ignoreHTTPSErrors: true,
      bypassCSP: true,
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--no-first-run",
        "--no-default-browser-check",
        "--start-maximized",
        "--ignore-certificate-errors",
        "--allow-insecure-localhost",
      ],
    });
    try {
      const page = context.pages()[0] ?? (await context.newPage());
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
      const runtimeMaterial = await collectGeminiRuntimeMaterial(
        context,
        page,
        state.url,
      );

      const output = {
        ok: true,
        runtimeStateObjectKey: objectKey,
        absolutePath: profileDir,
        outputMode,
        currentUrl: state.url,
        cookieCount: state.cookies.length,
        pageTitle: state.pageSignal?.title ?? null,
        hasNonDefaultAvatar: state.pageSignal?.hasNonDefaultAvatar ?? false,
        baseUrl,
        apiKeys: runtimeMaterial.apiKeys,
        suggestedShareId:
          normalizeString(process.env.GEMINI_CANVAS_SHARE_ID) ?? DEFAULT_SHARE_ID,
        note: "Focused helper only. Generated persistent browser profile directory for local Gemini Canvas validation.",
      };

      console.log(JSON.stringify(output, null, 2));
    } finally {
      await context.close().catch(() => undefined);
    }
    return;
  }

  const browserRuntimeStateObjectKey =
    objectStorageConfig.driver === "local" ? baseObjectKey : null;
  const profileDir = browserRuntimeStateObjectKey
    ? path.join(getStorageRoot(objectStorageConfig), ...browserRuntimeStateObjectKey.split("/"))
    : null;
  const context = profileDir
    ? await chromium.launchPersistentContext(profileDir, {
        executablePath,
        headless: false,
        locale: "en-US",
        viewport: null,
        ignoreHTTPSErrors: true,
        bypassCSP: true,
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
          "--start-maximized",
          "--ignore-certificate-errors",
          "--allow-insecure-localhost",
        ],
      })
    : await (async () => {
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
        const ephemeralContext = await browser.newContext({
          viewport: null,
          locale: "en-US",
          ignoreHTTPSErrors: true,
          bypassCSP: true,
        });
        ephemeralContext.__owningBrowser = browser;
        return ephemeralContext;
      })();

  try {
    const page = context.pages?.()[0] ?? (await context.newPage());
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
    const runtimeMaterial = await collectGeminiRuntimeMaterial(
      context,
      page,
      state.url,
    );

    const storageState = await context.storageState();
    const storageBuffer = Buffer.from(JSON.stringify(storageState, null, 2), "utf8");
    const absolutePath = await putObject(objectKey, storageBuffer);

    const output = {
      ok: true,
      runtimeStateObjectKey: objectKey,
      browserRuntimeStateObjectKey: browserRuntimeStateObjectKey,
      absolutePath,
      outputMode,
      currentUrl: state.url,
      cookieCount: state.cookies.length,
      pageTitle: state.pageSignal?.title ?? null,
      pageSignal: summarizeGeminiPageSignal(state.pageSignal),
      hasGeminiSurface: state.pageSignal?.hasGeminiSurface ?? false,
      hasNonDefaultAvatar: state.pageSignal?.hasNonDefaultAvatar ?? false,
      baseUrl,
      apiKeys: runtimeMaterial.apiKeys,
      suggestedShareId:
        normalizeString(process.env.GEMINI_CANVAS_SHARE_ID) ?? DEFAULT_SHARE_ID,
      note: "Focused helper only. Generated storageState for local Gemini Canvas validation.",
    };

    console.log(JSON.stringify(output, null, 2));
  } finally {
    const owningBrowser = context.__owningBrowser ?? null;
    await context.close().catch(() => undefined);
    if (owningBrowser) {
      await owningBrowser.close().catch(() => undefined);
    }
  }
}

export {
  buildUsageText,
  canForceCompleteGeminiCapture,
  extractGoogleApiKeysFromBlob,
  hasGoogleAuthCookies,
  isGoogleVerificationChallengeVisible,
  isGeminiAuthenticatedSignal,
  isGeminiSurfaceUrl,
  isHelpRequested,
  pickGeminiCandidatePage,
  slugify,
  summarizeGeminiPageSignal,
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
