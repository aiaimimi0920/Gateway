/**
 * Focused repo-side helper.
 *
 * Executes one real Gemini Canvas image edit using the latest persistent
 * browser profile, uploads a real local reference image, and
 * broadly records the matching HTTP requests/responses and WebSocket frames
 * so we can reverse the true edit transport contract.
 */

import { chromium } from "playwright-core";
import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { resolveGeminiCanvasManualLiveVendorProfileDir } from "./gemini-canvas-runtime-paths.mjs";

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
const manualOverrideProfile = process.env.GEMINI_CANVAS_PROFILE_OVERRIDE;
const currentVendorProfile = resolveGeminiCanvasManualLiveVendorProfileDir(storageRoot);
const profileRoot = path.join(storageRoot, "credential-runtime", "gemini-canvas-profile");

function resolveProfileDir() {
  if (manualOverrideProfile && existsSync(manualOverrideProfile)) {
    return {
      name: path.basename(manualOverrideProfile),
      fullPath: manualOverrideProfile,
      sortKey: "override",
      source: "env-override",
    };
  }

  if (existsSync(currentVendorProfile)) {
    return {
      name: path.basename(currentVendorProfile),
      fullPath: currentVendorProfile,
      sortKey: "manual-live-vendor-profile",
      source: "manual-live-vendor",
    };
  }

  const latestProfileDir = readdirSync(profileRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => ({
      name: entry.name,
      fullPath: path.join(profileRoot, entry.name, "user-data"),
      sortKey: entry.name,
      source: "latest-gemini-canvas-profile",
    }))
    .filter((entry) => existsSync(entry.fullPath))
    .sort((a, b) => b.sortKey.localeCompare(a.sortKey))[0];

  return latestProfileDir ?? null;
}

const latestProfileDir = resolveProfileDir();

if (!latestProfileDir) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: `No Gemini Canvas browser profile found under ${profileRoot}`,
      },
      null,
      2,
    ),
  );
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
const outDir = path.join(storageRoot, "debug", "gemini-canvas", `image-edit-broad-${stamp}`);
mkdirSync(outDir, { recursive: true });

const marker = process.env.GEMINI_CANVAS_IMAGE_EDIT_MARKER ?? `CANVAS_API_PROBE_IMAGE_EDIT_${stamp}`;
const prompt =
  process.env.GEMINI_CANVAS_IMAGE_EDIT_PROMPT ??
  `${marker} 把上传的样例图编辑成一个霓虹徽章，保留主体，增强边缘发光和科技感。`;
const sourceImageOverride = process.env.GEMINI_CANVAS_IMAGE_EDIT_SOURCE_PATH;
const samplePngBase64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z6f0AAAAASUVORK5CYII=";
const sourceImageExtension = sourceImageOverride
  ? path.extname(sourceImageOverride).toLowerCase() || ".png"
  : ".png";
const sampleImagePath = path.join(outDir, `edit-source${sourceImageExtension}`);
if (sourceImageOverride) {
  if (!existsSync(sourceImageOverride)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          message: `Override image not found: ${sourceImageOverride}`,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }
  copyFileSync(sourceImageOverride, sampleImagePath);
} else {
  writeFileSync(sampleImagePath, Buffer.from(samplePngBase64, "base64"));
}

const events = [];
const MAX_EVENTS = 1600;
let markerTransport = null;
let markerResponseSeenAt = null;
let markerRequestSeenAt = null;
let uploadTrigger = null;
let secondarySignalerPollUrl = null;
let secondarySignalerPollSeenAt = null;
let secondarySignalerPollCompletedAt = null;
const cdpRequests = new Map();
let finalizeUploadCapture = null;

const interestingUrl = (url) => {
  const lowered = url.toLowerCase();
  return (
    lowered.includes("gemini.google.com") ||
    lowered.includes("googleapis.com") ||
    lowered.includes("clients6.google.com") ||
    lowered.includes("googleusercontent.com") ||
    lowered.includes("gstatic.com") ||
    lowered.includes("content.googleapis.com")
  );
};

const shouldCaptureResponseBody = (contentType, url) => {
  if (/json|text|javascript|html/i.test(contentType)) {
    return true;
  }
  return /gg-dl|rd-gg-dl|googleusercontent|content\.googleapis/i.test(url);
};

