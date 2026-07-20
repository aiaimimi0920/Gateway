/**
 * Focused repo-side helper.
 *
 * Loads Gemini with an exported storageState and records interesting requests
 * made by the page itself so we can inspect the real browser-side upstream path.
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

const executablePath = WINDOWS_BROWSER_PATHS.find((candidate) => existsSync(candidate));
if (!executablePath) {
  console.error(JSON.stringify({ ok: false, message: "No Chromium-compatible browser found." }));
  process.exit(1);
}

const storageStatePath = resolveGeminiCanvasManualTestStorageStatePath();
const storageState = JSON.parse(readFileSync(storageStatePath, "utf8"));

const interesting = [];
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
  page.on("request", (request) => {
    const url = request.url();
    if (
      url.includes("generativelanguage.googleapis.com") ||
      url.includes("geminiweb-pa.clients6.google.com") ||
      url.includes("content.googleapis.com")
    ) {
      interesting.push({
        method: request.method(),
        url,
        headers: Object.fromEntries(
          Object.entries(request.headers()).filter(([key]) =>
            [
              "authorization",
              "x-origin",
              "x-goog-authuser",
              "x-goog-api-key",
              "content-type",
              "origin",
            ].includes(key.toLowerCase()),
          ),
        ),
      });
    }
  });

  await page.goto("https://gemini.google.com/app", {
    waitUntil: "domcontentloaded",
    timeout: 60_000,
  });
  await page.waitForLoadState("networkidle", { timeout: 20_000 }).catch(() => undefined);
  await page.waitForTimeout(15_000);

  console.log(
    JSON.stringify({
      ok: true,
      currentUrl: page.url(),
      requests: interesting,
    }),
  );
  await context.close();
} catch (error) {
  console.error(
    JSON.stringify({
      ok: false,
      message: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : null,
      requests: interesting,
    }),
  );
  process.exit(1);
} finally {
  await browser.close().catch(() => undefined);
}
