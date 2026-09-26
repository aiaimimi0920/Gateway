import assert from "node:assert/strict";
import { WebSocket, WebSocketServer } from "ws";
import test from "node:test";
import vm from "node:vm";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import {
  BROWSER_POOL_BODY_LIMIT_CODE,
  BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
} from "../gemini-canvas-browser-pool-body.mjs";

const app = await importTestableScript();
const plain = (value) => JSON.parse(JSON.stringify(value));
const send = (id, message) => app.handleConnectedClientMessage(id, JSON.stringify(message));

function protocolFixture(t) {
  const timers = new Map(), envKeys = ["GEMINI_CANVAS_BROWSER_CLIENT_API_KEY", "GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN", "GEMINI_CANVAS_CONNECTED_REQUEST_TIMEOUT_MS", "GEMINI_CANVAS_BROWSER_HOST", "GEMINI_CANVAS_BROWSER_POOL_TLS_PORT"];
  const environment = envKeys.map((key) => [key, process.env[key]]);
  for (const key of envKeys) delete process.env[key];
  assert.equal(app.connectedClients.size, 0);
  let sequence = 0;
  t.mock.method(globalThis, "setTimeout", (callback, ms) => { const id = ++sequence; timers.set(id, { callback, ms }); return id; });
  t.mock.method(globalThis, "clearTimeout", (id) => timers.delete(id));
  t.mock.method(console, "log", () => {});
  t.after(() => {
    for (const id of app.connectedClients.keys()) app.cleanupConnectedClient(id);
    for (const [key, value] of environment) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
    assert.equal(timers.size, 0, "Every synthetic request must settle or be disconnected");
  });
  const client = (id, authenticated = true, scheme = "http") => {
    const messages = [], closes = [];
    const entry = { connectionId: id, authenticated, scheme, connectedAt: "synthetic-time", clientLabel: null,
      ws: { send: (message) => messages.push(JSON.parse(message)), close: (...args) => closes.push(args) } };
    app.connectedClients.set(id, entry);
    return { entry, messages, closes };
  };
  const expire = () => { const [id, timer] = timers.entries().next().value; timers.delete(id); timer.callback(); };
  return { timers, client, expire };
}

test("connected protocol ignores malformed and unknown clients and rejects dispatch without authentication", async (t) => {
  const f = protocolFixture(t); f.client("unauthenticated", false);
  assert.doesNotThrow(() => app.handleConnectedClientMessage("unknown", "not JSON"));
  assert.doesNotThrow(() => send("unknown", { event_type: "authenticate" }));
  await assert.rejects(app.dispatchConnectedProxyRequest({}), { status: 503, code: "gemini_canvas_connected_client_unavailable" });
  assert.equal(f.timers.size, 0);
});

test("connected protocol keyless authentication truncates labels and retains fallback labels", (t) => {
  const f = protocolFixture(t), a = f.client("a", false), b = f.client("b", false);
  send("a", { event_type: "authenticate", clientLabel: ` ${"label".repeat(40)} ` });
  send("b", { event_type: "authenticate", clientLabel: " " });
  assert.equal(a.entry.clientLabel.length, 128);
  assert.equal(b.entry.clientLabel, "gemini-canvas-b");
  assert.deepEqual(a.messages, [{ event_type: "auth_ack", authorized: true, message: "" }]);
  assert.deepEqual(app.listConnectedClients(), [a.entry, b.entry]);
});

test("connected protocol authenticates with explicit key precedence and bearer fallback", (t) => {
  const f = protocolFixture(t);
  process.env.GEMINI_CANVAS_BROWSER_CLIENT_API_KEY = " synthetic-primary ";
  process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN = "synthetic-fallback";
  const wrong = f.client("wrong", false), right = f.client("right", false);
  send("wrong", { event_type: "authenticate", apiKey: "synthetic-fallback" });
  send("right", { event_type: "authenticate", apiKey: " synthetic-primary " });
  assert.deepEqual(wrong.closes, [[4001, "invalid_api_key"]]);
  assert.deepEqual(wrong.messages, [{ event_type: "auth_ack", authorized: false, message: "Invalid or missing API key" }]);
  assert.equal(right.entry.authenticated, true);
  process.env.GEMINI_CANVAS_BROWSER_CLIENT_API_KEY = " ";
  send("wrong", { event_type: "authenticate", apiKey: "synthetic-fallback" });
  assert.equal(wrong.entry.authenticated, true);
});

