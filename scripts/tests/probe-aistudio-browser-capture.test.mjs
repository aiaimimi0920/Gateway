import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test, { mock } from "node:test";
import { createBrowserCapture } from "../aistudio-live-probe/browser-capture.mjs";

const tick = () => new Promise((resolve) => setImmediate(resolve));

function fixture() {
  const context = new EventEmitter();
  const page = new EventEmitter();
  const capture = { requests: [], responses: [], websockets: [], pageErrors: [], console: [] };
  let writes = 0;
  const lifecycle = createBrowserCapture({
    capture, requestUrlIncludes: ["ai.studio"], responseBodyLimit: 4,
    persistCapture: async () => { writes++; },
  });
  lifecycle.attach(context, page);
  return { context, page, capture, lifecycle, writes: () => writes };
}

function request(url = "https://ai.studio/test") {
  return {
    url: () => url, method: () => "POST", resourceType: () => "fetch",
    allHeaders: async () => ({ "content-type": "application/json" }),
    postData: () => "body", frame: () => ({ url: () => "https://ai.studio/app" }),
  };
}

test("browser capture admits only activated matching network events", async () => {
  const f = fixture();
  f.context.emit("request", request());
  await tick();
  assert.equal(f.capture.requests.length, 0);
  f.lifecycle.activate();
  f.context.emit("request", request("https://example.invalid/"));
  await tick();
  assert.equal(f.writes(), 0);
  assert.deepEqual(f.lifecycle.getMatchTimes(), { firstMatchAt: null, lastMatchAt: null });
});

test("browser capture preserves request identity, response content selection and live match times", async () => {
  const f = fixture();
  f.lifecycle.activate();
  const req = request();
  f.context.emit("request", req);
  await tick();
  const response = {
    url: req.url, request: () => req, status: () => 200,
    allHeaders: async () => ({ "content-type": "text/plain" }), text: async () => "alphabet",
  };
  f.context.emit("response", response);
  await tick();
  assert.equal(f.capture.requests.length, 1);
  assert.equal(f.capture.responses[0].requestId, f.capture.requests[0].id);
  assert.equal(f.capture.requests[0].source, "context");
  assert.equal(f.capture.requests[0].postDataPreview, "body");
  assert.match(f.capture.responses[0].bodyPreview, /^alph/);
  assert.ok(f.lifecycle.getMatchTimes().firstMatchAt > 0);
  assert.ok(f.lifecycle.getMatchTimes().lastMatchAt >= f.lifecycle.getMatchTimes().firstMatchAt);
  f.context.emit("response", { ...response,
    allHeaders: async () => ({ "content-type": "image/png" }),
    text: async () => { throw new Error("binary body must not be read"); },
  });
  await tick();
  assert.equal(f.capture.responses[1].bodyPreview, null);
  assert.equal(f.writes(), 3);
});

test("browser capture retains early page diagnostics and WebSocket event contracts", () => {
  const f = fixture();
  f.page.emit("pageerror", new Error("synthetic page error"));
  f.page.emit("console", { type: () => "log", text: () => "hello" });
  f.page.emit("framenavigated", { url: () => "frame-url", name: () => "" });
  assert.equal(f.capture.pageErrors.length, 1);
  assert.deepEqual(f.capture.console.map((entry) => entry.type), ["log", "frame"]);
  const ws = Object.assign(new EventEmitter(), { url: () => "wss://ai.studio/ws" });
  f.page.emit("websocket", ws);
  assert.equal(f.capture.websockets.length, 0);
  f.lifecycle.activate();
  f.page.emit("websocket", ws);
  ws.emit("framesent", { payload: "sent" });
  ws.emit("framereceived", { payload: "received" });
  ws.emit("socketerror", "synthetic socket error");
  ws.emit("close");
  assert.deepEqual(f.capture.websockets[0].framesSent, ["sent"]);
  assert.deepEqual(f.capture.websockets[0].framesReceived, ["received"]);
  assert.deepEqual(f.capture.websockets[0].errors, ["synthetic socket error"]);
  assert.equal(f.capture.websockets[0].closed, true);
});

