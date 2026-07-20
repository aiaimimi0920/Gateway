/**
 * One-shot manual helper for Udio testing and credential bootstrap.
 *
 * This script opens a visible Chromium/Edge browser, optionally restores an
 * existing Playwright storageState, then waits for the user to solve any Udio
 * challenge and click generate once. The outgoing `POST /api/generate-proxy`
 * request is intercepted before it reaches Udio so the fresh request-scoped
 * `captchaToken` can be captured without consuming it upstream.
 *
 * This is a repo-side operator helper, not a request-time runtime path.
 *
 * For a stable manual-login window that should stay open and be reused across
 * multiple interactions, prefer `gateway/scripts/udio-manual-browser-helper.mjs`
 * via `deploy/start-udio-manual-browser-helper.ps1`.
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const DEFAULT_TARGET_URL = "https://www.udio.com/create";
const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const DEFAULT_TOKEN_OUTPUT_PATH = ".runtime/udio-live-captcha-token.json";
const DEFAULT_EXPORT_STORAGE_STATE_PATH = ".runtime/udio-live-post-captcha-storage-state.json";
const DEFAULT_STORAGE_STATE_PATH = ".runtime/udio-manual-post-challenge-storage-state.json";

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

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
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

async function ensureParentDir(filePath) {
  await mkdir(path.dirname(path.resolve(process.cwd(), filePath)), { recursive: true });
}

async function maybeReadStorageState(filePath) {
  const normalized = normalizeString(filePath);
  if (!normalized) {
    return undefined;
  }
  const absolutePath = path.resolve(process.cwd(), normalized);
  if (!existsSync(absolutePath)) {
    return undefined;
  }
  return JSON.parse(await readFile(absolutePath, "utf8"));
}

async function exportStorageState(context, filePath) {
  const normalized = normalizeString(filePath);
  if (!normalized) {
    return;
  }
  const absolutePath = path.resolve(process.cwd(), normalized);
  await ensureParentDir(absolutePath);
  const storageState = await context.storageState();
  await writeFile(absolutePath, JSON.stringify(storageState, null, 2), "utf8");
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
  const tokenOutputPath =
    normalizeString(process.env.UDIO_CAPTURE_TOKEN_OUTPUT_PATH) ?? DEFAULT_TOKEN_OUTPUT_PATH;
  const exportStorageStatePath =
    normalizeString(process.env.UDIO_CAPTURE_EXPORT_STORAGE_STATE_PATH) ??
    DEFAULT_EXPORT_STORAGE_STATE_PATH;
  const initialStorageStatePath =
    normalizeString(process.env.UDIO_CAPTURE_STORAGE_STATE_PATH) ?? DEFAULT_STORAGE_STATE_PATH;
  const initialStorageState = await maybeReadStorageState(initialStorageStatePath);

  console.log(`[udio-captcha] Opening browser at ${targetUrl}`);
  console.log(
    "[udio-captcha] One-shot helper only. For a reusable manual window, use deploy/start-udio-manual-browser-helper.ps1 instead.",
  );
  console.log(
    "[udio-captcha] Solve any Udio challenge and click generate once. The request will be intercepted before it reaches Udio so the captcha token stays unused.",
  );
  console.log(`[udio-captcha] Captured token will be written to ${tokenOutputPath}`);
  console.log(
    `[udio-captcha] Refreshed storageState will be written to ${exportStorageStatePath}`,
  );

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

  let captured = false;

  try {
    const context = await browser.newContext({
      viewport: null,
      locale: "en-US",
      ...(initialStorageState ? { storageState: initialStorageState } : {}),
    });

    const capturePromise = new Promise((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        reject(
          new Error(
            "Timed out waiting for a fresh Udio generate request. Solve the challenge and click generate while the helper window is open.",
          ),
        );
      }, timeoutMs);

      context.route("**/api/generate-proxy", async (route) => {
        if (captured) {
          await route.continue();
          return;
        }
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

        captured = true;
        clearTimeout(timeoutId);

        const output = {
          ok: true,
          capturedAt: new Date().toISOString(),
          targetUrl,
          requestUrl: request.url(),
          captchaToken,
          genParams: payload?.gen_params ?? null,
        };

        try {
          await ensureParentDir(tokenOutputPath);
          await writeFile(
            path.resolve(process.cwd(), tokenOutputPath),
            JSON.stringify(output, null, 2),
            "utf8",
          );
          await exportStorageState(context, exportStorageStatePath);
        } catch (error) {
          reject(error);
          await route.fulfill({
            status: 500,
            contentType: "application/json",
            body: JSON.stringify({
              error: "failed_to_persist_captcha_token",
            }),
          });
          return;
        }

        console.log("[udio-captcha] Captured a fresh captchaToken.");
        resolve(output);

        await route.fulfill({
          status: 409,
          contentType: "application/json",
          body: JSON.stringify({
            error: "intercepted_for_local_gateway_validation",
            message: "The captcha token was captured locally before the request reached Udio.",
          }),
        });
      });
    });

    const page = await context.newPage();
    await page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });
    await page.bringToFront().catch(() => undefined);

    const result = await capturePromise;
    console.log(JSON.stringify(result, null, 2));
    await page.waitForTimeout(2_000);
  } finally {
    await browser.close().catch(() => undefined);
  }
}

main().catch((error) => {
  console.error("[udio-captcha] Failed:", error);
  process.exitCode = 1;
});