test("connected dispatch selects the first authenticated HTTP or HTTPS client and preserves its envelope", async (t) => {
  const f = protocolFixture(t); f.client("unsigned", false);
  const first = f.client("tls", true, "https"), second = f.client("plain");
  const request = Object.freeze({ url: "https://fixture.invalid/probe", method: "PATCH", headers: { "X-Synthetic": "value" }, body: "payload" });
  const pending = app.dispatchConnectedProxyRequest(request);
  const envelope = first.messages[0];
  assert.match(envelope.request_id, /^gemini_canvas_req_\d+_[a-z0-9]+$/);
  assert.deepEqual(envelope, { event_type: "proxy_request", request_id: envelope.request_id, request_attempt_id: `${envelope.request_id}_attempt_1`, request_attempt_number: 1, streaming_mode: "fake", is_generative: true, ...request });
  assert.deepEqual(second.messages, []);
  send("tls", { event_type: "stream_close", request_id: envelope.request_id });
  await pending;
});

test("connected response aggregation ignores unknown messages and non-string chunks", async (t) => {
  const f = protocolFixture(t), c = f.client("stream");
  const pending = app.dispatchConnectedProxyRequest({ path: "probe" }), id = c.messages[0].request_id;
  send("stream", { event_type: "response_headers", request_id: id, status: "207", headers: { "X-Result": "synthetic" } });
  for (const data of ["first", 42, null, "second"]) send("stream", { event_type: "chunk", request_id: id, data });
  send("stream", { event_type: "chunk", request_id: "unknown", data: "ignored" });
  send("stream", { event_type: "unexpected", request_id: id });
  send("stream", { event_type: "stream_close", request_id: id });
  assert.deepEqual(await pending, { status: 207, headers: { "X-Result": "synthetic" }, bodyText: "firstsecond" });
  assert.equal(f.timers.size, 0);
  assert.doesNotThrow(() => send("stream", { event_type: "error", request_id: id }));
});

test("connected frame admission preserves worst-case JSON escaping for the full body budget", async (t) => {
  const f = protocolFixture(t), c = f.client("escaped-frame");
  const pending = app.dispatchConnectedProxyRequest({ path: "escaped-frame" });
  const requestId = c.messages.at(-1).request_id;
  const body = "\u0000".repeat(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES);
  assert.equal(Buffer.byteLength(body, "utf8"), BROWSER_POOL_TEXT_BODY_LIMIT_BYTES);
  const rawMessage = JSON.stringify({ event_type: "chunk", request_id: requestId, data: body });
  assert.ok(Buffer.byteLength(rawMessage, "utf8") <= BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES);

  app.handleConnectedClientMessage("escaped-frame", rawMessage);
  send("escaped-frame", { event_type: "stream_close", request_id: requestId });
  const response = await pending;
  assert.equal(Buffer.byteLength(response.bodyText, "utf8"), BROWSER_POOL_TEXT_BODY_LIMIT_BYTES);
  assert.equal(response.bodyText, body);
});

test("connected frame admission closes oversized input before JSON parsing and releases pending work", async (t) => {
  const f = protocolFixture(t), c = f.client("oversized-frame");
  const pending = app.dispatchConnectedProxyRequest({ path: "oversized-frame" });
  const rejected = assert.rejects(pending, {
    status: 503,
    code: "gemini_canvas_connected_client_disconnected",
  });
  let parseCalls = 0;
  const originalParse = JSON.parse;
  t.mock.method(JSON, "parse", (...args) => {
    parseCalls += 1;
    return originalParse(...args);
  });

  app.handleConnectedClientMessage(
    "oversized-frame",
    Buffer.alloc(BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES + 1),
  );
  assert.deepEqual(c.closes, [[1009, "message_too_large"]]);
  assert.equal(parseCalls, 0);
  app.cleanupConnectedClient("oversized-frame");
  await rejected;
  assert.equal(f.timers.size, 0);
});

