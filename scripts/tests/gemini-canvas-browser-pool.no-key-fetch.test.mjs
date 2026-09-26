import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { executorFixture, plain } from "./gemini-canvas-browser-pool.executor-fixtures.mjs";
import { BROWSER_POOL_BINARY_BODY_LIMIT_BYTES } from "../gemini-canvas-browser-pool-body.mjs";

const request = { url: "https://generativelanguage.googleapis.com/v1/synthetic", method: "POST", bodyText: "synthetic" };
const run = (f, input = request, timeout = 4500) => app.executeCanvasProxyPreviewNoKeyFetch(f.target, input, timeout);

test("no-key fetch preserves request and text response contracts without a module closure", async () => {
  const f = executorFixture({ fetch: async () => new Response("synthetic text", { status: 201, headers: { "content-type": "text/plain", "x-fixture": "yes" } }) });
  const result = await run(f, { ...request, headers: { host: "drop", HOST: "drop", Host: "legacy", origin: "drop", "sec-fetch-mode": "drop", Authorization: "synthetic", "content-type": "text/plain" } });
  const [url, init] = f.requests[0];
  assert.equal(url, request.url + "?key=");
  assert.equal(init.method, "POST");
  assert.equal(init.body, "synthetic");
  assert.equal(init.credentials, "include");
  assert.equal(init.mode, "cors");
  assert.deepEqual(plain(init.headers), { Host: "legacy", Authorization: "synthetic", "content-type": "text/plain" });
  assert.deepEqual(plain(result), { status: 201, ok: true, finalUrl: null, contentType: "text/plain", headers: { "content-type": "text/plain", "x-fixture": "yes" }, bodyText: "synthetic text", bodyBase64: Buffer.from("synthetic text").toString("base64") });
  assert.equal(f.timers.size, 0);
});

test("no-key fetch preserves binary bytes across base64 chunk boundaries and HTTP failure status", async () => {
  const bytes = Uint8Array.from({ length: 70001 }, (_, index) => index % 256);
  const f = executorFixture({ fetch: async () => new Response(bytes, { status: 503, headers: { "content-type": "application/octet-stream" } }) });
  const result = await run(f, { url: "https://fixture.invalid/data", method: "GET" });
  assert.equal(f.requests[0][1].body, undefined);
  assert.equal(result.status, 503);
  assert.equal(result.ok, false);
  assert.equal(result.bodyText, null);
  assert.deepEqual(Buffer.from(result.bodyBase64, "base64"), Buffer.from(bytes));
  assert.equal(f.timers.size, 0);
});

test("no-key fetch retains existing key and malformed URL handling", async () => {
  const f = executorFixture({ fetch: async () => new Response("") });
  for (const url of [request.url + "?key=synthetic", "https://fixture.invalid/data", " invalid-url ", " "]) {
    await run(f, { ...request, url });
  }
  assert.deepEqual(f.requests.map(([url]) => url), [request.url + "?key=synthetic", "https://fixture.invalid/data", "invalid-url", null]);
  assert.equal(f.timers.size, 0);
});

for (const synchronous of [false, true]) {
  test(`no-key fetch maps failure and clears its timer when synchronous is ${synchronous}`, async () => {
    const error = new TypeError("synthetic fetch failure");
    const f = executorFixture({ fetch: () => { if (synchronous) throw error; return Promise.reject(error); } });
    assert.deepEqual(plain(await run(f)), { status: 599, ok: false, finalUrl: null, contentType: null, headers: {}, bodyText: null, bodyBase64: null, errorMessage: error.message, errorName: "TypeError" });
    assert.equal(f.timers.size, 0);
  });
}

test("no-key fetch timeout aborts the pending request and clears its deadline", async () => {
  const f = executorFixture({ fetch: (_, init) => new Promise((resolve, reject) => {
    init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
  }) });
  const pending = run(f, request, 37);
  assert.equal(f.timers.size, 1);
  await f.fire(37);
  const result = await pending;
  assert.equal(result.status, 599);
  assert.equal(result.errorName, "AbortError");
  assert.equal(result.errorMessage, "Canvas preview no-key fetch timeout");
  assert.equal(f.requests[0][1].signal.aborted, true);
  assert.equal(f.timers.size, 0);
});

