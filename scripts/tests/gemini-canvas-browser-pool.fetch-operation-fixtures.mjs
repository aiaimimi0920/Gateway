import { EventEmitter } from "node:events";
import vm from "node:vm";
import {
  assertTextWithinLimit,
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
  readFetchResponseBody,
} from "../gemini-canvas-browser-pool-body.mjs";
import { createFetchPreviewOwner } from "../gemini-canvas-browser-pool-fetch-preview.mjs";
import { createFetchPageMusicOwner } from "../gemini-canvas-browser-pool-fetch-page-music.mjs";

export const fetchBaseUrl = "https://gemini.google.com";
export const fetchProgramUrl = `${fetchBaseUrl}/app/0123456789abcdef`;
export const fetchTargetUrl = "https://fixture.invalid/result";
export const fetchCalls = (h, name) => h.calls.filter(([stage]) => stage === name);

export function fetchOperationHarness(t, app, options = {}) {
  const calls = [], counts = new Map(), inherited = () => {};
  const failure = options.failure ?? new Error("fixture fetch dependency failure");
  let capture, stops = 0;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    counts.set(stage, (counts.get(stage) ?? 0) + 1);
    if (stage === options.failureAt) throw failure;
  };
  const frame = { url: () => "https://fixture.invalid/preview" };
  const mainFrame = { url: () => fetchProgramUrl };
  const timers = new Set();
  const fallbackBody = Buffer.from(options.body ?? '{"ok":true}');
  const responseHeaders = { "content-type": options.contentType ?? "application/json" };
  if (!options.omitContentLength && !Array.isArray(options.streamChunks)) {
    responseHeaders["content-length"] = String(options.contentLength ?? fallbackBody.byteLength);
  }
  const response = {
    status: options.status ?? 201, ok: (options.status ?? 201) >= 200 && (options.status ?? 201) < 300, url: fetchTargetUrl,
    headers: new Headers(responseHeaders),
    body: options.noBody ? null : Array.isArray(options.streamChunks) ? {
      getReader() {
        let index = 0;
        return {
          async read() {
            record("stream-read");
            return index < options.streamChunks.length
              ? { done: false, value: options.streamChunks[index++] }
              : { done: true };
          },
          async cancel() { record("stream-cancel"); },
          releaseLock() { record("stream-release"); },
        };
      },
    } : undefined,
    async arrayBuffer() { record("body"); return new Uint8Array(fallbackBody).buffer; },
  };
  const nativeFetch = async (...args) => { record("fetch", ...args); return response; };
  const makePage = (name) => {
    const page = new EventEmitter();
    for (const event of ["request", "response", "websocket"]) page.on(event, inherited);
    page.url = () => name === "original" ? fetchProgramUrl : `${fetchBaseUrl}/app/fedcba9876543210`;
    page.bringToFront = async () => { record("front", page); };
    page.frames = () => { record("frames"); return options.noPreviewFrame ? [mainFrame] : [mainFrame, frame]; };
    page.waitForTimeout = async (ms) => { record("wait", ms); };
    page.evaluate = async (callback, input) => {
      record("evaluate", input);
      const error = options.evaluateFailures?.[counts.get("evaluate") - 1];
      if (error) throw error;
      const browser = {
        Error, TextEncoder, TextDecoder, AbortController, fetch: nativeFetch,
        console: { debug() {} }, btoa: (value) => Buffer.from(value, "binary").toString("base64"),
        setTimeout(_callback, ms) { record("timer", ms); const id = {}; timers.add(id); return id; },
        clearTimeout(id) { record("clear-timer"); timers.delete(id); },
      };
      return await vm.runInNewContext(`(${callback.toString()})`, browser, { timeout: 1000 })(input);
    };
    return page;
  };
  const original = makePage("original"), attached = makePage("attached"), entry = { page: original };
  const result = options.previewResult ?? { status: 200, ok: true, finalUrl: fetchTargetUrl, contentType: "application/json", headers: {}, bodyText: "fixture", bodyBase64: "Zml4dHVyZQ==" };
  const stampedFrames = options.stampedFrames ?? [];
  const context = {
    ...app, Error, Buffer, DEFAULT_TIMEOUT_MS: 5000, fetch: nativeFetch,
    assertTextWithinLimit,
    BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
    BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
    readFetchResponseBody,
    isFixtureCanvasBaseUrl: () => options.fixture === true,
    async buildGoogleFetchAuthHeaders(...args) { record("auth-headers", ...args); return { "x-goog-authuser": "1", "x-origin": "fixture-origin" }; },
    resolveProgramPageUrl(...args) { record("resolve", ...args); return options.programUrl ?? null; },
    async findAttachedGeminiAppPage(...args) { record("attached", ...args); return options.attached === false ? null : attached; },
    startNetworkCapture(page, operation) {
      record("capture", page, operation);
      capture = app.startNetworkCapture(page, operation);
      for (let i = 0; i < (options.events ?? 0); i += 1) capture.state.events.push({ i });
      for (let i = 0; i < (options.rpcs ?? 0); i += 1) capture.state.rpcCaptures.push({ i });
      return { state: capture.state, stop() { stops += 1; record("stop", entry.page); capture.stop(); } };
    },
    async ensureProgramPage(...args) { record("program", ...args); },
    async tryOpenCanvasProxyPreview(...args) { record("open", ...args); return { opened: true }; },
    isCanvasProxyPreviewFrameUrl: (url) => url === frame.url(),
    async ensureCanvasProxyPreviewFrame(...args) { record("preview", ...args); return { page: entry.page, frame, stampedFrames }; },
    async executeCanvasProxyPreviewNoKeyFetch(...args) { record("preview-fetch", ...args); return options.onPreview ? await options.onPreview() : result; },
    async executeCanvasProxyPreviewNoKeyMusic(...args) { record("preview-music", ...args); return result; },
    async executeCanvasProgramPageNoKeyMusic(...args) { record("page-music", ...args); return options.onPageMusic ? await options.onPageMusic() : result; },
    async downloadBinaryViaNavigation(...args) { record("download", ...args); return result; },
    buildProgramHandleState(...args) { record("handle", ...args); return app.buildProgramHandleState(...args); },
    async ensureSharePage(...args) { record("share", ...args); },
    async ensureLoopbackConnectedClient(...args) { record("connect", ...args); },
    listConnectedClients() { record("clients"); return options.connected ? [{ id: "fixture-client" }] : []; },
    async dispatchConnectedProxyRequest(...args) { record("proxy", ...args); return { status: 202, headers: { "Content-Type": "text/plain" }, bodyText: "proxy result" }; },
    log(...args) { calls.push(["log", ...args]); },
  };
  const { runFetchPreviewOperation } = createFetchPreviewOwner(context);
  context.runFetchPreviewOperation = vm.runInNewContext(`(${runFetchPreviewOperation.toString()})`, context, { timeout: 1000 });
  const { runFetchPageMusicOperation } = createFetchPageMusicOwner(context);
  context.runFetchPageMusicOperation = vm.runInNewContext(`(${runFetchPageMusicOperation.toString()})`, context, { timeout: 1000 });
  const run = vm.runInNewContext(`(${app.runFetchOperation.toString()})`, context, { timeout: 1000 });
  t.after(() => { capture?.stop(); original.removeAllListeners(); attached.removeAllListeners(); });
  return {
    calls, entry, original, attached, frame, failure, result, stampedFrames, timers, inherited,
    get state() { return capture?.state; }, get stops() { return stops; },
    run(args = {}, request = {}) {
      return run(entry, { baseUrl: fetchBaseUrl, googleFetchMode: "canvas_preview_no_key", ...args, fetchRequest: { url: fetchTargetUrl, ...request } });
    },
  };
}

export function assertFetchReleased(assert, h) {
  assert.equal(h.entry.page, h.original);
  assert.equal(h.stops, 1);
  assert.equal(fetchCalls(h, "stop")[0][1], h.original, "restore original page before stopping capture");
  for (const page of [h.original, h.attached]) {
    for (const event of ["request", "response", "websocket"]) assert.deepEqual(page.listeners(event), [h.inherited]);
  }
  const state = structuredClone(h.state);
  h.attached.emit("websocket", { url: () => "wss://fixture.invalid/late" });
  h.original.emit("websocket", { url: () => "wss://fixture.invalid/late" });
  assert.deepEqual(h.state, state);
  assert.equal(h.timers.size, 0);
}