test("connected WebSocket maxPayload closes oversized frames and releases pending work", async (t) => {
  const f = protocolFixture(t), connectionId = "websocket-max-payload";
  const server = new WebSocketServer({ host: "127.0.0.1", port: 0, maxPayload: 1 });
  const serverSockets = new Set();
  let client, serverClosed;
  server.on("connection", (ws) => {
    serverSockets.add(ws);
    app.connectedClients.set(connectionId, {
      connectionId, ws, authenticated: true, scheme: "http", connectedAt: "synthetic-time", clientLabel: null,
    });
    ws.on("message", (message) => app.handleConnectedClientMessage(connectionId, message));
    ws.on("close", () => {
      app.cleanupConnectedClient(connectionId);
      serverSockets.delete(ws);
      serverClosed();
    });
    ws.on("error", () => app.cleanupConnectedClient(connectionId));
  });
  t.after(async () => {
    client?.terminate();
    for (const ws of serverSockets) ws.terminate();
    if (server.address()) await new Promise((resolve) => server.close(resolve));
  });
  await new Promise((resolve, reject) => {
    server.once("listening", resolve);
    server.once("error", reject);
  });
  client = new WebSocket(`ws://127.0.0.1:${server.address().port}`);
  client.on("error", () => {});
  await new Promise((resolve, reject) => {
    client.once("open", resolve);
    client.once("error", reject);
  });

  let markServerClosed;
  const serverCleanup = new Promise((resolve) => { markServerClosed = resolve; });
  serverClosed = markServerClosed;
  const pending = app.dispatchConnectedProxyRequest({ path: "oversized-real-frame" });
  const rejected = assert.rejects(pending, {
    status: 503,
    code: "gemini_canvas_connected_client_disconnected",
  });
  const clientClosed = new Promise((resolve) => client.once("close", (code) => resolve(code)));
  client.send(Buffer.alloc(2));

  assert.equal(await clientClosed, 1009);
  await serverCleanup;
  await rejected;
  assert.equal(app.connectedClients.has(connectionId), false);
  assert.equal(f.timers.size, 0);
}, { timeout: 5_000 });

test("connected response retention enforces the UTF-8 text budget and releases over-limit requests", async (t) => {
  const f = protocolFixture(t), c = f.client("bounded");
  const body = "é".repeat(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES / 2);
  assert.equal(Buffer.byteLength(body, "utf8"), BROWSER_POOL_TEXT_BODY_LIMIT_BYTES);

  const withinLimit = app.dispatchConnectedProxyRequest({ path: "within-limit" });
  const withinLimitId = c.messages.at(-1).request_id;
  send("bounded", { event_type: "chunk", request_id: withinLimitId, data: body.slice(0, body.length / 2) });
  send("bounded", { event_type: "chunk", request_id: withinLimitId, data: body.slice(body.length / 2) });
  send("bounded", { event_type: "stream_close", request_id: withinLimitId });
  assert.deepEqual(await withinLimit, { status: 200, headers: {}, bodyText: body });
  assert.equal(f.timers.size, 0);

  const overLimit = app.dispatchConnectedProxyRequest({ path: "over-limit" });
  const overLimitId = c.messages.at(-1).request_id;
  const rejected = assert.rejects(overLimit, (error) => {
    assert.equal(error.status, 413);
    assert.equal(error.code, BROWSER_POOL_BODY_LIMIT_CODE);
    assert.equal(error.kind, "connected client response");
    assert.equal(error.limit, BROWSER_POOL_TEXT_BODY_LIMIT_BYTES);
    assert.equal(error.actual, BROWSER_POOL_TEXT_BODY_LIMIT_BYTES + 1);
    return true;
  });
  send("bounded", { event_type: "chunk", request_id: overLimitId, data: body });
  send("bounded", { event_type: "chunk", request_id: overLimitId, data: "x" });
  await rejected;
  assert.equal(f.timers.size, 0);
  assert.doesNotThrow(() => send("bounded", { event_type: "chunk", request_id: overLimitId, data: "late" }));
  assert.doesNotThrow(() => send("bounded", { event_type: "stream_close", request_id: overLimitId }));

  const next = app.dispatchConnectedProxyRequest({ path: "after-over-limit" });
  const nextId = c.messages.at(-1).request_id;
  send("bounded", { event_type: "stream_close", request_id: nextId });
  assert.deepEqual(await next, { status: 200, headers: {}, bodyText: "" });
});

