/**
 * Manual Udio browser helper.
 *
 * Purpose:
 * - open one visible browser window for manual login/challenge handling
 * - keep that browser alive until explicitly stopped
 * - continuously export current auth/storage state for the local gateway
 * - record the latest generate request payload seen from the same page context
 *
 * This helper intentionally does NOT auto-close after first success. Repeated
 * launcher calls should reuse the same helper process instead of spawning more
 * one-shot windows.
 */

import { chromium } from "playwright-core";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

const DEFAULT_TARGET_URL = "https://www.udio.com/create";
const DEFAULT_STORAGE_STATE_PATH = ".runtime/udio-manual-browser-helper.storage-state.json";
const DEFAULT_STATUS_PATH = ".runtime/udio-manual-browser-helper.status.json";
const DEFAULT_LOCK_PATH = ".runtime/udio-manual-browser-helper.lock.json";
const DEFAULT_GENERATE_CAPTURE_PATH = ".runtime/udio-manual-browser-helper.latest-generate.json";
const DEFAULT_CAPTCHA_TOKEN_OUTPUT_PATH = ".runtime/udio-live-captcha-token.json";
const DEFAULT_OBJECT_KEY = "credential-runtime/udio/manual-browser-helper/storage-state.json";
const DEFAULT_POLL_INTERVAL_MS = 10_000;
const DEFAULT_IDLE_TIMEOUT_MS = 12 * 60 * 60 * 1000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_ACCOUNT_LABEL = "manual-browser-helper";

const UDIO_AUTH_COOKIE_PREFIX = "sb-ssr-production-auth-token";

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
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
const DEFAULT_PROFILE_DIRECTORY = "Default";
const WINDOWS_EDGE_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Microsoft",
  "Edge",
  "User Data",
);
const WINDOWS_CHROME_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Google",
  "Chrome",
  "User Data",
);
const ROOT_FILES_TO_COPY = ["Local State"];
const PROFILE_FILES_TO_COPY = ["Preferences", "Secure Preferences"];
// Keep the clone scope intentionally narrow. For the manual helper we only
// need a browser environment close to the user's real profile plus cookies in
// the Network store; copying the full web app storage tree makes startup slow
// and increases popup/service-worker churn during OAuth/captcha flows.
const PROFILE_DIRS_TO_COPY = ["Network"];

let objectStorageClient = null;
let shuttingDown = false;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeTimeoutMs(value, fallback, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(Math.max(Math.trunc(parsed), min), max);
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

function resolveProfileSource() {
  const configuredUserDataDir =
    normalizeString(process.env.UDIO_MANUAL_HELPER_BROWSER_USER_DATA_DIR) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_USER_DATA_DIR) ??
    null;
  const profileDirectory =
    normalizeString(process.env.UDIO_MANUAL_HELPER_BROWSER_PROFILE_DIR) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_PROFILE_DIR) ??
    DEFAULT_PROFILE_DIRECTORY;

  if (configuredUserDataDir && existsSync(configuredUserDataDir)) {
    return {
      browser: inferBrowserName(configuredUserDataDir),
      userDataDir: configuredUserDataDir,
      profileDirectory,
    };
  }
  if (process.platform === "win32" && existsSync(WINDOWS_EDGE_USER_DATA_DIR)) {
    return {
      browser: "edge",
      userDataDir: WINDOWS_EDGE_USER_DATA_DIR,
      profileDirectory,
    };
  }
  if (process.platform === "win32" && existsSync(WINDOWS_CHROME_USER_DATA_DIR)) {
    return {
      browser: "chrome",
      userDataDir: WINDOWS_CHROME_USER_DATA_DIR,
      profileDirectory,
    };
  }
  return null;
}

function inferBrowserName(userDataDir) {
  const lower = userDataDir.toLowerCase();
  if (lower.includes("\\edge\\") || lower.includes("/edge/")) {
    return "edge";
  }
  if (lower.includes("\\chrome\\") || lower.includes("/chrome/")) {
    return "chrome";
  }
  return "chromium";
}