const pushEvent = (event) => {
  events.push(event);
  if (events.length > MAX_EVENTS) {
    events.shift();
  }

  const haystack = `${event.url ?? ""}\n${event.postData ?? ""}\n${event.bodyText ?? ""}\n${event.payload ?? ""}`;
  if (!markerTransport && haystack.includes(marker)) {
    markerTransport = event;
    if (event.kind === "request") {
      markerRequestSeenAt = Date.now();
    }
  }
  if (
    markerTransport &&
    !markerResponseSeenAt &&
    event.kind === "response" &&
    event.url === markerTransport.url
  ) {
    markerResponseSeenAt = Date.now();
  }
  if (
    event.kind === "request" &&
    typeof event.url === "string" &&
    event.url.includes("signaler-pa.clients6.google.com/punctual/multi-watch/channel") &&
    /[?&]AID=(?:[1-9]\d*)\b/i.test(event.url)
  ) {
    secondarySignalerPollUrl = event.url;
    secondarySignalerPollSeenAt = Date.now();
  }
  if (
    secondarySignalerPollUrl &&
    (event.kind === "response" || event.kind === "cdp-response-body") &&
    event.url === secondarySignalerPollUrl
  ) {
    secondarySignalerPollCompletedAt = Date.now();
  }
};

async function uploadReferenceImage(page, imagePath) {
  const triggerFileChooser = async (locator, label, { force = false } = {}) => {
    const chooserPromise = page.waitForEvent("filechooser", { timeout: 15_000 });
    await locator.click({ timeout: 15_000, force });
    const chooser = await chooserPromise;
    await chooser.setFiles(imagePath);
    uploadTrigger = label;
    return true;
  };

  const uploadMenuButton = page.getByRole("button", { name: /打开文件上传菜单|upload/i }).first();
  try {
    await uploadMenuButton.click({ timeout: 8_000, force: true });
    await page.waitForTimeout(400);
  } catch {
    // keep trying other selectors
  }

  const followupTriggers = [
    [
      page.locator('[data-test-id="local-images-files-uploader-button"]').first(),
      "upload-menu-local-images-files-uploader-button",
      { force: false },
    ],
    [
      page.locator(".hidden-local-file-image-selector-button").first(),
      "upload-menu-hidden-local-file-image-selector-button",
      { force: false },
    ],
    [
      page.locator('[data-test-id="local-images-files-uploader-button"]').first(),
      "upload-menu-local-images-files-uploader-button-force",
      { force: true },
    ],
    [
      page.locator('[data-test-id="hidden-local-image-upload-button"]').first(),
      "upload-menu-hidden-local-image-upload-button-force",
      { force: true },
    ],
    [
      page.getByRole("button", { name: /图片|image/i }).first(),
      "upload-menu-image-button-force",
      { force: true },
    ],
    [
      page.getByRole("menuitem", { name: /图片|image/i }).first(),
      "upload-menu-image-menuitem-force",
      { force: true },
    ],
  ];

  for (const [locator, label, options] of followupTriggers) {
    try {
      return await triggerFileChooser(locator, label, options);
    } catch {
      // try next
    }
  }

  throw new Error("Failed to trigger Gemini Canvas local image upload chooser.");
}