test("connected response retention bounds non-empty chunk references and ignores empty chunks", async (t) => {
  const f = protocolFixture(t), c = f.client("chunk-count");
  const withinLimit = app.dispatchConnectedProxyRequest({ path: "chunk-limit" });
  const withinLimitId = c.messages.at(-1).request_id;
  for (let index = 0; index < 1024; index += 1) {
    send("chunk-count", { event_type: "chunk", request_id: withinLimitId, data: "x" });
  }
  send("chunk-count", { event_type: "chunk", request_id: withinLimitId, data: "" });
  send("chunk-count", { event_type: "stream_close", request_id: withinLimitId });
  assert.deepEqual(await withinLimit, { status: 200, headers: {}, bodyText: "x".repeat(1024) });

  const overLimit = app.dispatchConnectedProxyRequest({ path: "too-many-chunks" });
  const overLimitId = c.messages.at(-1).request_id;
  const rejected = assert.rejects(overLimit, {
    status: 413,
    code: "gemini_canvas_connected_client_chunk_limit",
    limit: 1024,
    actual: 1025,
  });
  for (let index = 0; index < 1025; index += 1) {
    send("chunk-count", { event_type: "chunk", request_id: overLimitId, data: "x" });
  }
  await rejected;
  assert.equal(f.timers.size, 0);
  assert.doesNotThrow(() => send("chunk-count", { event_type: "stream_close", request_id: overLimitId }));
});

test("connected empty stream returns default status and normalizes invalid response headers", async (t) => {
  const f = protocolFixture(t), c = f.client("empty");
  for (const withHeaders of [false, true]) {
    const pending = app.dispatchConnectedProxyRequest({}), id = c.messages.at(-1).request_id;
    if (withHeaders) send("empty", { event_type: "response_headers", request_id: id, status: 0, headers: [] });
    send("empty", { event_type: "stream_close", request_id: id });
    assert.deepEqual(await pending, { status: 200, headers: {}, bodyText: "" });
  }
});

test("connected error replies preserve normalized status message and body metadata", async (t) => {
  const f = protocolFixture(t), c = f.client("errors");
  for (const [message, status, expected] of [[" failure ", 429, "failure"], [" ", 0, "Connected browser client failed."]]) {
    const pending = app.dispatchConnectedProxyRequest({}), id = c.messages.at(-1).request_id;
    const rejected = assert.rejects(pending, { message: expected, status: status || 500, code: "gemini_canvas_connected_client_error", bodyText: message.trim() || null });
    send("errors", { event_type: "error", request_id: id, message, status });
    await rejected;
    assert.equal(f.timers.size, 0);
  }
});

test("connected cleanup rejects only its own pending requests and remains idempotent", async (t) => {
  const f = protocolFixture(t), a = f.client("a"), b = f.client("b");
  const one = app.dispatchConnectedProxyRequest({}), two = app.dispatchConnectedProxyRequest({});
  const rejected = [one, two].map((p) => assert.rejects(p, { status: 503, code: "gemini_canvas_connected_client_disconnected" }));
  a.entry.authenticated = false;
  const other = app.dispatchConnectedProxyRequest({}), id = b.messages[0].request_id;
  app.cleanupConnectedClient("a"); app.cleanupConnectedClient("a");
  await Promise.all(rejected);
  assert.equal(f.timers.size, 1);
  assert.deepEqual(app.listConnectedClients(), [b.entry]);
  send("b", { event_type: "stream_close", request_id: id });
  await other;
});