test("browser capture propagates callback rejection safely and detaches all owned listeners", async () => {
  const f = fixture();
  f.lifecycle.activate();
  const req = request();
  req.frame = () => { throw new Error("synthetic-secret"); };
  f.context.emit("request", req);
  await tick();
  assert.throws(() => f.lifecycle.getMatchTimes(), { code: "aistudio_probe_capture_callback_failed" });
  await assert.rejects(f.lifecycle.stop(), { message: "AI Studio browser capture callback failed." });
  assert.equal(f.context.listenerCount("request"), 0);
  assert.equal(f.page.listenerCount("pageerror"), 0);
  f.lifecycle.attach(f.context, f.page);
  assert.equal(f.context.listenerCount("request"), 0);
  f.page.emit("pageerror", new Error("late"));
  assert.equal(f.capture.pageErrors.length, 0);
});

test("browser capture stop drains admitted work before allowing final publication", async () => {
  const f = fixture();
  f.lifecycle.activate();
  let complete;
  const req = request();
  req.allHeaders = () => new Promise((resolve) => { complete = resolve; });
  f.context.emit("request", req);
  await tick();
  let stopped = false;
  const stopping = f.lifecycle.stop().then(() => { stopped = true; });
  await tick();
  assert.equal(stopped, false);
  f.context.emit("request", request());
  complete({});
  await stopping;
  assert.equal(f.capture.requests.length, 1);
  assert.equal(f.writes(), 1);
});

test("browser capture bounds concurrent admission without queueing unmatched events", async () => {
  const f = fixture();
  f.lifecycle.activate();
  for (let i = 0; i < 1000; i++) f.context.emit("request", request("https://unmatched.invalid/"));
  assert.doesNotThrow(() => f.lifecycle.getMatchTimes());
  const req = request();
  req.allHeaders = () => new Promise(() => {});
  for (let i = 0; i < 65; i++) f.context.emit("request", req);
  await assert.rejects(f.lifecycle.stop(), { code: "aistudio_probe_capture_overloaded" });
  assert.equal(f.capture.requests.length, 0);
  assert.equal(f.context.listenerCount("request"), 0);
});

test("browser callback deadline cancels capture mutation after a late native completion", async () => {
  const original = globalThis.setTimeout;
  let expire;
  const timer = mock.method(globalThis, "setTimeout", (callback, ms, ...args) => {
    if (ms !== 30000) return original(callback, ms, ...args);
    expire = callback;
    return original(() => {}, ms);
  });
  try {
    const f = fixture();
    f.lifecycle.activate();
    let complete;
    const req = request();
    req.allHeaders = () => new Promise((resolve) => { complete = resolve; });
    f.context.emit("request", req);
    await tick();
    expire();
    await assert.rejects(f.lifecycle.stop(), { code: "aistudio_probe_capture_timeout" });
    complete({});
    await tick();
    assert.equal(f.capture.requests.length, 0);
    assert.equal(f.writes(), 0);
  } finally {
    timer.mock.restore();
  }
});

test("browser diagnostic floods exhaust a shared record budget and detach listeners", async () => {
  const f = fixture();
  for (let i = 0; i < 4097; i++) f.page.emit("console", { type: () => "log", text: () => "entry" });
  assert.equal(f.capture.console.length, 4096);
  await assert.rejects(f.lifecycle.stop(), { code: "aistudio_probe_capture_budget_exceeded", status: 413 });
  assert.equal(f.page.listenerCount("console"), 0);
});

test("browser rejects an oversized network header before retaining its request", async () => {
  const f = fixture();
  f.lifecycle.activate();
  const req = request();
  req.allHeaders = async () => ({ huge: "x".repeat(1024 * 1024 + 1) });
  f.context.emit("request", req);
  await assert.rejects(f.lifecycle.stop(), { code: "aistudio_probe_capture_budget_exceeded" });
  assert.equal(f.capture.requests.length, 0);
  assert.equal(f.writes(), 0);
});

test("browser WebSocket frames cannot grow one retained entry beyond its record budget", async () => {
  const f = fixture();
  f.lifecycle.activate();
  const ws = Object.assign(new EventEmitter(), { url: () => "wss://ai.studio/ws" });
  f.page.emit("websocket", ws);
  for (let i = 0; i < 300; i++) ws.emit("framereceived", { payload: "x".repeat(4096) });
  await assert.rejects(f.lifecycle.stop(), { code: "aistudio_probe_capture_budget_exceeded" });
  assert.ok(Buffer.byteLength(JSON.stringify(f.capture.websockets[0])) <= 1024 * 1024);
  assert.equal(ws.listenerCount("framereceived"), 0);
});
