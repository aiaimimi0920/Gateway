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
import { S3Client, PutObjectCommand } from "@aws-sdk/client-s3";

const DEFAULT_BASE_URL = "https://gemini.google.com";
const DEFAULT_TARGET_PATH = "/app";
const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const DEFAULT_ACCOUNT_LABEL = "manual-test";
const DEFAULT_SHARE_ID = "fe24c455a570";
const DEFAULT_OUTPUT_MODE = "storage_state";

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

let objectStorageClient = null;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function timestampTag() {
  const now = new Date();
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  const hh = String(now.getHours()).padStart(2, "0");
  const mi = String(now.getMinutes()).padStart(2, "0");
  const ss = String(now.getSeconds()).padStart(2, "0");
  return `${yyyy}${mm}${dd}-${hh}${mi}${ss}`;
}

function slugify(value) {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "");
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
  return cookies.some(
    (cookie) =>
      AUTH_COOKIE_NAMES.has(cookie.name) &&
      typeof cookie.domain === "string" &&
      cookie.domain.includes("google.com"),
  );
}

async function collectGeminiPageAuthSignal(page) {
  return await page.evaluate(() => {
    const bodyText = document.body?.innerText ?? "";
    const loweredBody = bodyText.toLowerCase();
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
    return {
      bodyText,
      avatarSources,
      hasNonDefaultAvatar,
      hasConversationInput,
      loginVisible,
      marketingVisible,
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
      const lowered = url.toLowerCase();
      const cookies = await context.cookies([
        "https://gemini.google.com",
        "https://accounts.google.com",
        "https://google.com",
        "https://www.google.com",
      ]);

      const authenticatedHost =
        lowered.startsWith("https://gemini.google.com") &&
        !lowered.includes("signin") &&
        !lowered.includes("servicelogin") &&
        !lowered.includes("accounts.google.com");

      const pageSignal = await collectGeminiPageAuthSignal(page);
      const authenticatedGeminiApp =
        authenticatedHost &&
        hasGoogleAuthCookies(cookies) &&
        !pageSignal.loginVisible &&
        (!pageSignal.marketingVisible || pageSignal.hasNonDefaultAvatar) &&
        (pageSignal.hasConversationInput || pageSignal.hasNonDefaultAvatar);

      if (authenticatedGeminiApp) {
        return { url, cookies, pageSignal };
      }

      if (lowered.startsWith("https://gemini.google.com") && !lowered.includes("/app")) {
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
    "Timed out waiting for manual Gemini login. Please complete login in the opened browser window and keep the page on gemini.google.com.",
  );
}

async function main() {
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

  console.log(`[gemini-canvas-export] Opening browser at ${targetUrl}`);
  console.log("[gemini-canvas-export] Complete Gemini login in the opened browser window.");
  console.log("[gemini-canvas-export] The script will auto-detect success and export storageState.");

  if (outputMode === "profile_dir") {
    const config = resolveObjectStorageConfig();
    if (config.driver !== "local") {
      throw new Error("profile_dir capture mode currently requires local object storage.");
    }
    const profileDir = path.join(getStorageRoot(config), ...objectKey.split("/"));
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
      locale: "en-US",
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
      outputMode,
      currentUrl: state.url,
      cookieCount: state.cookies.length,
      pageTitle: state.pageSignal?.title ?? null,
      hasNonDefaultAvatar: state.pageSignal?.hasNonDefaultAvatar ?? false,
      baseUrl,
      suggestedShareId:
        normalizeString(process.env.GEMINI_CANVAS_SHARE_ID) ?? DEFAULT_SHARE_ID,
      note: "Focused helper only. Generated storageState for local Gemini Canvas validation.",
    };

    console.log(JSON.stringify(output, null, 2));
  } finally {
    await browser.close().catch(() => undefined);
  }
}

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
