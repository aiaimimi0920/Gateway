/**
 * Focused repo-side helper.
 *
 * Uses the latest persistent Gemini profile to automatically trigger one real
 * image generation from the Gemini web UI, while recording the matching
 * BardChatUi batchexecute request/response and any rendered image nodes.
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
const outDir = path.join(storageRoot, "debug", "gemini-canvas", `auto-run-${stamp}`);
mkdirSync(outDir, { recursive: true });

const prompt =
  "CANVAS_API_PROBE_CAT_20260411 一只穿宇航服的可爱猫在月球上吃热狗，电影感，高清细节，16:9";

const trackedRequests = new Map();
const batchexecuteEvents = [];
const streamGenerateEvents = [];
let matchedArtifact = null;

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

const isTargetRequest = (request) =>
  request.method().toUpperCase() === "POST" &&
  request.url().includes("gemini.google.com/_/BardChatUi/data/batchexecute");

const isStreamGenerateRequest = (request) =>
  request.method().toUpperCase() === "POST" &&
  request.url().includes("/BardFrontendService/StreamGenerate");

const context = await chromium.launchPersistentContext(latestProfileDir.fullPath, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

await context.addInitScript(() => {
  const store = (globalThis.__canvasStreamGenerateIntercepts =
    globalThis.__canvasStreamGenerateIntercepts ?? []);
  const pushIntercept = (entry) => {
    store.push({
      ts: new Date().toISOString(),
      ...entry,
    });
    if (store.length > 24) {
      store.splice(0, store.length - 24);
    }
  };
  const normalizeBody = (body) => {
    if (typeof body === "string") return body;
    if (body instanceof URLSearchParams) return body.toString();
    return null;
  };
  const recordIfStreamGenerate = (via, url, body, headers = null) => {
    try {
      const normalizedUrl = typeof url === "string" ? url : String(url ?? "");
      if (!normalizedUrl.includes("/StreamGenerate")) return;
      const normalizedBody = normalizeBody(body);
      const base = {
        via,
        url: normalizedUrl,
        headers,
        bodyType:
          body === null
            ? "null"
            : body === undefined
              ? "undefined"
              : body?.constructor?.name ?? typeof body,
        stack: new Error("canvas-stream-generate-hook").stack,
      };
      if (!normalizedBody) {
        pushIntercept(base);
        return;
      }
      const params = new URLSearchParams(normalizedBody);
      const rawFReq = params.get("f.req");
      if (!rawFReq) {
        pushIntercept({
          ...base,
          bodyText: normalizedBody,
          bodyPreview: normalizedBody.slice(0, 240),
        });
        return;
      }
      const outer = JSON.parse(rawFReq);
      const inner = JSON.parse(outer[1]);
      pushIntercept({
        ...base,
        bodyText: normalizedBody,
        bodyPreview: normalizedBody.slice(0, 240),
        rawFReq,
        inner3Head: typeof inner?.[3] === "string" ? inner[3].slice(0, 120) : null,
        inner3Len: typeof inner?.[3] === "string" ? inner[3].length : null,
        inner4: typeof inner?.[4] === "string" ? inner[4] : null,
        inner49: inner?.[49] ?? null,
        inner59: inner?.[59] ?? null,
      });
    } catch (error) {
      pushIntercept({
        via,
        url: typeof url === "string" ? url : String(url ?? ""),
        hookError: error instanceof Error ? error.message : String(error),
        stack: new Error("canvas-stream-generate-hook-error").stack,
      });
    }
  };

  const originalFetch = globalThis.fetch.bind(globalThis);
  globalThis.fetch = (...args) => {
    const [input, init] = args;
    const url =
      typeof input === "string" ? input : input?.url ?? init?.url ?? "";
    const headerEntries =
      init?.headers && typeof init.headers.entries === "function"
        ? Object.fromEntries(init.headers.entries())
        : init?.headers && !Array.isArray(init.headers)
          ? init.headers
          : null;
    recordIfStreamGenerate("fetch", url, init?.body, headerEntries);
    return originalFetch(...args);
  };

  const xhrOpen = XMLHttpRequest.prototype.open;
  const xhrSend = XMLHttpRequest.prototype.send;
  const xhrSetRequestHeader = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.open = function patchedOpen(method, url, ...rest) {
    this.__canvasHookUrl = typeof url === "string" ? url : String(url ?? "");
    this.__canvasHookMethod = method;
    this.__canvasHookHeaders = {};
    return xhrOpen.call(this, method, url, ...rest);
  };
  XMLHttpRequest.prototype.setRequestHeader = function patchedSetRequestHeader(name, value) {
    try {
      const key = typeof name === "string" ? name : String(name ?? "");
      this.__canvasHookHeaders ??= {};
      this.__canvasHookHeaders[key] = value;
    } catch {}
    return xhrSetRequestHeader.call(this, name, value);
  };
  XMLHttpRequest.prototype.send = function patchedSend(body) {
    recordIfStreamGenerate(
      "xhr",
      this.__canvasHookUrl ?? "",
      body,
      this.__canvasHookHeaders ?? null,
    );
    return xhrSend.call(this, body);
  };
});

const page = context.pages()[0] ?? (await context.newPage());

page.on("request", (request) => {
  const streamGenerate = isStreamGenerateRequest(request);
  if (!isTargetRequest(request) && !streamGenerate) return;
  const postData = request.postData() ?? null;
  const record = {
    kind: "request",
    ts: new Date().toISOString(),
    method: request.method(),
    resourceType: request.resourceType(),
    url: request.url(),
    headers: pickHeaders(request.headers()),
    postData,
    promptMatched: postData?.includes("CANVAS_API_PROBE") ?? false,
  };
  if (streamGenerate) {
    streamGenerateEvents.push(record);
    if (record.promptMatched && !matchedArtifact) {
      matchedArtifact = {
        request: record,
        response: null,
      };
    }
  } else {
    trackedRequests.set(request, record);
    batchexecuteEvents.push(record);
  }
});

page.on("response", async (response) => {
  const request = response.request();
  const streamGenerate = isStreamGenerateRequest(request);
  if (!isTargetRequest(request) && !streamGenerate) return;
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
  if (streamGenerate) {
    streamGenerateEvents.push(responseRecord);
    const markerMatched =
      request.postData()?.includes("CANVAS_API_PROBE") ||
      responseRecord.bodyText?.includes("CANVAS_API_PROBE");
    if (markerMatched) {
      matchedArtifact = {
        request:
          matchedArtifact?.request ??
          {
            kind: "request",
            ts: new Date().toISOString(),
            method: request.method(),
            resourceType: request.resourceType(),
            url: request.url(),
            headers: pickHeaders(request.headers()),
            postData: request.postData() ?? null,
            promptMatched: true,
          },
        response: responseRecord,
      };
    }
  } else {
    batchexecuteEvents.push(responseRecord);
    const markerMatched =
      requestRecord?.postData?.includes("CANVAS_API_PROBE") ||
      responseRecord.bodyText?.includes("CANVAS_API_PROBE");
    if (markerMatched && !matchedArtifact) {
      matchedArtifact = {
        request: requestRecord,
        response: responseRecord,
      };
    }
  }
});

await page.goto("https://gemini.google.com/app", {
  waitUntil: "domcontentloaded",
  timeout: 60_000,
});
await page.waitForTimeout(6_000);

const imageButton = page.getByRole("button", { name: /制作图片/ }).first();
await imageButton.click({ timeout: 30_000 });
await page.waitForTimeout(800);

const textbox = page.locator('div[role="textbox"][aria-label="为 Gemini 输入提示"]').first();
await textbox.click({ timeout: 30_000 });
await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
await page.keyboard.type(prompt, { delay: 16 });
await page.waitForTimeout(400);

const sendButton = page.getByRole("button", { name: "发送" }).first();
await sendButton.click({ timeout: 30_000 });

const waitUntil = Date.now() + 4 * 60 * 1000;
while (Date.now() < waitUntil && !matchedArtifact) {
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
  if (imageNodes.some((node) => /googleusercontent|gstatic|blob:|data:image\//i.test(node.src))) {
    break;
  }
  await page.waitForTimeout(2_000);
}

const pageState = await page.evaluate(() => ({
  url: location.href,
  title: document.title,
  bodyText: document.body.innerText.slice(0, 8000),
}));
const streamGenerateIntercepts = await page.evaluate(
  () => globalThis.__canvasStreamGenerateIntercepts ?? [],
);
const storageStatePath = path.join(outDir, "storage-state.json");
await context.storageState({ path: storageStatePath });

await page.screenshot({
  path: path.join(outDir, "page.png"),
  fullPage: true,
});

const artifact = {
  ok: true,
  capturedAt: new Date().toISOString(),
  profileDir: latestProfileDir.fullPath,
  runtimeStateObjectKey: path.relative(storageRoot, latestProfileDir.fullPath).replaceAll(path.sep, "/"),
  prompt,
  matchedArtifact,
  streamGenerateIntercepts,
  streamGenerateEvents,
  storageStatePath,
  imageNodes,
  pageState,
  batchexecuteEvents,
  note: "Focused automatic Gemini Canvas image run only.",
};
writeFileSync(path.join(outDir, "auto-run.json"), `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

console.log(
  JSON.stringify(
    {
      ok: true,
      outDir,
      matchedRequest: Boolean(matchedArtifact?.request),
      matchedResponse: Boolean(matchedArtifact?.response),
      interceptCount: streamGenerateIntercepts.length,
      streamEventCount: streamGenerateEvents.length,
      imageNodeCount: imageNodes.length,
      pageUrl: pageState.url,
      runtimeStateObjectKey: artifact.runtimeStateObjectKey,
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