async function cloneBrowserProfile(userDataDir, profileDirectory) {
  const sourceProfileDir = path.join(userDataDir, profileDirectory);
  if (!existsSync(userDataDir) || !existsSync(sourceProfileDir)) {
    throw new Error(
      `Browser profile not found at ${sourceProfileDir}. Provide an existing signed-in Edge/Chrome profile.`,
    );
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "udio-manual-profile-"));
  await mkdir(tempRoot, { recursive: true });

  for (const fileName of ROOT_FILES_TO_COPY) {
    await copyIfExists(path.join(userDataDir, fileName), path.join(tempRoot, fileName));
  }

  const clonedProfileDir = path.join(tempRoot, profileDirectory);
  await mkdir(clonedProfileDir, { recursive: true });
  for (const fileName of PROFILE_FILES_TO_COPY) {
    await copyIfExists(path.join(sourceProfileDir, fileName), path.join(clonedProfileDir, fileName));
  }
  for (const directoryName of PROFILE_DIRS_TO_COPY) {
    await copyRecursive(
      path.join(sourceProfileDir, directoryName),
      path.join(clonedProfileDir, directoryName),
    );
  }

  return tempRoot;
}

async function copyIfExists(source, destination) {
  if (!existsSync(source)) {
    return;
  }
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    if (shouldIgnoreLockedProfileFile(error)) {
      return;
    }
    throw error;
  }
}

async function copyRecursive(source, destination) {
  if (!existsSync(source)) {
    return;
  }
  const sourceStat = await stat(source);
  if (sourceStat.isDirectory()) {
    await mkdir(destination, { recursive: true });
    const entries = await readdir(source, { withFileTypes: true });
    for (const entry of entries) {
      await copyRecursive(path.join(source, entry.name), path.join(destination, entry.name));
    }
    return;
  }
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    if (shouldIgnoreLockedProfileFile(error)) {
      return;
    }
    throw error;
  }
}

function shouldIgnoreLockedProfileFile(error) {
  const code = normalizeString(error?.code)?.toUpperCase();
  return code === "EBUSY" || code === "EPERM";
}