test("no-key fetch keeps its deadline through body reading and maps body rejection", async () => {
  let bodyStarted, rejectBody;
  const started = new Promise((resolve) => { bodyStarted = resolve; });
  const f = executorFixture({ fetch: async () => ({ status: 200,
    headers: { get: (name) => name === "content-length" ? "1" : null, forEach() {} }, arrayBuffer: () => {
    bodyStarted(); return new Promise((resolve, reject) => { rejectBody = reject; });
  } }) });
  const pending = run(f);
  await started;
  assert.equal(f.timers.size, 1);
  rejectBody(new Error("synthetic body read"));
  const result = await pending;
  assert.equal(result.errorMessage, "synthetic body read");
  assert.equal(f.timers.size, 0);
});

test("no-key fetch refuses unknown-size fallback before arrayBuffer", async () => {
  let reads = 0;
  const f = executorFixture({ fetch: async () => ({
    status: 200,
    ok: true,
    url: request.url,
    headers: { get: () => null, forEach() {} },
    arrayBuffer() { reads++; return new ArrayBuffer(0); },
  }) });
  const result = await run(f);
  assert.equal(reads, 0);
  assert.match(result.errorMessage, /no declared size/);
  assert.equal(f.timers.size, 0);
});

test("no-key fetch rejects a declared oversized response before reading its body", async () => {
  const f = executorFixture({ fetch: async () => ({
    status: 200,
    ok: true,
    url: request.url,
    headers: {
      get(name) { return name === "content-length" ? String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1) : null; },
      forEach() {},
    },
    arrayBuffer() { throw new Error("body reader must not run"); },
  }) });
  const result = await run(f);
  assert.equal(result.status, 599);
  assert.match(result.errorMessage, /body exceeded/);
  assert.doesNotMatch(result.errorMessage, /body reader must not run/);
  assert.equal(f.timers.size, 0);
});

test("no-key fetch ignores representation length for a bodyless response", async () => {
  let reads = 0;
  const f = executorFixture({ fetch: async () => ({
    status: 304,
    ok: false,
    url: request.url,
    body: null,
    headers: {
      get(name) { return name === "content-length" ? String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1) : "application/octet-stream"; },
      forEach() {},
    },
    arrayBuffer() { reads++; throw new Error("body reader must not run"); },
  }) });
  const result = await run(f, { ...request, method: "HEAD" });
  assert.equal(result.status, 304);
  assert.equal(result.bodyText, null);
  assert.equal(result.bodyBase64, "");
  assert.equal(reads, 0);
  assert.equal(f.timers.size, 0);
});

test("no-key fetch executes in a real offline frame and returns intercepted response bytes", browserTest, async (t) => {
  const f = await appFixture(t, { html: (input) => new URL(input.url()).hostname === "preview.scf.usercontent.goog"
    ? "<main>synthetic preview</main>" : '<iframe src="https://preview.scf.usercontent.goog/frame"></iframe>' });
  const captured = [];
  await f.context.route("https://preview.scf.usercontent.goog/probe", async (route) => {
    captured.push({ method: route.request().method(), body: route.request().postData() });
    await route.fulfill({ status: 202, contentType: "application/json", body: '{"synthetic":"ok"}' });
  });
  const frame = f.page.frames().find((item) => item.url().includes("preview.scf.usercontent.goog"));
  assert.ok(frame);
  const result = await app.executeCanvasProxyPreviewNoKeyFetch(frame, { url: "https://preview.scf.usercontent.goog/probe", method: "POST", bodyText: "offline" }, 5000);
  assert.deepEqual(captured, [{ method: "POST", body: "offline" }]);
  assert.equal(result.status, 202);
  assert.equal(result.bodyText, '{"synthetic":"ok"}');
  assert.equal(Buffer.from(result.bodyBase64, "base64").toString(), result.bodyText);
  assert.equal(result.finalUrl, "https://preview.scf.usercontent.goog/probe");
});
