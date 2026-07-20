/**
 * Focused repo-side helper.
 *
 * Executes one real Gemini image generation using the latest persistent
 * profile, while broadly recording interesting HTTP requests/responses and
 * WebSocket frames so we can identify the true prompt transport.
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
  console.error(JSON.stringify({ ok: false, message: `No profile found under ${profileRoot}` }));
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
const outDir = path.join(storageRoot, "debug", "gemini-canvas", `auto-run-broad-${stamp}`);
mkdirSync(outDir, { recursive: true });

const prompt =
  "CANVAS_API_PROBE_BROAD_20260411 一只戴透明头盔的柯基在霓虹城市街头吃拉面，电影感，高清细节，16:9";
const marker = "CANVAS_API_PROBE_BROAD_20260411";

const events = [];
const MAX_EVENTS = 1200;
let markerTransport = null;

const interestingUrl = (url) => {
  const lowered = url.toLowerCase();
  return (
    lowered.includes("gemini.google.com") ||
    lowered.includes("googleapis.com") ||
    lowered.includes("clients6.google.com") ||
    lowered.includes("googleusercontent.com") ||
    lowered.includes("gstatic.com")
  );
};

const pushEvent = (event) => {
  events.push(event);
  if (events.length > MAX_EVENTS) events.shift();
  const haystack = `${event.url ?? ""}\n${event.postData ?? ""}\n${event.bodyText ?? ""}\n${event.payload ?? ""}`;
  if (!markerTransport && haystack.includes(marker)) {
    markerTransport = event;
  }
};

const context = await chromium.launchPersistentContext(latestProfileDir.fullPath, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

const page = context.pages()[0] ?? (await context.newPage());

page.on("request", (request) => {
  if (!interestingUrl(request.url())) return;
  const method = request.method().toUpperCase();
  if (!["POST", "PUT", "PATCH", "GET"].includes(method)) return;
  const resourceType = request.resourceType();
  if (method === "GET" && !["image", "fetch", "xhr", "document"].includes(resourceType)) return;
  pushEvent({
    kind: "request",
    ts: new Date().toISOString(),
    method,
    resourceType,
    url: request.url(),
    headers: request.headers(),
    postData: request.postData() ?? null,
  });
});

page.on("response", async (response) => {
  const request = response.request();
  if (!interestingUrl(response.url())) return;
  const method = request.method().toUpperCase();
  const resourceType = request.resourceType();
  if (!["POST", "PUT", "PATCH", "GET"].includes(method)) return;
  if (method === "GET" && !["image", "fetch", "xhr", "document"].includes(resourceType)) return;
  const contentType = response.headers()["content-type"] ?? "";
  const event = {
    kind: "response",
    ts: new Date().toISOString(),
    method,
    resourceType,
    url: response.url(),
    status: response.status(),
    contentType,
    bodyText: null,
  };
  if (/json|text|javascript|html/i.test(contentType)) {
    try {
      event.bodyText = await response.text();
    } catch {
      event.bodyText = null;
    }
  }
  pushEvent(event);
});

page.on("websocket", (websocket) => {
  const base = {
    kind: "websocket",
    ts: new Date().toISOString(),
    url: websocket.url(),
  };
  pushEvent({ ...base, phase: "open" });
  websocket.on("framesent", (frame) => {
    pushEvent({
      ...base,
      phase: "framesent",
      payload: typeof frame.payload === "string" ? frame.payload.slice(0, 4000) : String(frame.payload),
    });
  });
  websocket.on("framereceived", (frame) => {
    pushEvent({
      ...base,
      phase: "framereceived",
      payload:
        typeof frame.payload === "string" ? frame.payload.slice(0, 4000) : String(frame.payload),
    });
  });
  websocket.on("close", () => {
    pushEvent({ ...base, phase: "close" });
  });
});

await page.goto("https://gemini.google.com/app", {
  waitUntil: "domcontentloaded",
  timeout: 60_000,
});
await page.waitForTimeout(6_000);

await page.getByRole("button", { name: /制作图片/ }).first().click({ timeout: 30_000 });
await page.waitForTimeout(800);
const textbox = page.locator('div[role="textbox"][aria-label="为 Gemini 输入提示"]').first();
await textbox.click({ timeout: 30_000 });
await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
await page.keyboard.type(prompt, { delay: 16 });
await page.waitForTimeout(400);
await page.getByRole("button", { name: "发送" }).first().click({ timeout: 30_000 });

const waitUntil = Date.now() + 5 * 60 * 1000;
while (Date.now() < waitUntil && !markerTransport) {
  await page.waitForTimeout(1_000);
}

const imageWaitUntil = Date.now() + 6 * 60 * 1000;
let imageNodes = [];
while (Date.now() < imageWaitUntil) {
  imageNodes = await page.evaluate(() =>
    Array.from(document.querySelectorAll("img"))
      .map((img, i) => ({
        i,
        alt: img.getAttribute("alt"),
        src: img.getAttribute("src"),
        width: img.naturalWidth,
        height: img.naturalHeight,
      }))
      .filter((item) => item.src && !item.src.startsWith("data:image/gif")),
  );
  if (
    imageNodes.some(
      (node) =>
        node.width >= 256 ||
        node.height >= 256 ||
        /AI 生成/i.test(node.alt ?? "") ||
        /gg-dl|googleusercontent\/gg-dl|=s1024|=w\d+/i.test(node.src),
    )
  ) {
    break;
  }
  await page.waitForTimeout(2_000);
}

const artifact = {
  ok: true,
  capturedAt: new Date().toISOString(),
  runtimeStateObjectKey: path.relative(storageRoot, latestProfileDir.fullPath).replaceAll(path.sep, "/"),
  profileDir: latestProfileDir.fullPath,
  prompt,
  markerTransport,
  imageNodes,
  pageUrl: page.url(),
  events,
  note: "Focused broad Gemini Canvas transport probe only.",
};

await page.screenshot({
  path: path.join(outDir, "page.png"),
  fullPage: true,
});
writeFileSync(path.join(outDir, "auto-run-broad.json"), `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

console.log(
  JSON.stringify(
    {
      ok: true,
      outDir,
      runtimeStateObjectKey: artifact.runtimeStateObjectKey,
      markerTransportKind: markerTransport?.kind ?? null,
      markerTransportUrl: markerTransport?.url ?? null,
      imageNodeCount: imageNodes.length,
      pageUrl: artifact.pageUrl,
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
