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
import { existsSync } from "node:fs";
import { rm } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { DEFAULT_TARGET_URL, normalizeString, normalizeTimeoutMs, parseBoolean } from "./udio-manual-browser/settings.mjs";
import { resolveExecutablePath, resolveProfileSource, cloneBrowserProfile } from "./udio-manual-browser/profile.mjs";
import { maybeReadJsonFile, writeJsonFile, writeStatus } from "./udio-manual-browser/persistence.mjs";
import { exportAuthSnapshot } from "./udio-manual-browser/snapshot.mjs";

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

let shuttingDown = false;

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