function sha1(value) {
  return createHash("sha1").update(value).digest("hex");
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
      String(
        process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
          process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
          "",
      )
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

async function ensureParentDir(filePath) {
  await mkdir(path.dirname(path.resolve(process.cwd(), filePath)), { recursive: true });
}

async function writeJsonFile(filePath, value) {
  await ensureParentDir(filePath);
  await writeFile(path.resolve(process.cwd(), filePath), JSON.stringify(value, null, 2), "utf8");
}

async function writeStatus(statusPath, partial) {
  await writeJsonFile(statusPath, {
    ok: true,
    pid: process.pid,
    updatedAt: new Date().toISOString(),
    startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT ?? new Date().toISOString(),
    targetUrl: globalThis.__UDIO_MANUAL_HELPER_TARGET_URL ?? DEFAULT_TARGET_URL,
    phase: globalThis.__UDIO_MANUAL_HELPER_PHASE ?? "starting",
    ...partial,
  });
}

async function maybeReadJsonFile(filePath) {
  const normalized = normalizeString(filePath);
  if (!normalized) {
    return null;
  }
  const absolutePath = path.resolve(process.cwd(), normalized);
  if (!existsSync(absolutePath)) {
    return null;
  }
  return JSON.parse(await readFile(absolutePath, "utf8"));
}

function hasUdioAuthCookies(cookies) {
  return cookies.some((cookie) => {
    const name = String(cookie?.name ?? "");
    return name === UDIO_AUTH_COOKIE_PREFIX || name.startsWith(`${UDIO_AUTH_COOKIE_PREFIX}.`);
  });
}

function buildCookieHeader(cookies) {
  return cookies
    .map((cookie) => {
      const name = normalizeString(cookie?.name);
      const value = normalizeString(cookie?.value);
      return name && value ? `${name}=${value}` : null;
    })
    .filter(Boolean)
    .join("; ");
}

function isProcessAlive(pid) {
  if (!Number.isFinite(pid) || pid <= 0) {
    return false;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function removeIfExists(filePath) {
  const absolutePath = path.resolve(process.cwd(), filePath);
  if (existsSync(absolutePath)) {
    await rm(absolutePath, { force: true }).catch(() => undefined);
  }
}

async function readAccountSnapshot(page) {
  const fallback = {
    title: await page.title().catch(() => null),
    currentUrl: page.url() || null,
    heading: null,
    accountName: null,
    accountEmail: null,
    localStorageKeys: [],
  };
  try {
    return await page.evaluate(() => {
      const trim = (value) => (typeof value === "string" && value.trim() ? value.trim() : null);
      const storageKeys = Object.keys(localStorage);
      let accountName = null;
      let accountEmail = null;
      for (const key of storageKeys) {
        const lowered = key.toLowerCase();
        if (!lowered.includes("auth") && !lowered.includes("user") && !lowered.includes("account")) {
          continue;
        }
        const raw = trim(localStorage.getItem(key));
        if (!raw) {
          continue;
        }
        try {
          const parsed = JSON.parse(raw);
          if (!parsed || typeof parsed !== "object") {
            continue;
          }
          accountName =
            trim(parsed.name) ??
            trim(parsed.full_name) ??
            trim(parsed.username) ??
            trim(parsed.user?.name) ??
            trim(parsed.user?.full_name) ??
            accountName;
          accountEmail =
            trim(parsed.email) ??
            trim(parsed.user?.email) ??
            trim(parsed.currentSession?.user?.email) ??
            trim(parsed.session?.user?.email) ??
            accountEmail;
        } catch {
          // Ignore non-JSON auth cache entries.
        }
      }
      return {
        title: trim(document.title),
        currentUrl: trim(location.href),
        heading:
          trim(document.querySelector("h1")?.textContent ?? null) ??
          trim(document.querySelector("[role='heading']")?.textContent ?? null),
        accountName,
        accountEmail,
        localStorageKeys: storageKeys,
      };
    });
  } catch {
    return fallback;
  }
}

async function exportAuthSnapshot({
  context,
  page,
  profileSource,
  storageStatePath,
  statusPath,
  objectKey,
  lastSnapshotHash,
  lastRemoteHash,
}) {
  const cookies = await context.cookies(["https://www.udio.com", "https://udio.com"]);
  const authenticated = hasUdioAuthCookies(cookies);
  const cookieHeader = buildCookieHeader(cookies);
  const pageSnapshot = await readAccountSnapshot(page);
  const storageState = await context.storageState();
  const storageText = JSON.stringify(storageState, null, 2);
  const storageHash = sha1(storageText);
  let runtimeStateAbsolutePath = null;

  if (authenticated && storageHash !== lastSnapshotHash.value) {
    await ensureParentDir(storageStatePath);
    await writeFile(path.resolve(process.cwd(), storageStatePath), storageText, "utf8");
    lastSnapshotHash.value = storageHash;
  }

  if (authenticated && objectKey && storageHash !== lastRemoteHash.value) {
    runtimeStateAbsolutePath = await putObject(objectKey, Buffer.from(storageText, "utf8"));
    lastRemoteHash.value = storageHash;
  }

  const status = {
    ok: true,
    pid: process.pid,
    startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT,
    updatedAt: new Date().toISOString(),
    authenticated,
    targetUrl: globalThis.__UDIO_MANUAL_HELPER_TARGET_URL,
    currentUrl: pageSnapshot.currentUrl,
    title: pageSnapshot.title,
    heading: pageSnapshot.heading,
    accountName: pageSnapshot.accountName,
    accountEmail: pageSnapshot.accountEmail,
    profileSource,
    cookieCount: cookies.length,
    cookieHeader,
    storageStatePath: path.resolve(process.cwd(), storageStatePath),
    runtimeStateObjectKey: objectKey,
    runtimeStateAbsolutePath,
    latestGenerateCapturePath: path.resolve(
      process.cwd(),
      globalThis.__UDIO_MANUAL_HELPER_GENERATE_CAPTURE_PATH,
    ),
  };

  await writeJsonFile(statusPath, status);
  return status;
}

async function main() {
  let clonedUserDataDir = null;
  const executablePath = resolveExecutablePath(
    process.env.UDIO_MANUAL_HELPER_BROWSER_EXECUTABLE_PATH ??
      process.env.UDIO_BROWSER_EXECUTABLE_PATH ??
      null,
  );
  if (!executablePath) {
    throw new Error(
      "Unable to locate Edge/Chrome automatically. Set UDIO_MANUAL_HELPER_BROWSER_EXECUTABLE_PATH.",
    );
  }

  const lockPath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_LOCK_PATH) ?? DEFAULT_LOCK_PATH;
  const statusPath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_STATUS_PATH) ?? DEFAULT_STATUS_PATH;
  const storageStatePath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_STORAGE_STATE_PATH) ??
    DEFAULT_STORAGE_STATE_PATH;
  const generateCapturePath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_GENERATE_CAPTURE_PATH) ??
    DEFAULT_GENERATE_CAPTURE_PATH;
  const captchaTokenOutputPath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_CAPTCHA_TOKEN_OUTPUT_PATH) ??
    DEFAULT_CAPTCHA_TOKEN_OUTPUT_PATH;
  let interceptGenerateOnce = parseBoolean(
    process.env.UDIO_MANUAL_HELPER_INTERCEPT_GENERATE_ONCE,
    false,
  );
  const pollIntervalMs = normalizeTimeoutMs(
    process.env.UDIO_MANUAL_HELPER_POLL_INTERVAL_MS,
    DEFAULT_POLL_INTERVAL_MS,
    1_000,
    30_000,
  );
  const idleTimeoutMs = normalizeTimeoutMs(
    process.env.UDIO_MANUAL_HELPER_IDLE_TIMEOUT_MS,
    DEFAULT_IDLE_TIMEOUT_MS,
    60_000,
    24 * 60 * 60 * 1000,
  );
  const locale =
    normalizeString(process.env.UDIO_MANUAL_HELPER_ACCEPT_LANGUAGE)?.split(",")[0]?.trim() ??
    DEFAULT_LOCALE;
  const targetUrl =
    normalizeString(process.env.UDIO_MANUAL_HELPER_TARGET_URL) ?? DEFAULT_TARGET_URL;
  const objectKey = parseBoolean(process.env.UDIO_MANUAL_HELPER_DISABLE_OBJECT_SYNC, false)
    ? null
    : normalizeString(process.env.UDIO_MANUAL_HELPER_OBJECT_KEY) ?? DEFAULT_OBJECT_KEY;

  globalThis.__UDIO_MANUAL_HELPER_STARTED_AT = new Date().toISOString();
  globalThis.__UDIO_MANUAL_HELPER_TARGET_URL = targetUrl;
  globalThis.__UDIO_MANUAL_HELPER_GENERATE_CAPTURE_PATH = generateCapturePath;
  globalThis.__UDIO_MANUAL_HELPER_PHASE = "starting";

  const existingLock = await maybeReadJsonFile(lockPath);
  if (existingLock?.pid && isProcessAlive(Number(existingLock.pid))) {
    process.stdout.write(
      JSON.stringify({
        ok: true,
        reused: true,
        pid: Number(existingLock.pid),
        statusPath: path.resolve(process.cwd(), statusPath),
        lockPath: path.resolve(process.cwd(), lockPath),
      }),
    );
    return;
  }

  await removeIfExists(lockPath);
  await writeStatus(statusPath, {
    authenticated: false,
    currentUrl: null,
    title: null,
    heading: null,
    accountName: null,
    accountEmail: null,
    cookieCount: 0,
    cookieHeader: "",
    storageStatePath: path.resolve(process.cwd(), storageStatePath),
    runtimeStateObjectKey: objectKey,
    runtimeStateAbsolutePath: null,
    latestGenerateCapturePath: path.resolve(process.cwd(), generateCapturePath),
    note: "Preparing the manual Udio browser helper.",
  });
  const profileSource = resolveProfileSource();
  if (!profileSource) {
    throw new Error(
      "Unable to locate a local Edge/Chrome user data directory for the Udio manual browser helper.",
    );
  }
  await writeJsonFile(lockPath, {
    ok: true,
    pid: process.pid,
    startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT,
    targetUrl,
    browserExecutablePath: executablePath,
    profileSource,
    accountLabel:
      normalizeString(process.env.UDIO_MANUAL_HELPER_ACCOUNT_LABEL) ?? DEFAULT_ACCOUNT_LABEL,
    statusPath: path.resolve(process.cwd(), statusPath),
    storageStatePath: path.resolve(process.cwd(), storageStatePath),
    latestGenerateCapturePath: path.resolve(process.cwd(), generateCapturePath),
  });
  globalThis.__UDIO_MANUAL_HELPER_PHASE = "cloning_profile";
  await writeStatus(statusPath, {
    profileSource,
    authenticated: false,
    currentUrl: null,
    title: null,
    heading: null,
    accountName: null,
    accountEmail: null,
    cookieCount: 0,
    cookieHeader: "",
    storageStatePath: path.resolve(process.cwd(), storageStatePath),
    runtimeStateObjectKey: objectKey,
    runtimeStateAbsolutePath: null,
    latestGenerateCapturePath: path.resolve(process.cwd(), generateCapturePath),
    note: "Cloning the local browser profile into a temporary helper profile.",
  });
  clonedUserDataDir = await cloneBrowserProfile(
    profileSource.userDataDir,
    profileSource.profileDirectory,
  );
  globalThis.__UDIO_MANUAL_HELPER_PHASE = "launching_browser";
  await writeStatus(statusPath, {
    profileSource,
    clonedUserDataDir,
    authenticated: false,
    currentUrl: null,
    title: null,
    heading: null,
    accountName: null,
    accountEmail: null,
    cookieCount: 0,
    cookieHeader: "",
    storageStatePath: path.resolve(process.cwd(), storageStatePath),
    runtimeStateObjectKey: objectKey,
    runtimeStateAbsolutePath: null,
    latestGenerateCapturePath: path.resolve(process.cwd(), generateCapturePath),
    note: "Launching the cloned-profile Edge helper window.",
  });
  const context = await chromium.launchPersistentContext(clonedUserDataDir, {
    executablePath,
    headless: false,
    viewport: null,
    locale,
    args: [
      "--disable-blink-features=AutomationControlled",
      "--disable-dev-shm-usage",
      "--no-first-run",
      "--no-default-browser-check",
      "--start-maximized",
      `--profile-directory=${profileSource.profileDirectory}`,
    ],
  });
  const page = context.pages()[0] ?? (await context.newPage());

  const writeLock = async () => {
    await writeJsonFile(lockPath, {
      ok: true,
      pid: process.pid,
      startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT,
      targetUrl,
      browserExecutablePath: executablePath,
      profileSource,
      accountLabel:
        normalizeString(process.env.UDIO_MANUAL_HELPER_ACCOUNT_LABEL) ?? DEFAULT_ACCOUNT_LABEL,
      statusPath: path.resolve(process.cwd(), statusPath),
      storageStatePath: path.resolve(process.cwd(), storageStatePath),
      latestGenerateCapturePath: path.resolve(process.cwd(), generateCapturePath),
    });
  };

  const lastSnapshotHash = { value: null };
  const lastRemoteHash = { value: null };
  let lastObservedActivityAt = Date.now();

  await context.route("**/api/generate-proxy", async (route) => {
    if (!interceptGenerateOnce) {
      await route.continue();
      return;
    }
    try {
      const request = route.request();
      if (request.method().toUpperCase() !== "POST") {
        await route.continue();
        return;
      }
      let payload = null;
      try {
        payload = request.postDataJSON();
      } catch {
        payload = null;
      }
      const captchaToken = normalizeString(payload?.captchaToken);
      if (!captchaToken) {
        await route.continue();
        return;
      }

      interceptGenerateOnce = false;
      lastObservedActivityAt = Date.now();
      const capture = {
        ok: true,
        capturedAt: new Date().toISOString(),
        requestUrl: request.url(),
        intercepted: true,
        message: "The captcha token was captured locally before the request reached Udio.",
        captchaToken,
        genParams: payload?.gen_params ?? null,
      };
      await writeJsonFile(generateCapturePath, capture);
      await writeJsonFile(captchaTokenOutputPath, capture);
      console.log("[udio-manual-helper] Captured a fresh captcha token before upstream send.");

      await route.fulfill({
        status: 409,
        contentType: "application/json",
        body: JSON.stringify({
          error: "intercepted_for_local_gateway_validation",
          message: capture.message,
        }),
      });
    } catch {
      await route.continue().catch(() => undefined);
    }
  });

  context.on("request", async (request) => {
    try {
      if (!request.url().includes("/api/generate-proxy")) {
        return;
      }
      if (request.method().toUpperCase() !== "POST") {
        return;
      }
      let payload = null;
      try {
        payload = request.postDataJSON();
      } catch {
        payload = null;
      }
      if (!payload || typeof payload !== "object") {
        return;
      }
      lastObservedActivityAt = Date.now();
      await writeJsonFile(generateCapturePath, {
        ok: true,
        capturedAt: new Date().toISOString(),
        requestUrl: request.url(),
        captchaToken: normalizeString(payload?.captchaToken),
        genParams: payload?.gen_params ?? null,
      });
    } catch {
      // Ignore auxiliary capture failures to keep the helper stable.
    }
  });

  const markActivity = () => {
    lastObservedActivityAt = Date.now();
  };
  page.on("framenavigated", markActivity);
  page.on("load", markActivity);
  page.on("domcontentloaded", markActivity);

  const beginShutdown = () => {
    shuttingDown = true;
  };
  process.on("SIGINT", beginShutdown);
  process.on("SIGTERM", beginShutdown);

  try {
    await page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });
    await page.bringToFront().catch(() => undefined);
    globalThis.__UDIO_MANUAL_HELPER_PHASE = "ready";
    await writeLock();

    console.log(`[udio-manual-helper] Opened visible browser at ${targetUrl}`);
    console.log("[udio-manual-helper] Reuse this helper instead of spawning new one-shot temp windows.");

    while (!shuttingDown) {
      const pages = context.pages().filter((entry) => !entry.isClosed());
      if (pages.length === 0) {
        break;
      }

      const activePage =
        pages.find((entry) => entry.url()?.includes("udio.com")) ??
        pages[0];
      const status = await exportAuthSnapshot({
        context,
        page: activePage,
        profileSource,
        storageStatePath,
        statusPath,
        objectKey,
        lastSnapshotHash,
        lastRemoteHash,
      });

      if (status.authenticated) {
        lastObservedActivityAt = Date.now();
      }

      if (Date.now() - lastObservedActivityAt > idleTimeoutMs) {
        console.log("[udio-manual-helper] Idle timeout reached, shutting down helper.");
        break;
      }

      await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
    }
  } finally {
    shuttingDown = true;
    globalThis.__UDIO_MANUAL_HELPER_PHASE = "shutting_down";
    await exportAuthSnapshot({
      context,
      page,
      profileSource,
      storageStatePath,
      statusPath,
      objectKey,
      lastSnapshotHash,
      lastRemoteHash,
    }).catch(() => undefined);
    await context.close().catch(() => undefined);
    if (clonedUserDataDir) {
      await rm(clonedUserDataDir, { recursive: true, force: true }).catch(() => undefined);
    }
    await removeIfExists(lockPath);
  }
}

main().catch(async (error) => {
  const statusPath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_STATUS_PATH) ?? DEFAULT_STATUS_PATH;
  globalThis.__UDIO_MANUAL_HELPER_PHASE = "failed";
  await writeJsonFile(statusPath, {
    ok: false,
    pid: process.pid,
    updatedAt: new Date().toISOString(),
    startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT ?? new Date().toISOString(),
    targetUrl: globalThis.__UDIO_MANUAL_HELPER_TARGET_URL ?? DEFAULT_TARGET_URL,
    phase: globalThis.__UDIO_MANUAL_HELPER_PHASE,
    error: {
      message: typeof error?.message === "string" ? error.message : String(error),
      stack: typeof error?.stack === "string" ? error.stack : null,
    },
  }).catch(() => undefined);
  console.error("[udio-manual-helper] Failed:", error);
  process.exitCode = 1;
});
