/**
 * One-shot manual helper for Udio testing and credential bootstrap.
 *
 * This script opens a visible Chromium/Edge browser, waits for a manual Udio
 * login, then exports Playwright storageState into the gateway object storage
 * and prints the resulting runtimeStateObjectKey.
 *
 * This is a repo-side operator helper. It exists to bootstrap a
 * local test credential for real-provider verification.
 *
 * For a stable manual-login window that should stay open and be reused across
 * multiple interactions, prefer `gateway/scripts/udio-manual-browser-helper.mjs`
 * via `deploy/start-udio-manual-browser-helper.ps1`.
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

const DEFAULT_TARGET_URL = "https://www.udio.com/create";
const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const DEFAULT_ACCOUNT_LABEL = "manual-test";

const UDIO_AUTH_COOKIE_PREFIX = "sb-ssr-production-auth-token";

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

function hasUdioAuthCookies(cookies) {
  return cookies.some((cookie) => {
    const name = String(cookie?.name ?? "");
    return (
      name === UDIO_AUTH_COOKIE_PREFIX ||
      name.startsWith(`${UDIO_AUTH_COOKIE_PREFIX}.`)
    );
  });
}

async function waitForManualLogin({ page, context, timeoutMs }) {
  const deadline = Date.now() + timeoutMs;
  const probeUrls = ["https://www.udio.com", "https://udio.com"];

  while (Date.now() < deadline) {
    const url = page.url();
    const lowered = url.toLowerCase();
    const cookies = await context.cookies(probeUrls);
    const looksAuthenticatedHost =
      lowered.includes("udio.com") &&
      !lowered.includes("/login") &&
      !lowered.includes("/sign-in") &&
      !lowered.includes("/sign-up");

    if (looksAuthenticatedHost && hasUdioAuthCookies(cookies)) {
      return { url, cookies };
    }

    await page.waitForTimeout(2_000);
  }

  throw new Error(
    "Timed out waiting for manual Udio login. Please complete login in the opened browser window and keep the page on udio.com.",
  );
}

async function main() {
  const executablePath = resolveExecutablePath(
    process.env.UDIO_CAPTURE_BROWSER_EXECUTABLE_PATH ??
      process.env.UDIO_BROWSER_EXECUTABLE_PATH ??
      null,
  );
  if (!executablePath) {
    throw new Error(
      "Unable to locate Edge/Chrome automatically. Set UDIO_CAPTURE_BROWSER_EXECUTABLE_PATH.",
    );
  }

  const targetUrl = normalizeString(process.env.UDIO_CAPTURE_TARGET_URL) ?? DEFAULT_TARGET_URL;
  const timeoutMs = Number(process.env.UDIO_CAPTURE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS);
  const accountLabel = slugify(
    normalizeString(process.env.UDIO_CAPTURE_ACCOUNT_LABEL) ?? DEFAULT_ACCOUNT_LABEL,
  );
  const objectKey =
    normalizeString(process.env.UDIO_CAPTURE_OBJECT_KEY) ??
    `credential-runtime/udio/${accountLabel}-${timestampTag()}/storage-state.json`;

  console.log(`[udio-export] Opening browser at ${targetUrl}`);
  console.log(
    "[udio-export] One-shot helper only. For a reusable manual window, use deploy/start-udio-manual-browser-helper.ps1 instead.",
  );
  console.log("[udio-export] Complete Udio login in the opened browser window.");
  console.log("[udio-export] The script will auto-detect login and export storageState.");

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
    });
    const page = await context.newPage();
    await page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });
    await page.bringToFront().catch(() => undefined);

    const result = await waitForManualLogin({
      page,
      context,
      timeoutMs,
    });

    const storageState = await context.storageState();
    const storageBuffer = Buffer.from(JSON.stringify(storageState, null, 2), "utf8");
    const absolutePath = await putObject(objectKey, storageBuffer);

    const output = {
      ok: true,
      runtimeStateObjectKey: objectKey,
      absolutePath,
      currentUrl: result.url,
      cookieCount: result.cookies.length,
      note: "Focused Udio storage-state export only.",
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
        message: error instanceof Error ? error.message : String(error),
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