const context = await chromium.launchPersistentContext(latestProfileDir.fullPath, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

const page = context.pages()[0] ?? (await context.newPage());
const cdp = await context.newCDPSession(page);
await cdp.send("Network.enable", {
  maxTotalBufferSize: 128 * 1024 * 1024,
  maxResourceBufferSize: 32 * 1024 * 1024,
});

cdp.on("Network.requestWillBeSent", (event) => {
  const requestUrl = event.request?.url ?? "";
  if (!interestingUrl(requestUrl)) {
    return;
  }
  cdpRequests.set(event.requestId, {
    requestId: event.requestId,
    url: requestUrl,
    method: event.request?.method ?? null,
    headers: event.request?.headers ?? null,
    postData: event.request?.postData ?? null,
  });
});

cdp.on("Network.loadingFinished", async (event) => {
  const request = cdpRequests.get(event.requestId);
  if (!request) {
    return;
  }
  if (
    !finalizeUploadCapture &&
    typeof request.url === "string" &&
    request.url.includes("push.clients6.google.com/upload/") &&
    typeof request.headers?.["x-goog-upload-command"] === "string" &&
    /upload,\s*finalize/i.test(request.headers["x-goog-upload-command"])
  ) {
    try {
      const postDataResponse = await cdp.send("Network.getRequestPostData", {
        requestId: event.requestId,
      });
      const postData = postDataResponse?.postData ?? null;
      if (typeof postData === "string" && postData.length > 0) {
        const buffer = Buffer.from(postData, "latin1");
        const sha256 = createHash("sha256").update(buffer).digest("hex");
        const fileName = `upload-finalize-body-${sha256.slice(0, 12)}.bin`;
        const outputPath = path.join(outDir, fileName);
        writeFileSync(outputPath, buffer);
        finalizeUploadCapture = {
          byteLength: buffer.length,
          sha256,
          fileName,
          outputPath,
          uploadUrl: request.url,
          headers: request.headers,
          source: "Network.getRequestPostData",
        };
      }
    } catch {
      // continue with normal body capture
    }
  }
  if (
    !/StreamGenerate|batchexecute|chooseServer|multi-watch\/channel|refreshCreds|content-push|push\.clients6/i.test(
      request.url,
    )
  ) {
    return;
  }
  try {
    const response = await cdp.send("Network.getResponseBody", { requestId: event.requestId });
    const bodyText = response.base64Encoded
      ? Buffer.from(response.body, "base64").toString("utf8")
      : response.body;
    pushEvent({
      kind: "cdp-response-body",
      ts: new Date().toISOString(),
      method: request.method,
      resourceType: "cdp",
      url: request.url,
      status: null,
      headers: request.headers,
      postData: request.postData,
      bodyText: bodyText.slice(0, 120000),
    });
  } catch {
    // Some streaming requests do not expose a retrievable body.
  }
});

page.on("request", async (request) => {
  if (!interestingUrl(request.url())) return;
  const method = request.method().toUpperCase();
  const resourceType = request.resourceType();
  if (!["POST", "PUT", "PATCH", "GET"].includes(method)) return;
  if (method === "GET" && !["image", "media", "fetch", "xhr", "document"].includes(resourceType)) {
    return;
  }
  let headers = request.headers();
  try {
    headers = await request.allHeaders();
  } catch {
    headers = request.headers();
  }
  let postData = request.postData() ?? null;
  let binaryBodyMeta = null;
  const uploadCommandHeader =
    headers["x-goog-upload-command"] ??
    headers["X-Goog-Upload-Command"] ??
    null;
  if (
    method === "POST" &&
    typeof request.url() === "string" &&
    request.url().includes("push.clients6.google.com/upload/") &&
    typeof uploadCommandHeader === "string" &&
    /upload,\s*finalize/i.test(uploadCommandHeader)
  ) {
    try {
      const bodyBuffer = await request.postDataBuffer();
      if (bodyBuffer?.length) {
        const sha256 = createHash("sha256").update(bodyBuffer).digest("hex");
        const fileName = `upload-finalize-body-${sha256.slice(0, 12)}.bin`;
        const outputPath = path.join(outDir, fileName);
        writeFileSync(outputPath, bodyBuffer);
        binaryBodyMeta = {
          byteLength: bodyBuffer.length,
          sha256,
          fileName,
          outputPath,
        };
        finalizeUploadCapture = {
          byteLength: bodyBuffer.length,
          sha256,
          fileName,
          outputPath,
          uploadUrl: request.url(),
          headers,
        };
        postData = null;
      }
    } catch {
      // ignore binary capture failures and keep textual metadata only
    }
  }
  pushEvent({
    kind: "request",
    ts: new Date().toISOString(),
    method,
    resourceType,
    url: request.url(),
    headers,
    postData,
    binaryBodyMeta,
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

  let responseHeaders = response.headers();
  try {
    responseHeaders = await response.allHeaders();
  } catch {
    responseHeaders = response.headers();
  }
  const contentType = responseHeaders["content-type"] ?? "";
  const event = {
    kind: "response",
    ts: new Date().toISOString(),
    method,
    resourceType,
    url: response.url(),
    status: response.status(),
    contentType,
    headers: responseHeaders,
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

await page.getByRole("button", { name: /制作图片|Create image|Create images|Make image/i }).first().click({
  timeout: 30_000,
});
await page.waitForTimeout(1_000);

await uploadReferenceImage(page, sampleImagePath);
await page.waitForTimeout(3_000);

const textbox = page
  .locator(
    '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
  )
  .first();
await textbox.click({ timeout: 30_000 });
await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
await page.keyboard.type(prompt, { delay: 16 });
await page.waitForTimeout(400);
const sendButton = page.getByRole("button", { name: /发送|Send|提交/i }).first();
await sendButton.click({ timeout: 30_000, force: true }).catch(() => undefined);
await page.waitForTimeout(800);
if (!markerTransport) {
  await page.keyboard.press("Enter").catch(() => undefined);
}

const waitUntil = Date.now() + 5 * 60 * 1000;
while (Date.now() < waitUntil && !markerTransport) {
  await page.waitForTimeout(1_000);
}

const imageWaitUntil = Date.now() + 12 * 60 * 1000;
let imageNodes = [];
let anchorNodes = [];
let cssBgNodes = [];
let canvasNodes = [];
let buttonNodes = [];
let pageState = null;
let exportedPageBlobs = [];
let hasGeneratedImage = false;
let hasAiGeneratedImage = false;
let hasUploadedPreviewImage = false;
let hasInterestingAnchor = false;
let hasInterestingCssBackground = false;
let hasLargeCanvas = false;
let hasDownloadUi = false;
let strongSuccessSignal = false;
let styleSelectionHandled = false;
while (Date.now() < imageWaitUntil) {
  if (!styleSelectionHandled) {
    const styleSelectionVisible = await page
      .getByText(/为图片选择风格|选择风格|Choose a style/i)
      .first()
      .isVisible({ timeout: 1_000 })
      .catch(() => false);
    if (styleSelectionVisible) {
      const styleLocators = [
        page.locator('img[alt*="珐琅胸针"]').first(),
        page.getByText(/珐琅胸针|Enamel pin/i).first(),
        page.locator('img[alt*="单色"]').first(),
        page.getByText(/单色|Monochrome/i).first(),
      ];
      for (const locator of styleLocators) {
        try {
          await locator.click({ timeout: 5_000, force: true });
          await page.waitForTimeout(800);
          await sendButton.click({ timeout: 10_000, force: true }).catch(() => undefined);
          await page.keyboard.press("Enter").catch(() => undefined);
          styleSelectionHandled = true;
          break;
        } catch {
          // try next style selector
        }
      }
    }
  }

  ({ imageNodes, anchorNodes, cssBgNodes, canvasNodes, buttonNodes, pageState } = await page.evaluate(() => {
    const imageNodes = Array.from(document.querySelectorAll("img"))
      .map((img, i) => ({
        i,
        alt: img.getAttribute("alt"),
        src: img.getAttribute("src"),
        width: img.naturalWidth,
        height: img.naturalHeight,
      }))
      .filter((item) => item.src && !item.src.startsWith("data:image/gif"));

    const anchorNodes = Array.from(document.querySelectorAll("a"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        href: node.getAttribute("href"),
      }))
      .filter((node) => node.href || node.text);

    const cssBgNodes = Array.from(document.querySelectorAll("*"))
      .map((node, i) => {
        const style = getComputedStyle(node);
        const backgroundImage = style.backgroundImage || "";
        if (backgroundImage === "none") {
          return null;
        }
        if (!/googleusercontent|gg-dl|rd-gg-dl|data:image|blob:/i.test(backgroundImage)) {
          return null;
        }
        return {
          i,
          tag: node.tagName,
          className: String(node.className || "").slice(0, 200),
          backgroundImage: backgroundImage.slice(0, 800),
          width: node.clientWidth,
          height: node.clientHeight,
          text: (node.textContent || "").trim().slice(0, 200),
        };
      })
      .filter(Boolean);

    const canvasNodes = Array.from(document.querySelectorAll("canvas"))
      .map((node, i) => ({
        i,
        width: node.width,
        height: node.height,
        clientWidth: node.clientWidth,
        clientHeight: node.clientHeight,
      }))
      .filter((node) => (node.width ?? 0) >= 256 || (node.height ?? 0) >= 256);

    const buttonNodes = Array.from(document.querySelectorAll("button"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        aria: node.getAttribute("aria-label"),
      }))
      .filter((node) => node.text || node.aria);

    return {
      imageNodes,
      anchorNodes,
      cssBgNodes,
      canvasNodes,
      buttonNodes,
      pageState: {
        url: location.href,
        title: document.title,
        bodyText: document.body.innerText.slice(0, 12000),
      },
    };
  }));

  hasGeneratedImage = imageNodes.some(
    (node) =>
      /googleusercontent|gstatic|blob:|data:image\/|gg-dl|rd-gg-dl/i.test(node.src) &&
      ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256),
  );
  hasAiGeneratedImage = imageNodes.some(
    (node) =>
      /AI\s*生成/i.test(node.alt ?? "") &&
      /googleusercontent|gstatic|blob:|data:image\/|gg-dl|rd-gg-dl/i.test(node.src) &&
      ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256),
  );
  hasUploadedPreviewImage = imageNodes.some(
    (node) =>
      /上传图片的预览图/i.test(node.alt ?? "") &&
      /googleusercontent|gstatic|blob:|data:image\/|gg-dl|rd-gg-dl/i.test(node.src) &&
      ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256),
  );
  hasInterestingAnchor = anchorNodes.some((node) =>
    /googleusercontent|gg-dl|rd-gg-dl|\.png(\?|$)|\.jpg(\?|$)|\.jpeg(\?|$)|\.webp(\?|$)/i.test(
      node.href ?? "",
    ),
  );
  hasInterestingCssBackground = cssBgNodes.some(
    (node) =>
      /googleusercontent|gg-dl|rd-gg-dl|data:image|blob:/i.test(node.backgroundImage ?? "") &&
      (((node.width ?? 0) >= 256) || ((node.height ?? 0) >= 256)),
  );
  hasLargeCanvas = canvasNodes.length > 0;
  const hasMarkerResponse = Boolean(
    markerTransport &&
      events.some((event) => event.kind === "response" && event.url === markerTransport.url),
  );
  hasDownloadUi = buttonNodes.some((node) =>
    /(download|下载|save|保存|open|打开)/i.test(`${node.text ?? ""} ${node.aria ?? ""}`),
  );
  strongSuccessSignal =
    hasAiGeneratedImage ||
    hasInterestingAnchor ||
    hasInterestingCssBackground ||
    hasDownloadUi;

  if (markerResponseSeenAt && strongSuccessSignal) {
    if (
      secondarySignalerPollSeenAt &&
      !secondarySignalerPollCompletedAt &&
      Date.now() - secondarySignalerPollSeenAt < 270_000
    ) {
      await page.waitForTimeout(2_000);
      continue;
    }
    break;
  }
  if (
    hasMarkerResponse &&
    markerResponseSeenAt &&
    Date.now() - markerResponseSeenAt >= 180_000 &&
    strongSuccessSignal
  ) {
    if (
      secondarySignalerPollSeenAt &&
      !secondarySignalerPollCompletedAt &&
      Date.now() - secondarySignalerPollSeenAt < 270_000
    ) {
      await page.waitForTimeout(2_000);
      continue;
    }
    break;
  }
  await page.waitForTimeout(2_000);
}

await page.screenshot({
  path: path.join(outDir, "page.png"),
  fullPage: true,
});
const finalHtml = await page.content();
writeFileSync(path.join(outDir, "page.html"), finalHtml, "utf8");

const pageBlobCaptures = await page.evaluate(async () => {
  const nodes = Array.from(document.querySelectorAll("img"))
    .map((img, i) => ({
      i,
      alt: img.getAttribute("alt") || "",
      src: img.getAttribute("src") || "",
    }))
    .filter((node) => node.src.startsWith("blob:"));

  const toBase64 = (bytes) => {
    let binary = "";
    const chunkSize = 0x8000;
    for (let offset = 0; offset < bytes.length; offset += chunkSize) {
      const slice = bytes.subarray(offset, offset + chunkSize);
      binary += String.fromCharCode(...slice);
    }
    return btoa(binary);
  };

  const captures = [];
  for (const node of nodes) {
    try {
      const response = await fetch(node.src);
      const mimeType = response.headers.get("content-type") || "application/octet-stream";
      const bytes = new Uint8Array(await response.arrayBuffer());
      captures.push({
        ...node,
        mimeType,
        byteLength: bytes.length,
        dataBase64: toBase64(bytes),
      });
    } catch (error) {
      captures.push({
        ...node,
        error: String(error),
      });
    }
  }
  return captures;
});

const extensionForMimeType = (mimeType) => {
  const normalized = String(mimeType || "").toLowerCase();
  if (normalized.includes("jpeg") || normalized.includes("jpg")) return "jpg";
  if (normalized.includes("png")) return "png";
  if (normalized.includes("webp")) return "webp";
  if (normalized.includes("gif")) return "gif";
  return "bin";
};

const slugifyLabel = (value) =>
  String(value || "")
    .toLowerCase()
    .replace(/ai\s*生成/g, "ai-generated")
    .replace(/所上传图片的预览图/g, "uploaded-preview")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48) || "blob";

