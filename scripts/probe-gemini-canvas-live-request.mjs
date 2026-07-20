/**
 * Focused repo-side helper.
 *
 * Opens a persistent Gemini browser profile so the user can manually log in and
 * trigger one real image generation. The script captures the actual upstream
 * requests/responses made by the page and prints a compact JSON summary.
 */

import { chromium } from "playwright-core";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";

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

const now = new Date();
const stamp = [
  now.getFullYear(),
  String(now.getMonth() + 1).padStart(2, "0"),
  String(now.getDate()).padStart(2, "0"),
  "-",
  String(now.getHours()).padStart(2, "0"),
  String(now.getMinutes()).padStart(2, "0"),
  String(now.getSeconds()).padStart(2, "0"),
].join("");

const relativeKey = `credential-runtime/gemini-canvas-profile/live-probe-${stamp}/user-data`;
const storageRoot = path.resolve(
  process.cwd(),
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects",
);
const userDataDir = path.join(storageRoot, ...relativeKey.split("/"));
mkdirSync(userDataDir, { recursive: true });

const interestingRequests = [];
const interestingResponses = [];

const matchesInterestingUrl = (url) =>
  url.includes("generativelanguage.googleapis.com") ||
  url.includes("geminiweb-pa.clients6.google.com") ||
  url.includes("content.googleapis.com") ||
  url.includes("googleapis.com");

const isInterestingMutation = (url, method) => {
  const lowered = url.toLowerCase();
  return (
    method === "POST" &&
    (lowered.includes("generatecontent") ||
      lowered.includes("streamgeneratecontent") ||
      lowered.includes(":predict") ||
      lowered.includes("/predict") ||
      lowered.includes("imagen"))
  );
};

const sanitizeHeaders = (headers) =>
  Object.fromEntries(
    Object.entries(headers).filter(([key]) =>
      [
        "authorization",
        "x-origin",
        "x-goog-authuser",
        "x-goog-api-key",
        "content-type",
        "origin",
        "referer",
      ].includes(key.toLowerCase()),
    ),
  );

console.log("[live-probe] Opening persistent Gemini browser profile.");
console.log(`[live-probe] Profile dir: ${userDataDir}`);
console.log("[live-probe] Please log in if needed, then manually generate one image in Gemini.");
console.log("[live-probe] The script will capture the first real upstream generation request and stop.");

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

page.on("request", (request) => {
  const url = request.url();
  if (!matchesInterestingUrl(url)) return;
  const item = {
    method: request.method(),
    url,
    headers: sanitizeHeaders(request.headers()),
    postData: request.postData() ?? null,
  };
  interestingRequests.push(item);
  if (isInterestingMutation(url, request.method())) {
    finish({
      type: "request",
      item,
    });
  }
});

page.on("response", async (response) => {
  const url = response.url();
  if (!matchesInterestingUrl(url)) return;
  const request = response.request();
  const item = {
    method: request.method(),
    url,
    status: response.status(),
    headers: sanitizeHeaders(request.headers()),
    responseHeaders: Object.fromEntries(
      Object.entries(response.headers()).filter(([key]) =>
        ["content-type", "location"].includes(key.toLowerCase()),
      ),
    ),
    bodyText: null,
  };
  if (isInterestingMutation(url, request.method())) {
    try {
      item.bodyText = await response.text();
    } catch {
      item.bodyText = null;
    }
    interestingResponses.push(item);
    finish({
      type: "response",
      item,
    });
  } else {
    interestingResponses.push(item);
  }
});

await page.goto("https://gemini.google.com/app", {
  waitUntil: "domcontentloaded",
  timeout: 60_000,
});

const timeoutId = setTimeout(() => {
  finish({
    type: "timeout",
  });
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
      trigger: result,
      interestingRequests,
      interestingResponses,
      note: "Focused persistent-profile network probe only.",
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