test("connected request timeout preserves defaults minimum delay and late-reply isolation", async (t) => {
  const f = protocolFixture(t), c = f.client("timed");
  for (const [configured, expected] of [[undefined, 120000], ["1", 10000], ["45000", 45000]]) {
    if (configured === undefined) delete process.env.GEMINI_CANVAS_CONNECTED_REQUEST_TIMEOUT_MS;
    else process.env.GEMINI_CANVAS_CONNECTED_REQUEST_TIMEOUT_MS = configured;
    const pending = app.dispatchConnectedProxyRequest({}), id = c.messages.at(-1).request_id;
    const rejected = assert.rejects(pending, { status: 504, code: "gemini_canvas_connected_client_timeout" });
    assert.equal([...f.timers.values()][0].ms, expected);
    f.expire(); await rejected;
    send("timed", { event_type: "stream_close", request_id: id });
    assert.equal(f.timers.size, 0);
  }
});

test("connected synchronous send failure clears the pending deadline and preserves errors", async (t) => {
  const f = protocolFixture(t), c = f.client("broken"), error = new Error("synthetic send failure");
  c.entry.ws.send = () => { throw error; };
  await assert.rejects(app.dispatchConnectedProxyRequest({}), (actual) => actual === error);
  assert.equal(f.timers.size, 0);
  c.entry.ws.send = () => assert.fail("circular request must fail before send");
  const circular = {}; circular.self = circular;
  await assert.rejects(app.dispatchConnectedProxyRequest(circular), TypeError);
  assert.equal(f.timers.size, 0);
});

test("connected response attribution rejects foreign and unauthenticated senders", async (t) => {
  const f = protocolFixture(t), owner = f.client("owner"); f.client("unsigned", false); f.client("foreign", true);
  const pending = app.dispatchConnectedProxyRequest({}), id = owner.messages[0].request_id;
  const observed = pending.then((value) => ({ value }), (error) => ({ error }));
  const forged = [
    { event_type: "response_headers", status: 418, headers: { "X-Foreign": "untrusted" } },
    { event_type: "chunk", data: "foreign" }, { event_type: "stream_close" },
    { event_type: "error", message: "foreign failure" },
  ];
  for (const sender of ["unsigned", "foreign"]) {
    for (const message of forged) send(sender, { ...message, request_id: id });
  }
  owner.entry.authenticated = false;
  for (const message of forged) send("owner", { ...message, request_id: id });
  assert.equal(f.timers.size, 1, "Only the authenticated owning connection may settle the request");
  owner.entry.authenticated = true;
  send("owner", { event_type: "chunk", request_id: id, data: "owned response" });
  send("owner", { event_type: "stream_close", request_id: id });
  assert.deepEqual(await observed, { value: { status: 200, headers: {}, bodyText: "owned response" } });
  assert.equal(f.timers.size, 0);
});

function bootstrapPage(options = {}) {
  const scripts = [], configs = [], sandbox = vm.createContext({ window: {}, Error });
  if (!options.missing) sandbox.window.__neuroGeminiCanvasConnectedClient = { connect: async (config) => {
    configs.push(plain(config)); if (options.connectError) throw options.connectError;
    return { connected: true, synthetic: true };
  } };
  return { scripts, configs, page: {
    addScriptTag: async ({ content }) => { scripts.push(content); if (options.addError) throw options.addError; },
    evaluate: async (callback, input) => { sandbox.input = input; return await vm.runInContext(`(${callback.toString()})(input)`, sandbox); },
  } };
}

test("connected loopback bootstrap preserves default and configured connection fields and source reuse", async (t) => {
  protocolFixture(t); const f = bootstrapPage();
  assert.deepEqual(plain(await app.ensureLoopbackConnectedClient({ page: f.page })), { connected: true, synthetic: true });
  assert.deepEqual(f.configs[0], { endpoint: "wss://127.0.0.1:42322/ws", apiKey: "", clientLabel: `gemini-canvas-loopback-${process.pid}` });
  process.env.GEMINI_CANVAS_BROWSER_HOST = " fixture.invalid ";
  process.env.GEMINI_CANVAS_BROWSER_POOL_TLS_PORT = "44444";
  process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN = "synthetic-key";
  await app.ensureLoopbackConnectedClient({ page: f.page });
  assert.equal(f.configs[1].endpoint, "wss://fixture.invalid:44444/ws");
  assert.equal(f.configs[1].apiKey, "synthetic-key");
  assert.equal(f.scripts[0], f.scripts[1]);
  assert.match(f.scripts[0], /window\.__neuroGeminiCanvasConnectedClient = new ConnectedClient/);
});

