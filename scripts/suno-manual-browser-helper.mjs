/**
 * Visible Suno login helper.
 *
 * Keeps a dedicated browser session alive for manual sign-in and writes only
 * local runtime credentials. Secret material is never printed to stdout.
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const TARGET_URL = process.env.SUNO_MANUAL_HELPER_TARGET_URL ?? "https://suno.com/create";
const STORAGE_STATE_PATH =
  process.env.SUNO_MANUAL_HELPER_STORAGE_STATE_PATH ??
  ".runtime/suno-manual-browser-helper.storage-state.json";
const COOKIE_PATH =
  process.env.SUNO_MANUAL_HELPER_COOKIE_PATH ?? ".runtime/suno-manual-browser-helper.cookie";
const STATUS_PATH =
  process.env.SUNO_MANUAL_HELPER_STATUS_PATH ?? ".runtime/suno-manual-browser-helper.status.json";
const RUNTIME_STATE_OBJECT_KEY =
  process.env.SUNO_MANUAL_HELPER_OBJECT_KEY ??
  "credential-runtime/suno/manual-browser-helper/storage-state.json";
const OBJECT_STORAGE_LOCAL_DIR =
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects";
const IDLE_TIMEOUT_MS = Number(process.env.SUNO_MANUAL_HELPER_IDLE_TIMEOUT_MS ?? 12 * 60 * 60 * 1000);
const POLL_INTERVAL_MS = Number(process.env.SUNO_MANUAL_HELPER_POLL_INTERVAL_MS ?? 10_000);
const AUTH_COOKIE_NAME = "__session";

const browserPaths = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/microsoft-edge",
  "/usr/bin/chromium",
];

function resolveExecutablePath() {
  const configured = process.env.SUNO_MANUAL_HELPER_BROWSER_EXECUTABLE_PATH;
  return configured && existsSync(configured)
    ? configured
    : browserPaths.find((candidate) => existsSync(candidate)) ?? null;
}

function resolvePath(value) {
  return path.resolve(process.cwd(), value);
}

async function writeJson(filePath, value) {
  const absolutePath = resolvePath(filePath);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, JSON.stringify(value, null, 2), "utf8");
}

function cookieHeader(cookies) {
  return cookies
    .filter((cookie) => cookie.domain.includes("suno.com") || cookie.domain.includes("suno.ai"))
    .map((cookie) => `${cookie.name}=${cookie.value}`)
    .join("; ");
}

function isAuthenticated(cookies) {
  return cookies.some((cookie) => cookie.name === AUTH_COOKIE_NAME && Boolean(cookie.value));
}

async function main() {
  const executablePath = resolveExecutablePath();
  if (!executablePath) {
    throw new Error("Unable to locate Chrome/Edge. Set SUNO_MANUAL_HELPER_BROWSER_EXECUTABLE_PATH.");
  }

  const browser = await chromium.launch({
    executablePath,
    headless: false,
    args: ["--disable-blink-features=AutomationControlled", "--no-first-run", "--no-default-browser-check"],
  });
  const context = await browser.newContext({ locale: "en-US" });
  const page = await context.newPage();
  await page.goto(TARGET_URL, { waitUntil: "domcontentloaded", timeout: 45_000 });

  let lastAuthenticatedAt = 0;
  const writeSnapshot = async () => {
    const cookies = await context.cookies(["https://suno.com", "https://suno.ai"]);
    const authenticated = isAuthenticated(cookies);
    if (authenticated) {
      const state = await context.storageState();
      await writeJson(STORAGE_STATE_PATH, state);
      await writeJson(path.join(OBJECT_STORAGE_LOCAL_DIR, RUNTIME_STATE_OBJECT_KEY), state);
      await mkdir(path.dirname(resolvePath(COOKIE_PATH)), { recursive: true });
      await writeFile(resolvePath(COOKIE_PATH), cookieHeader(cookies), "utf8");
      lastAuthenticatedAt = Date.now();
    }
    await writeJson(STATUS_PATH, {
      ok: true,
      authenticated,
      updatedAt: new Date().toISOString(),
      currentUrl: page.url(),
      storageStatePath: resolvePath(STORAGE_STATE_PATH),
      runtimeStateObjectKey: RUNTIME_STATE_OBJECT_KEY,
      cookiePath: resolvePath(COOKIE_PATH),
    });
    return authenticated;
  };

  try {
    while (!page.isClosed()) {
      await writeSnapshot();
      if (lastAuthenticatedAt && Date.now() - lastAuthenticatedAt > IDLE_TIMEOUT_MS) break;
      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
    }
  } finally {
    await writeSnapshot().catch(() => undefined);
    await browser.close().catch(() => undefined);
  }
}

main().catch(async (error) => {
  await writeJson(STATUS_PATH, {
    ok: false,
    authenticated: false,
    updatedAt: new Date().toISOString(),
    error: error instanceof Error ? error.message : String(error),
  }).catch(() => undefined);
  console.error("[suno-manual-helper] failed:", error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
