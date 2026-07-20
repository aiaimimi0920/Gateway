/**
 * Focused repo-side helper.
 *
 * Reopens the latest persistent Gemini profile and records same-origin
 * BardChatUi batchexecute traffic to disk so we can reverse the real image
 * generation request/response shape.
 */

import { chromium } from "playwright-core";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
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

const storageRoot = path.resolve(
  process.cwd(),
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects",
);
const profileRoot = path.join(storageRoot, "credential-runtime", "gemini-canvas-profile");
if (!existsSync(profileRoot)) {
  console.error(
    JSON.stringify({ ok: false, message: `Profile root not found: ${profileRoot}` }),
  );
  process.exit(1);
}

const latestProfileDir = readdirSync(profileRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => ({
    name: entry.name,
    fullPath: path.join(profileRoot, entry.name, "user-data"),
    sortKey: entry.name,
  }))
  .filter((entry) => existsSync(entry.fullPath))
  .sort((a, b) => b.sortKey.localeCompare(a.sortKey))[0];

if (!latestProfileDir) {
  console.error(
    JSON.stringify({ ok: false, message: `No persistent Gemini profile found under ${profileRoot}` }),
  );
  process.exit(1);
}

const relativeProfileKey = path
  .relative(storageRoot, latestProfileDir.fullPath)
  .replaceAll(path.sep, "/");

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
const captureDir = path.join(storageRoot, "debug", "gemini-canvas", `batchexecute-${stamp}`);
mkdirSync(captureDir, { recursive: true });
const PROMPT_MARKER = "CANVAS_API_PROBE";

const interesting = [];
const MAX_RECORDS = 200;
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

const trackedRequests = new Map();

const pickHeaders = (headers) =>
  Object.fromEntries(
    Object.entries(headers).filter(([key]) =>
      [
        "content-type",
        "origin",
        "referer",
        "authorization",
        "x-goog-authuser",
        "x-goog-api-key",
        "x-goog-api-client",
      ].includes(key.toLowerCase()),
    ),
  );

const isTargetRequest = (request) => {
  const url = request.url();
  return (
    request.method().toUpperCase() === "POST" &&
    url.includes("gemini.google.com/_/BardChatUi/data/batchexecute")
  );
};

console.log("[batchexecute-probe] Reopening latest persistent Gemini profile.");
console.log(`[batchexecute-probe] Profile key: ${relativeProfileKey}`);
console.log(`[batchexecute-probe] Profile dir: ${latestProfileDir.fullPath}`);
console.log(`[batchexecute-probe] Capture dir: ${captureDir}`);
console.log(
  "[batchexecute-probe] Please generate ONE image now, and include a unique marker in the prompt, for example: CANVAS_API_PROBE_CAT_20260411",
);

const context = await chromium.launchPersistentContext(latestProfileDir.fullPath, {
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

page.on("request", (request) => {
  if (!isTargetRequest(request)) return;
  const postData = request.postData() ?? null;
  const record = {
    kind: "request",
    ts: new Date().toISOString(),
    method: request.method(),
    resourceType: request.resourceType(),
    url: request.url(),
    headers: pickHeaders(request.headers()),
    postData,
  };
  trackedRequests.set(request, record);
  interesting.push(record);
  if (interesting.length > MAX_RECORDS) interesting.shift();
  if (postData?.includes(PROMPT_MARKER)) {
    writeFileSync(
      path.join(captureDir, "batchexecute-marker-request.json"),
      `${JSON.stringify(record, null, 2)}\n`,
      "utf8",
    );
  }
});

page.on("response", async (response) => {
  const request = response.request();
  if (!isTargetRequest(request)) return;
  const requestRecord = trackedRequests.get(request) ?? null;
  const responseRecord = {
    kind: "response",
    ts: new Date().toISOString(),
    method: request.method(),
    resourceType: request.resourceType(),
    url: response.url(),
    status: response.status(),
    requestHeaders: pickHeaders(request.headers()),
    responseHeaders: Object.fromEntries(
      Object.entries(response.headers()).filter(([key]) =>
        ["content-type", "content-length", "location"].includes(key.toLowerCase()),
      ),
    ),
    bodyText: null,
  };
  try {
    responseRecord.bodyText = await response.text();
  } catch {
    responseRecord.bodyText = null;
  }
  interesting.push(responseRecord);
  if (interesting.length > MAX_RECORDS) interesting.shift();

  const markerMatched =
    requestRecord?.postData?.includes(PROMPT_MARKER) || responseRecord.bodyText?.includes(PROMPT_MARKER);
  if (markerMatched) {
    const artifact = {
      ok: true,
      capturedAt: new Date().toISOString(),
      runtimeStateObjectKey: relativeProfileKey,
      profileDir: latestProfileDir.fullPath,
      currentUrl: page.url(),
      request: requestRecord,
      response: responseRecord,
      allObserved: interesting,
      note: "Focused BardChatUi batchexecute probe only.",
    };
    const artifactPath = path.join(captureDir, "batchexecute-capture.json");
    writeFileSync(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`, "utf8");
    finish({
      artifactPath,
      status: response.status(),
      url: response.url(),
      requestPreview: requestRecord?.postData?.slice(0, 500) ?? null,
      responsePreview: responseRecord.bodyText?.slice(0, 500) ?? null,
    });
  }
});

await page.goto("https://gemini.google.com/app", {
  waitUntil: "domcontentloaded",
  timeout: 60_000,
});

const timeoutId = setTimeout(() => {
  const artifact = {
    ok: false,
    capturedAt: new Date().toISOString(),
    runtimeStateObjectKey: relativeProfileKey,
    profileDir: latestProfileDir.fullPath,
    currentUrl: page.url(),
    observedCount: interesting.length,
    allObserved: interesting,
    note: `Timed out waiting for a BardChatUi batchexecute response containing ${PROMPT_MARKER}.`,
  };
  const artifactPath = path.join(captureDir, "batchexecute-timeout.json");
  writeFileSync(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`, "utf8");
  finish({
    artifactPath,
    timedOut: true,
    observedCount: interesting.length,
  });
}, 20 * 60 * 1000);

const result = await done;
clearTimeout(timeoutId);

console.log(JSON.stringify(result, null, 2));

await context.close().catch(() => undefined);