test("connected loopback bootstrap propagates injection missing-client and connect failures", async (t) => {
  protocolFixture(t);
  const error = new Error("synthetic bootstrap failure"), missing = bootstrapPage({ missing: true });
  await assert.rejects(app.ensureLoopbackConnectedClient({ page: missing.page }), /bootstrap did not install/);
  for (const options of [{ addError: error }, { connectError: error }]) {
    const f = bootstrapPage(options);
    await assert.rejects(app.ensureLoopbackConnectedClient({ page: f.page }), (actual) => actual === error);
  }
});

test("connected browser bootstrap executes the real client through authenticate fetch response and disconnect", async (t) => {
  const f = protocolFixture(t), requests = [], endpoints = [], outgoing = [];
  process.env.GEMINI_CANVAS_BROWSER_CLIENT_API_KEY = "synthetic-key";
  class Socket {
    static OPEN = 1;
    constructor(endpoint) {
      endpoints.push(endpoint); this.readyState = 0; this.listeners = new Map();
      app.connectedClients.set("vm-client", { connectionId: "vm-client", authenticated: false, ws: {
        send: (data) => { outgoing.push(JSON.parse(data)); queueMicrotask(() => this.emit("message", { data })); },
        close: () => this.close(),
      } });
      queueMicrotask(() => { this.readyState = 1; this.emit("open"); });
    }
    addEventListener(type, callback) { const callbacks = this.listeners.get(type) ?? []; callbacks.push(callback); this.listeners.set(type, callbacks); }
    emit(type, payload) { for (const callback of this.listeners.get(type) ?? []) callback(payload); }
    send(data) { app.handleConnectedClientMessage("vm-client", data); }
    close() { this.readyState = 3; app.cleanupConnectedClient("vm-client"); this.emit("close"); }
  }
  const sandbox = vm.createContext({ window: {}, WebSocket: Socket, AbortController, Error, console: { info() {} }, fetch: async (url, init) => {
    requests.push({ url, init });
    if (url.includes("failure")) throw Object.assign(new Error("synthetic abort"), { name: "AbortError" });
    return { status: 201, headers: new Map([["x-synthetic", "yes"]]), text: async () => "synthetic response" };
  } });
  const page = { addScriptTag: async ({ content }) => vm.runInContext(content, sandbox), evaluate: async (callback, input) => {
    sandbox.input = input; return await vm.runInContext(`(${callback.toString()})(input)`, sandbox);
  } };
  const connected = await app.ensureLoopbackConnectedClient({ page });
  assert.equal(connected.connected, true);
  const client = sandbox.window.__neuroGeminiCanvasConnectedClient;
  assert.deepEqual(plain(await app.ensureLoopbackConnectedClient({ page })), { connected: true, reused: true });
  assert.equal(sandbox.window.__neuroGeminiCanvasConnectedClient, client);
  assert.deepEqual(endpoints, ["wss://127.0.0.1:42322/ws"]);
  const result = await app.dispatchConnectedProxyRequest({ path: "/v1beta/synthetic", headers: { Host: "drop", Authorization: "synthetic" }, body: "payload" });
  assert.deepEqual(result, { status: 201, headers: { "x-synthetic": "yes" }, bodyText: "synthetic response" });
  assert.equal(requests[0].url, "https://generativelanguage.googleapis.com/v1beta/synthetic");
  assert.equal(requests[0].init.method, "POST");
  assert.deepEqual(plain(requests[0].init.headers), { Authorization: "synthetic" });
  assert.equal(requests[0].init.body, "payload");
  await assert.rejects(app.dispatchConnectedProxyRequest({ url: "https://fixture.invalid/failure" }), { status: 499, code: "gemini_canvas_connected_client_error", bodyText: "synthetic abort" });
  assert.equal(outgoing[0].authorized, true);
  assert.equal(client.pending.size, 0);
  assert.equal(f.timers.size, 0);
  client.disconnect();
  assert.equal(client.socket, null);
  assert.equal(app.connectedClients.size, 0);
});
