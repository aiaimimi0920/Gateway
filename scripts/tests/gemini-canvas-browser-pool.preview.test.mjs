import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { app, appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { BROWSER_POOL_TEXT_BODY_LIMIT_BYTES } from "../gemini-canvas-browser-pool-body.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));
const pageOf = (...frames) => ({ frames: () => [{ url: () => "https://gemini.google.com/app" }, ...frames] });

function frameFixture(options = {}) {
  const calls = [], timers = new Map(), messages = [], sockets = [], establishes = [];
  let sequence = 0, initializations = 0;
  const responseText = "response ".repeat(200);
  class Socket {
    static OPEN = 1;
    constructor(...args) { sockets.push(args); }
  }
  class Manager {
    async establish(...args) { establishes.push([this.endpoint, ...args]); return "established"; }
  }
  const url = options.url ?? "https://preview.scf.usercontent.goog/frame";
  const window = { chrome: { retained: true }, WebSocket: Socket, __authIndexReady: false,
    postMessage: (...args) => { messages.push(args); if (options.postError) throw new Error("post failed"); } };
  const sandbox = vm.createContext({ window, Error, URL, AbortController,
    location: { href: url }, document: { readyState: "complete", body: { innerText: "frame ".repeat(100) } },
    ConnectionManager: Manager,
    initializeProxySystem: () => { initializations += 1; if (options.initError) throw new Error("init failed"); },
    setTimeout: (callback, ms) => { const id = ++sequence; timers.set(id, { callback, ms }); return id; },
    clearTimeout: (id) => timers.delete(id),
    fetch: (requestUrl, init) => {
      calls.push({ url: requestUrl, init });
      if (options.fetch) return options.fetch(requestUrl, init);
      return Promise.resolve({ ok: true, status: 200, url: requestUrl,
        headers: { get: (name) => name === "content-length" ? String(Buffer.byteLength(responseText)) : "application/json" },
        text: async () => responseText });
    },
  });
  if (options.noRuntime) { delete window.WebSocket; delete sandbox.ConnectionManager; delete sandbox.initializeProxySystem; }
  const frame = { url: () => url, name: () => "fixture-frame", evaluate: async (callback, input) => {
    sandbox.input = input;
    return await vm.runInContext(`(${callback.toString()})(input)`, sandbox);
  } };
  return { frame, window, sandbox, calls, timers, messages, sockets, establishes, Manager, Socket,
    initializations: () => initializations };
}

test("preview stamping skips main and unrelated frames and isolates frame evaluation failure", async () => {
  const values = [];
  const skipped = { url: () => "https://unrelated.invalid/frame", evaluate: () => assert.fail("unrelated frame") };
  const broken = { url: () => "blob:broken", evaluate: async () => { throw new Error("frame detached"); } };
  const good = { url: () => "blob:retained", evaluate: async (_, input) => { values.push(input); return { chromeContextId: input.authValue }; } };
  const result = await app.stampCanvasProxyPreviewFrames(pageOf(skipped, broken, good), " 3 ", { url: " https://fixture.invalid/probe ", method: " patch ", headers: ["invalid"], bodyText: 42 });
  assert.deepEqual(result, [{ index: 2, url: "blob:broken", stamped: false, errorMessage: "frame detached" }, { index: 3, url: "blob:retained", stamped: true, chromeContextId: 3 }]);
  assert.deepEqual(values, [{ authValue: 3, previewFetchRequest: { url: "https://fixture.invalid/probe", method: "PATCH", headers: {}, bodyText: null }, maxTextBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES }]);
  await app.stampCanvasProxyPreviewFrames(pageOf(good), "invalid", { url: " " });
  assert.deepEqual(values[1], { authValue: 0, previewFetchRequest: null, maxTextBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES });
});

test("preview default probe is intercepted and returns bounded frame and response diagnostics", async () => {
  const f = frameFixture();
  const [result] = await app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  assert.equal(f.calls.length, 1);
  const { url, init } = f.calls[0];
  assert.equal(url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key=");
  assert.equal(init.method, "POST");
  assert.deepEqual(plain(init.headers), { "content-type": "application/json" });
  assert.deepEqual(JSON.parse(init.body), { contents: [{ role: "user", parts: [{ text: "Reply with exactly: ok" }] }] });
  assert.equal(init.credentials, "include");
  assert.equal(init.mode, "cors");
  assert.equal(init.signal.aborted, false);
  assert.equal(f.timers.size, 0);
  assert.equal(result.bodyPreview.length, 400);
  assert.equal(result.chromeContextId, 0);
  assert.equal(result.hasAuthIndexReady, true);
  assert.equal(result.readyState, "complete");
  assert.equal(result.probeFetchResult.bodyText.length, 1800);
  assert.equal(result.probeFetchResult.bodyPreview.length, 800);
  assert.equal(result.probeFetchResult.contentType, "application/json");
});

test("preview custom probe preserves legacy header casing and request body semantics", async () => {
  const f = frameFixture();
  await app.stampCanvasProxyPreviewFrames(pageOf(f.frame), " 7 ", { url: "https://fixture.invalid/probe", method: " put ", bodyText: "", headers: { host: "drop", HOST: "drop", Host: "legacy-title-case", ORIGIN: "drop", "sec-fetch-mode": "drop", Authorization: "synthetic", Cookie: "synthetic=1" } });
  const { init } = f.calls[0];
  assert.equal(init.method, "PUT");
  assert.equal(init.body, "");
  assert.deepEqual(plain(init.headers), { Host: "legacy-title-case", Authorization: "synthetic", Cookie: "synthetic=1" });
  assert.equal(f.window.chrome._contextId, 7);
  assert.equal(f.window.chrome.retained, true);
});

test("preview probe adds an absent Google key while preserving existing and malformed URLs", async () => {
  const f = frameFixture();
  for (const url of ["https://generativelanguage.googleapis.com/probe", "https://generativelanguage.googleapis.com/probe?key=synthetic", "not a URL"]) {
    await app.stampCanvasProxyPreviewFrames(pageOf(f.frame), null, { url });
  }
  assert.deepEqual(f.calls.map((call) => call.url), ["https://generativelanguage.googleapis.com/probe?key=", "https://generativelanguage.googleapis.com/probe?key=synthetic", "not a URL"]);
});

test("preview runtime stamping preserves WebSocket construction and patches connection establishment once", async () => {
  const f = frameFixture();
  await app.stampCanvasProxyPreviewFrames(pageOf(f.frame), "2");
  const wrapped = f.window.WebSocket, establish = f.Manager.prototype.establish;
  const socket = new wrapped("ws://127.0.0.1:9998/path?x=1", ["fixture"]);
  new wrapped("ws://127.0.0.1:99980/path");
  assert.equal(socket instanceof f.Socket, true);
  assert.equal(wrapped.OPEN, 1);
  assert.deepEqual(plain(f.sockets), [["wss://127.0.0.1:9998/path?x=1", ["fixture"]], ["ws://127.0.0.1:99980/path"]]);
  const manager = new f.Manager(); manager.endpoint = "ws://127.0.0.1:9998?x=1";
  assert.equal(await manager.establish("argument"), "established");
  assert.deepEqual(f.establishes, [["wss://127.0.0.1:9998?x=1", "argument"]]);
  await app.stampCanvasProxyPreviewFrames(pageOf(f.frame), "4");
  assert.equal(f.window.WebSocket, wrapped);
  assert.equal(f.Manager.prototype.establish, establish);
  assert.equal(f.initializations(), 1);
  assert.equal(f.calls.length, 2);
  assert.deepEqual(plain(f.messages), [[{ type: "authIndexResponse", authIndex: 2 }, "*"], [{ type: "authIndexResponse", authIndex: 4 }, "*"]]);
});

test("preview stamping tolerates absent runtime and failed post or initialization", async () => {
  const absent = frameFixture({ noRuntime: true });
  const [result] = await app.stampCanvasProxyPreviewFrames(pageOf(absent.frame));
  assert.equal(result.wsRewriteInstalled, false);
  assert.equal(result.connectionManagerPatched, false);
  assert.equal(result.hasInitializeProxySystem, false);
  const broken = frameFixture({ postError: true, initError: true });
  await app.stampCanvasProxyPreviewFrames(pageOf(broken.frame));
  await app.stampCanvasProxyPreviewFrames(pageOf(broken.frame));
  assert.equal(broken.initializations(), 1);
  assert.equal(broken.calls.length, 2);
});

test("preview rejected probe clears its abort deadline and preserves the failure", async () => {
  const f = frameFixture({ fetch: () => Promise.reject(new Error("synthetic rejection")) });
  const [result] = await app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  assert.deepEqual(plain(result.probeFetchResult), { ok: false, errorMessage: "synthetic rejection" });
  assert.equal(f.timers.size, 0);
  assert.equal(f.calls[0].init.signal.aborted, false);
});

test("preview synchronous fetch and response-read errors stay inside probe results", async () => {
  const sync = frameFixture({ fetch: () => { throw new Error("sync fetch"); } });
  const [a] = await app.stampCanvasProxyPreviewFrames(pageOf(sync.frame));
  assert.equal(a.probeFetchResult.errorMessage, "sync fetch");
  assert.equal(sync.timers.size, 0);
  assert.equal(sync.calls[0].init.signal.aborted, false);
  const body = frameFixture({ fetch: () => Promise.resolve({
    headers: { get: (name) => name === "content-length" ? "1" : "text/plain" },
    text: async () => { throw new Error("body read"); },
  }) });
  const [b] = await app.stampCanvasProxyPreviewFrames(pageOf(body.frame));
  assert.equal(b.probeFetchResult.errorMessage, "body read");
  assert.equal(body.timers.size, 0);
});

test("preview pending probe keeps its deadline until abort settles the failure", async () => {
  const f = frameFixture({ fetch: (_, init) => new Promise((resolve, reject) => {
    init.signal.addEventListener("abort", () => reject(new Error("synthetic deadline")), { once: true });
  }) });
  const pending = app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  assert.equal(f.timers.size, 1);
  const timer = [...f.timers.values()][0];
  assert.equal(timer.ms, 15000);
  assert.equal(f.calls[0].init.signal.aborted, false);
  timer.callback();
  const [result] = await pending;
  assert.equal(f.calls[0].init.signal.aborted, true);
  assert.deepEqual(plain(result.probeFetchResult), { ok: false, errorMessage: "synthetic deadline" });
  assert.equal(f.timers.size, 0);
});

test("preview probe refuses unknown-size fallback before response.text", async () => {
  let reads = 0;
  const f = frameFixture({ fetch: () => Promise.resolve({
    ok: true,
    status: 200,
    url: "https://fixture.invalid/probe",
    headers: { get: () => null },
    text: () => { reads++; return Promise.resolve("unbounded"); },
  }) });
  const [result] = await app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  assert.equal(reads, 0);
  assert.match(result.probeFetchResult.errorMessage, /no declared size/);
  assert.equal(f.timers.size, 0);
});

test("preview successful headers clear the deadline before a deferred response body", async () => {
  let releaseBody, markBodyStarted;
  const body = new Promise((resolve) => { releaseBody = resolve; });
  const bodyStarted = new Promise((resolve) => { markBodyStarted = resolve; });
  const f = frameFixture({ fetch: (url) => Promise.resolve({ ok: true, status: 200, url,
    headers: { get: (name) => name === "content-length" ? String(Buffer.byteLength("deferred response")) : "text/plain" },
    text: () => { markBodyStarted(); return body; } }) });
  const pending = app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  await bodyStarted;
  try {
    assert.equal(f.timers.size, 0);
    assert.equal(f.calls[0].init.signal.aborted, false);
  } finally {
    releaseBody("deferred response");
  }
  const [result] = await pending;
  assert.equal(result.probeFetchResult.bodyText, "deferred response");
  assert.equal(f.timers.size, 0);
});

test("preview probe rejects a declared oversized text body before reading it", async () => {
  const f = frameFixture({ fetch: () => Promise.resolve({
    ok: true,
    status: 200,
    url: "https://fixture.invalid/probe",
    headers: { get: (name) => name === "content-length" ? String(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES + 1) : "application/json" },
    text: () => { throw new Error("probe body reader must not run"); },
  }) });
  const [result] = await app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  assert.match(result.probeFetchResult.errorMessage, /body exceeded/);
  assert.doesNotMatch(result.probeFetchResult.errorMessage, /probe body reader must not run/);
  assert.equal(f.timers.size, 0);
});

test("preview probe ignores representation length for a bodyless response", async () => {
  let reads = 0;
  const f = frameFixture({ fetch: () => Promise.resolve({
    ok: false,
    status: 304,
    url: "https://fixture.invalid/probe",
    body: null,
    headers: { get: (name) => name === "content-length" ? String(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES + 1) : "application/json" },
    text() { reads++; throw new Error("body reader must not run"); },
  }) });
  const [result] = await app.stampCanvasProxyPreviewFrames(pageOf(f.frame));
  assert.equal(result.probeFetchResult.status, 304);
  assert.equal(result.probeFetchResult.bodyText, "");
  assert.equal(reads, 0);
  assert.equal(f.timers.size, 0);
});

function openingPage(chosen, options = {}) {
  const choices = [], waits = [], clicks = [];
  const f = frameFixture();
  f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__ = { events: Array.from({ length: 15 }, (_, index) => ({ index })) };
  const locate = (label) => {
    const index = choices.length; choices.push(label);
    const candidate = { first: () => candidate, filter: () => candidate,
      count: async () => index <= chosen ? 1 : 0,
      waitFor: async (input) => { waits.push([index, input]); if (index < chosen) throw new Error("not visible"); },
      click: async (input) => { clicks.push([index, input]); } };
    return candidate;
  };
  const page = { url: () => "https://gemini.google.com/u/3/app", frames: () => [f.frame],
    getByRole: (role) => locate(role), locator: (selector) => selector === "iframe" ? { count: async () => 0 } : locate(selector),
    addInitScript: async () => {}, waitForTimeout: async () => {},
    evaluate: async (callback, input) => {
      if (options.evaluateError) throw new Error("unavailable");
      if (input?.authIndexValue) return { authIndex: input.authIndexValue, eventCount: 15 };
      if (callback.toString().includes("querySelectorAll")) return [];
      return await f.frame.evaluate(callback, input);
    } };
  return { page, choices, waits, clicks };
}

for (const chosen of [0, 1, 2, 3]) {
  test(`preview opening falls through unavailable controls to candidate ${chosen}`, async () => {
    const f = openingPage(chosen);
    const result = await app.tryOpenCanvasProxyPreview(f.page, 30000);
    assert.deepEqual(f.choices.slice(0, 2), ["button", "tab"]);
    assert.match(f.choices[2], /aria-label/);
    assert.equal(f.choices[3], 'button,[role="button"],[role="tab"]');
    assert.equal(f.waits.length, chosen + 1);
    assert.deepEqual(f.waits.at(-1), [chosen, { state: "visible", timeout: 4000 }]);
    assert.deepEqual(f.clicks, [[chosen, { timeout: 12000, force: true }]]);
    assert.equal(result.clicked, true);
    assert.equal(result.reason, "preview_clicked");
    assert.equal(result.bridge.authIndex, "3");
    assert.equal(result.bodyPreview.length, 600);
    assert.deepEqual(plain(result.bridgeEvents), Array.from({ length: 12 }, (_, index) => ({ index: index + 3 })));
    assert.deepEqual(result.stampedFrames, []);
  });
}

test("preview missing controls and unavailable evaluation retain the fallback result", async () => {
  const f = openingPage(-1, { evaluateError: true });
  const result = await app.tryOpenCanvasProxyPreview(f.page, 500);
  assert.deepEqual(result, { clicked: false, reason: "preview_button_not_clickable", pageUrl: f.page.url(), bodyPreview: "", bridge: { authIndex: "3", installedAt: null, eventCount: 0 }, stampedFrames: [] });
  assert.deepEqual(f.clicks, []);
});

test("preview real offline page click stamps the eligible iframe and intercepts its default probe", browserTest, async (t) => {
  const f = await appFixture(t, { initialUrl: "https://gemini.google.com/u/4/app", html: (request) => {
    const url = new URL(request.url());
    if (url.hostname === "generativelanguage.googleapis.com") return '{"synthetic":"ok"}';
    if (url.hostname === "preview.scf.usercontent.goog") return "<main>frame ready</main>";
    return '<button onclick="document.querySelector(\'main\').textContent=\'Connected\'">Preview app</button><main>waiting</main><iframe id="preview" src="https://preview.scf.usercontent.goog/frame"></iframe>';
  } });
  const probes = [];
  await f.context.route("https://generativelanguage.googleapis.com/**", async (route) => {
    probes.push(route.request().method());
    await route.fulfill({ status: 200, contentType: "application/json", body: '{"synthetic":"ok"}', headers: {
      "access-control-allow-origin": "https://preview.scf.usercontent.goog",
      "access-control-allow-credentials": "true", "access-control-allow-methods": "POST, OPTIONS",
      "access-control-allow-headers": "content-type",
    } });
  });
  const result = await app.tryOpenCanvasProxyPreview(f.page, 5000);
  assert.equal(result.clicked, true);
  assert.match(result.bodyPreview, /Connected/);
  assert.deepEqual(f.state.waits, [1200]);
  assert.equal(result.stampedFrames.length, 1);
  assert.equal(result.stampedFrames[0].chromeContextId, 4);
  assert.equal(result.stampedFrames[0].probeFetchResult.ok, true);
  assert.equal(result.stampedFrames[0].probeFetchResult.bodyText, '{"synthetic":"ok"}');
  assert.equal(result.iframeNodes[0].id, "preview");
  assert.equal(result.frameUrls.length, 2);
  assert.equal(probes.filter((method) => method === "POST").length, 1);
});