exportedPageBlobs = pageBlobCaptures.map((capture) => {
  if (!capture?.dataBase64) {
    return {
      i: capture?.i ?? null,
      alt: capture?.alt ?? "",
      src: capture?.src ?? "",
      error: capture?.error ?? "missing blob data",
    };
  }
  const ext = extensionForMimeType(capture.mimeType);
  const label = slugifyLabel(capture.alt);
  const fileName = `page-blob-${label}-${capture.i}.${ext}`;
  const outputPath = path.join(outDir, fileName);
  const buffer = Buffer.from(capture.dataBase64, "base64");
  writeFileSync(outputPath, buffer);
  return {
    i: capture.i,
    alt: capture.alt,
    src: capture.src,
    mimeType: capture.mimeType,
    byteLength: capture.byteLength,
    sha256: createHash("sha256").update(buffer).digest("hex"),
    fileName,
    outputPath,
  };
});

const storageStatePath = path.join(outDir, "storage-state.json");
await context.storageState({ path: storageStatePath });

const artifact = {
  ok: strongSuccessSignal,
  capturedAt: new Date().toISOString(),
  runtimeStateObjectKey: path.relative(storageRoot, latestProfileDir.fullPath).replaceAll(path.sep, "/"),
  profileDir: latestProfileDir.fullPath,
  profileSource: latestProfileDir.source,
  sampleImagePath,
  prompt,
  uploadTrigger,
  markerTransport,
  markerRequestSeenAt,
  markerResponseSeenAt,
  secondarySignalerPollUrl,
  secondarySignalerPollSeenAt,
  secondarySignalerPollCompletedAt,
  finalizeUploadCapture,
  imageNodes,
  anchorNodes,
  cssBgNodes,
  canvasNodes,
  buttonNodes,
  pageState,
  exportedPageBlobs,
  pageUrl: page.url(),
  strongSuccessSignal,
  hasGeneratedImage,
  hasAiGeneratedImage,
  hasUploadedPreviewImage,
  hasInterestingAnchor,
  hasInterestingCssBackground,
  hasLargeCanvas,
  hasDownloadUi,
  events,
  note: "Focused broad Gemini Canvas image edit probe only.",
};

writeFileSync(path.join(outDir, "image-edit-broad.json"), `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

console.log(
  JSON.stringify(
    {
      ok: artifact.ok,
      outDir,
      runtimeStateObjectKey: artifact.runtimeStateObjectKey,
      uploadTrigger,
      markerTransportKind: markerTransport?.kind ?? null,
      markerTransportUrl: markerTransport?.url ?? null,
      exportedPageBlobCount: exportedPageBlobs.length,
      imageNodeCount: imageNodes.length,
      anchorNodeCount: anchorNodes.length,
      pageUrl: artifact.pageUrl,
      strongSuccessSignal: artifact.strongSuccessSignal,
      hasAiGeneratedImage: artifact.hasAiGeneratedImage,
      hasUploadedPreviewImage: artifact.hasUploadedPreviewImage,
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
