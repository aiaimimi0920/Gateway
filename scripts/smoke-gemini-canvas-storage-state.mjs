/**
 * Focused repo-side helper.
 *
 * Validates whether a previously exported Gemini Canvas storageState can be
 * loaded into a fresh Playwright browser context and navigate to Gemini.
 */

import { chromium } from "playwright-core";
import { existsSync, readFileSync } from "node:fs";
import { resolveGeminiCanvasManualTestStorageStatePath } from "./gemini-canvas-runtime-paths.mjs";

const WINDOWS_BROWSER_PATHS = [
  process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH,
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
].filter(Boolean);

const storageStatePath =
  process.argv[2] ??
  resolveGeminiCanvasManualTestStorageStatePath();

const executablePath = WINDOWS_BROWSER_PATHS.find((candidate) => existsSync(candidate));
if (!executablePath) {
  console.error(JSON.stringify({ ok: false, message: "No Chromium-compatible browser found." }));
  process.exit(1);
}

const storageState = JSON.parse(readFileSync(storageStatePath, "utf8"));

const browser = await chromium.launch({
  executablePath,
  headless: false,
  args: [
    "--disable-blink-features=AutomationControlled",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
  ],
});

try {
  const context = await browser.newContext({
    storageState,
    locale: "zh-CN",
  });
  const page = await context.newPage();
  await page.goto("https://gemini.google.com/app", {
    waitUntil: "domcontentloaded",
    timeout: 60_000,
  });
  console.log(
    JSON.stringify({
      ok: true,
      executablePath,
      currentUrl: page.url(),
      title: await page.title(),
      cookieCount: (await context.cookies()).length,
    }),
  );
  await context.close();
} catch (error) {
  console.error(
    JSON.stringify({
      ok: false,
      executablePath,
      message: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : null,
    }),
  );
  process.exit(1);
} finally {
  await browser.close().catch(() => undefined);
}
