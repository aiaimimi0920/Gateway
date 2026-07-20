/**
 * Focused repo-side helper.
 *
 * Uses the latest persistent Gemini Canvas browser profile to trigger one real
 * media generation run from the Gemini web UI while broadly recording the
 * matching HTTP requests/responses and rendered media nodes.
 *
 * Usage:
 *   $env:GEMINI_CANVAS_MEDIA_KIND='music'; node gateway/scripts/run-gemini-canvas-media-broad.mjs
 *   $env:GEMINI_CANVAS_MEDIA_KIND='video'; node gateway/scripts/run-gemini-canvas-media-broad.mjs
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

const kind = (process.env.GEMINI_CANVAS_MEDIA_KIND ?? "").trim().toLowerCase();
const configs = {
  music: {
    buttonName: /创作音乐/,
    prompt:
      "CANVAS_API_PROBE_MUSIC_20260411 创作一段 10 秒的轻快 lo-fi 钢琴与雨声氛围音乐，温暖、干净、适合夜晚学习。",
    marker: "CANVAS_API_PROBE_MUSIC_20260411",
    outPrefix: "music-run-broad",
    maxWaitMs: 8 * 60 * 1000,
    settleAfterMarkerResponseMs: 45_000,
  },
  video: {
    buttonName: /创作视频/,
    prompt:
      "CANVAS_API_PROBE_VIDEO_20260411 一只穿宇航服的柯基在月球表面慢动作奔跑，电影感，16:9，镜头缓慢推进。",
    marker: "CANVAS_API_PROBE_VIDEO_20260411",
    outPrefix: "video-run-broad",
    maxWaitMs: 12 * 60 * 1000,
    settleAfterMarkerResponseMs: 90_000,
  },
};

const config = configs[kind];
if (!config) {
  console.error(
    JSON.stringify({
      ok: false,
      message: "Set GEMINI_CANVAS_MEDIA_KIND to 'music' or 'video'.",
    }),
  );
  process.exit(1);
}

const promptOverride = (process.env.GEMINI_CANVAS_MEDIA_PROMPT ?? "").trim();
if (promptOverride) {
  config.prompt = promptOverride;
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
const outDir = path.join(storageRoot, "debug", "gemini-canvas", `${config.outPrefix}-${stamp}`);
mkdirSync(outDir, { recursive: true });

const events = [];
const MAX_EVENTS = 1800;
let markerTransport = null;
let markerResponseSeenAt = null;

const interestingUrl = (url) => {
  const lowered = url.toLowerCase();
  return (
    lowered.includes("gemini.google.com") ||
    lowered.includes("googleapis.com") ||
    lowered.includes("clients6.google.com") ||
    lowered.includes("googleusercontent.com") ||
    lowered.includes("googlevideo.com") ||
    lowered.includes("gvt1.com") ||
    lowered.includes("gstatic.com")
  );
};

const shouldCaptureResponseBody = (contentType, url) => {
  if (/json|text|javascript|html/i.test(contentType)) {
    return true;
  }
  return /gg-dl|rd-gg-dl|googleusercontent/i.test(url) && /text\/plain/i.test(contentType);
};

const pushEvent = (event) => {
  events.push(event);
  if (events.length > MAX_EVENTS) {
    events.shift();
  }

  const haystack = `${event.url ?? ""}\n${event.postData ?? ""}\n${event.bodyText ?? ""}\n${event.payload ?? ""}`;
  if (!markerTransport && haystack.includes(config.marker)) {
    markerTransport = event;
  }
  if (
    markerTransport &&
    !markerResponseSeenAt &&
    event.kind === "response" &&
    event.url === markerTransport.url
  ) {
    markerResponseSeenAt = Date.now();
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
  const resourceType = request.resourceType();
  if (!["POST", "PUT", "PATCH", "GET"].includes(method)) return;
  if (method === "GET" && !["image", "media", "fetch", "xhr", "document"].includes(resourceType)) {
    return;
  }
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
  if (method === "GET" && !["image", "media", "fetch", "xhr", "document"].includes(resourceType)) {
    return;
  }

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
  if (shouldCaptureResponseBody(contentType, response.url())) {
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
      payload: typeof frame.payload === "string" ? frame.payload.slice(0, 6000) : String(frame.payload),
    });
  });
  websocket.on("framereceived", (frame) => {
    pushEvent({
      ...base,
      phase: "framereceived",
      payload: typeof frame.payload === "string" ? frame.payload.slice(0, 6000) : String(frame.payload),
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

await page.getByRole("button", { name: config.buttonName }).first().click({ timeout: 30_000 });
await page.waitForTimeout(1200);

const textbox = page
  .locator('[role="textbox"][aria-label="为 Gemini 输入提示"], [role="textbox"], [contenteditable="true"]')
  .first();
await textbox.focus();
await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
await page.keyboard.type(config.prompt, { delay: 16 });
await page.waitForTimeout(500);
await page.getByRole("button", { name: "发送" }).first().click({ timeout: 30_000, force: true });

const submitWaitUntil = Date.now() + 2 * 60 * 1000;
while (Date.now() < submitWaitUntil && !markerTransport) {
  await page.waitForTimeout(1000);
}

const mediaWaitUntil = Date.now() + config.maxWaitMs;
let mediaNodes = [];
let anchorNodes = [];
let pageState = null;
while (Date.now() < mediaWaitUntil) {
  ({ mediaNodes, anchorNodes, pageState } = await page.evaluate(() => {
    const mediaNodes = [
      ...Array.from(document.querySelectorAll("audio")).map((node, i) => ({
        kind: "audio",
        i,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
      })),
      ...Array.from(document.querySelectorAll("video")).map((node, i) => ({
        kind: "video",
        i,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        width: node.videoWidth,
        height: node.videoHeight,
        duration: Number.isFinite(node.duration) ? node.duration : null,
        poster: node.getAttribute("poster"),
      })),
      ...Array.from(document.querySelectorAll("img")).map((node, i) => ({
        kind: "img",
        i,
        alt: node.getAttribute("alt"),
        src: node.getAttribute("src"),
        width: node.naturalWidth,
        height: node.naturalHeight,
      })),
    ].filter((node) => node.src || node.currentSrc);

    const anchorNodes = Array.from(document.querySelectorAll("a"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        href: node.getAttribute("href"),
      }))
      .filter((node) => node.href || node.text);

    return {
      mediaNodes,
      anchorNodes,
      pageState: {
        url: location.href,
        title: document.title,
        bodyText: document.body.innerText.slice(0, 12000),
      },
    };
  }));

  const hasInterestingMedia = mediaNodes.some((node) => {
    const src = `${node.currentSrc ?? ""} ${node.src ?? ""}`;
    const isLikelyGeneratedImage =
      node.kind === "img" &&
      (
        /AI 生成/i.test(node.alt ?? "") ||
        (
          /blob:|googleusercontent|googlevideo|gvt1|gg-dl|rd-gg-dl/i.test(src) &&
          !/lh3\.google(?:usercontent)?\.com\/u\/0\/ogw|lh3\.googleusercontent\.com\/a\//i.test(src) &&
          ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256)
        )
      );
    return (
      node.kind === "audio" ||
      node.kind === "video" ||
      isLikelyGeneratedImage
    );
  });
  const hasInterestingAnchor = anchorNodes.some((node) =>
    /googleusercontent|googlevideo|gg-dl|rd-gg-dl|\.mp3(\?|$)|\.wav(\?|$)|\.m4a(\?|$)|\.mp4(\?|$)|\.webm(\?|$)/i.test(
      node.href ?? "",
    ),
  );
  const hasMarkerResponse = Boolean(
    markerTransport &&
      events.some((event) => event.kind === "response" && event.url === markerTransport.url),
  );

  if (hasInterestingMedia || hasInterestingAnchor) {
    break;
  }

  if (
    hasMarkerResponse &&
    markerResponseSeenAt &&
    Date.now() - markerResponseSeenAt >= config.settleAfterMarkerResponseMs
  ) {
    break;
  }

  await page.waitForTimeout(2000);
}

await page.screenshot({
  path: path.join(outDir, "page.png"),
  fullPage: true,
});

const artifact = {
  ok: true,
  capturedAt: new Date().toISOString(),
  kind,
  runtimeStateObjectKey: path.relative(storageRoot, latestProfileDir.fullPath).replaceAll(path.sep, "/"),
  profileDir: latestProfileDir.fullPath,
  prompt: config.prompt,
  markerTransport,
  mediaNodes,
  anchorNodes,
  pageState,
  events,
  note: "Focused Gemini Canvas media transport probe only.",
};
writeFileSync(path.join(outDir, `${kind}-run-broad.json`), `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

console.log(
  JSON.stringify(
    {
      ok: true,
      outDir,
      kind,
      runtimeStateObjectKey: artifact.runtimeStateObjectKey,
      markerTransportKind: markerTransport?.kind ?? null,
      markerTransportUrl: markerTransport?.url ?? null,
      mediaNodeCount: mediaNodes.length,
      anchorNodeCount: anchorNodes.length,
      pageUrl: pageState?.url ?? null,
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
