import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

const DEFAULT_TARGET_URL = "https://www.udio.com/create";
const DEFAULT_STORAGE_STATE_PATH = ".runtime/udio-manual-browser-helper.storage-state.json";
const DEFAULT_STATUS_PATH = ".runtime/udio-manual-browser-helper.status.json";
const DEFAULT_OBJECT_KEY = "credential-runtime/udio/manual-browser-helper/storage-state.json";
const DEFAULT_LOCALE = "en-US";
const DEFAULT_PROFILE_DIRECTORY = "Default";

const UDIO_AUTH_COOKIE_PREFIX = "sb-ssr-production-auth-token";

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
];

let objectStorageClient = null;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
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

function resolveExecutablePath() {
  const configured = normalizeString(
    process.env.UDIO_MANUAL_HELPER_BROWSER_EXECUTABLE_PATH ??
      process.env.UDIO_BROWSER_EXECUTABLE_PATH ??
      null,
  );
  if (configured && existsSync(configured)) {
    return configured;
  }
  return WINDOWS_BROWSER_PATHS.find((entry) => existsSync(entry)) ?? null;
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

async function readPageSnapshot(page) {
  const fallback = {
    title: await page.title().catch(() => null),
    currentUrl: page.url() || null,
  };
  try {
    return await page.evaluate(() => ({
      title: typeof document?.title === "string" ? document.title : null,
      currentUrl: typeof location?.href === "string" ? location.href : null,
    }));
  } catch {
    return fallback;
  }
}

async function main() {
  const cloneRoot = normalizeString(process.env.UDIO_MANUAL_EXPORT_PROFILE_ROOT);
  if (!cloneRoot || !existsSync(cloneRoot)) {
    throw new Error("UDIO_MANUAL_EXPORT_PROFILE_ROOT is missing or does not exist.");
  }

  const executablePath = resolveExecutablePath();
  if (!executablePath) {
    throw new Error("Unable to locate Edge/Chrome automatically for Udio export.");
  }

  const profileDirectory =
    normalizeString(process.env.UDIO_MANUAL_EXPORT_PROFILE_DIRECTORY) ?? DEFAULT_PROFILE_DIRECTORY;
  const targetUrl =
    normalizeString(process.env.UDIO_MANUAL_EXPORT_TARGET_URL) ?? DEFAULT_TARGET_URL;
  const storageStatePath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_STORAGE_STATE_PATH) ??
    DEFAULT_STORAGE_STATE_PATH;
  const statusPath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_STATUS_PATH) ?? DEFAULT_STATUS_PATH;
  const objectKey = parseBoolean(process.env.UDIO_MANUAL_HELPER_DISABLE_OBJECT_SYNC, false)
    ? null
    : normalizeString(process.env.UDIO_MANUAL_HELPER_OBJECT_KEY) ?? DEFAULT_OBJECT_KEY;

  const context = await chromium.launchPersistentContext(cloneRoot, {
    executablePath,
    headless: parseBoolean(process.env.UDIO_MANUAL_EXPORT_HEADLESS, true),
    viewport: null,
    locale: DEFAULT_LOCALE,
    args: [
      "--disable-blink-features=AutomationControlled",
      "--disable-dev-shm-usage",
      "--no-first-run",
      "--no-default-browser-check",
      `--profile-directory=${profileDirectory}`,
    ],
  });

  try {
    const page = context.pages()[0] ?? (await context.newPage());
    await page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    }).catch(() => undefined);
    await page.waitForTimeout(2_000);

    const cookies = await context.cookies(["https://www.udio.com", "https://udio.com"]);
    const authenticated = hasUdioAuthCookies(cookies);
    const cookieHeader = buildCookieHeader(cookies);
    const pageSnapshot = await readPageSnapshot(page);
    const storageState = await context.storageState();
    const storageText = JSON.stringify(storageState, null, 2);

    await ensureParentDir(storageStatePath);
    await writeFile(path.resolve(process.cwd(), storageStatePath), storageText, "utf8");

    let runtimeStateAbsolutePath = null;
    if (authenticated && objectKey) {
      runtimeStateAbsolutePath = await putObject(objectKey, Buffer.from(storageText, "utf8"));
    }

    const payload = {
      ok: true,
      pid: process.pid,
      updatedAt: new Date().toISOString(),
      phase: "manual_login_exported",
      authenticated,
      targetUrl,
      currentUrl: pageSnapshot.currentUrl,
      title: pageSnapshot.title,
      cookieCount: cookies.length,
      cookieHeader,
      storageStatePath: path.resolve(process.cwd(), storageStatePath),
      runtimeStateObjectKey: objectKey,
      runtimeStateAbsolutePath,
      cloneRoot,
      profileDirectory,
      note: "Exported Udio session material from the native cloned browser profile.",
    };

    await writeJsonFile(statusPath, payload);
    process.stdout.write(`${JSON.stringify(payload)}\n`);
  } finally {
    await context.close().catch(() => undefined);
  }
}

main().catch(async (error) => {
  const statusPath =
    normalizeString(process.env.UDIO_MANUAL_HELPER_STATUS_PATH) ?? DEFAULT_STATUS_PATH;
  const payload = {
    ok: false,
    pid: process.pid,
    updatedAt: new Date().toISOString(),
    phase: "manual_login_export_failed",
    error: {
      message: typeof error?.message === "string" ? error.message : String(error),
      stack: typeof error?.stack === "string" ? error.stack : null,
    },
  };
  await writeJsonFile(statusPath, payload).catch(() => undefined);
  process.stderr.write(`${JSON.stringify(payload)}\n`);
  process.exitCode = 1;
});
