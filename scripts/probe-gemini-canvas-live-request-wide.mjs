/**
 * Focused repo-side helper.
 *
 * Reopens an already captured persistent Gemini profile and records broad POST
 * traffic so we can identify the real web-generation backend route.
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import path from "node:path";
import { resolveGeminiCanvasLiveProbeUserDataDir } from "./gemini-canvas-runtime-paths.mjs";

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

const storageRoot = path.resolve(
  process.cwd(),
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects",
);
const userDataDir = resolveGeminiCanvasLiveProbeUserDataDir(storageRoot);
if (!existsSync(userDataDir)) {
  console.error(JSON.stringify({ ok: false, message: `Profile dir not found: ${userDataDir}` }));
  process.exit(1);
}

const interesting = [];
let resolveDone;
const done = new Promise((resolve) => {
  resolveDone = resolve;
});
let settled = false;
const finish = (payload) => {
  if (settled) return;
  settled = true;
  resolveDone(payload);
};

console.log("[live-probe-wide] Reopening persistent Gemini profile.");
console.log(`[live-probe-wide] Profile dir: ${userDataDir}`);
console.log("[live-probe-wide] No re-login should be needed. Please generate one image again.");

const context = await chromium.launchPersistentContext(userDataDir, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: [
    "--disable-blink-features=AutomationControlled",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
  ],
});

const page = context.pages()[0] ?? (await context.newPage());

const shouldTrack = (request) => {
  const url = request.url();
  const method = request.method().toUpperCase();
  const lowered = url.toLowerCase();
  if (!["POST", "PUT", "PATCH"].includes(method)) return false;
  return (
    lowered.includes("gemini.google.com") ||
    lowered.includes("googleapis.com") ||
    lowered.includes("clients6.google.com") ||
    lowered.includes("google.com")
  );
};

const sanitizeHeaders = (headers) =>
  Object.fromEntries(
    Object.entries(headers).filter(([key]) =>
      [
        "authorization",
        "content-type",
        "origin",
        "referer",
        "x-origin",
        "x-goog-authuser",
        "x-goog-api-key",
        "x-goog-api-client",
      ].includes(key.toLowerCase()),
    ),
  );

page.on("request", (request) => {
  if (!shouldTrack(request)) return;
  const item = {
    stage: "request",
    method: request.method(),
    resourceType: request.resourceType(),
    url: request.url(),
    headers: sanitizeHeaders(request.headers()),
    postData: request.postData() ?? null,
  };
  interesting.push(item);
  if (
    item.url.toLowerCase().includes("generate") ||
    item.url.toLowerCase().includes("image") ||
    item.url.toLowerCase().includes("upload") ||
    item.url.toLowerCase().includes("stream")
  ) {
    finish({ trigger: item });
  }
});

page.on("response", async (response) => {
  const request = response.request();
  if (!shouldTrack(request)) return;
  const item = {
    stage: "response",
    method: request.method(),
    resourceType: request.resourceType(),
    url: response.url(),
    status: response.status(),
    headers: sanitizeHeaders(request.headers()),
    responseHeaders: Object.fromEntries(
      Object.entries(response.headers()).filter(([key]) =>
        ["content-type", "location"].includes(key.toLowerCase()),
      ),
    ),
    bodyText: null,
  };
  if (
    item.url.toLowerCase().includes("generate") ||
    item.url.toLowerCase().includes("image") ||
    item.url.toLowerCase().includes("upload") ||
    item.url.toLowerCase().includes("stream")
  ) {
    try {
      item.bodyText = await response.text();
    } catch {
      item.bodyText = null;
    }
    interesting.push(item);
    finish({ trigger: item });
  } else {
    interesting.push(item);
  }
});

await page.goto("https://gemini.google.com/app/35b8e4b04edec786", {
  waitUntil: "domcontentloaded",
  timeout: 60_000,
});

const timeoutId = setTimeout(() => {
  finish({ trigger: { stage: "timeout" } });
}, 20 * 60 * 1000);

const result = await done;
clearTimeout(timeoutId);

console.log(
  JSON.stringify(
    {
      ok: true,
      runtimeStateObjectKey: relativeKey,
      absolutePath: userDataDir,
      currentUrl: page.url(),
      result,
      interesting,
      note: "Focused wide Gemini web probe only.",
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
