import { EventEmitter } from "node:events";
import vm from "node:vm";
import { BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES } from "../gemini-canvas-browser-pool-body.mjs";

export const serverCalls = (h, name) => h.calls.filter(([stage]) => stage === name);

export function serverHarness(t, app, options = {}) {
  const calls = [], servers = [], sockets = [], decodedBodies = [], intervals = [];
  const failure = new Error("fixture server dependency failure");
  const contexts = options.contexts ?? new Map([["fixture-context", {}]]);
  const connectedClients = new Map();
  let nextId = 0;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    if (stage === options.failureAt) throw failure;
  };
  const create = (scheme, handler) => {
    record("create-" + scheme, handler);
    const server = new EventEmitter();
    server.scheme = scheme;
    server.handler = handler;
    server.listen = (port, host, callback) => { record("listen", scheme, port, host); callback(); };
    servers.push(server);
    return server;
  };
  class FixtureWebSocketServer {
    constructor(config) { record("websocket-server", config); }
    handleUpgrade(req, socket, head, callback) {
      record("upgrade", req, socket, head);
      const ws = new EventEmitter();
      sockets.push(ws);
      callback(ws);
    }
  }
  class FixtureDate extends Date {
    constructor() { super("2026-01-01T00:00:00.000Z"); }
  }
  const tlsBundle = { key: "fixture key", cert: "fixture certificate", keyPath: "fixture.key", certPath: "fixture.crt", generated: options.generated ?? false };
  const result = options.result ?? { ok: true, result: { marker: "fixture invocation" } };
  const dependencies = {
    ...app, Error, Buffer, URL, Date: FixtureDate,
    process: { env: { ...options.env } },
    BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES,
    DEFAULT_HOST: "127.0.0.1", DEFAULT_PORT: 42321, DEFAULT_TLS_PORT: 42322,
    getStorageRoot: () => "fixture-storage",
    async mkdir(...args) { record("mkdir", ...args); },
    WebSocketServer: FixtureWebSocketServer,
    setInterval(callback, ms) {
      record("interval", ms);
      intervals.push(callback);
      return { unref() { record("unref"); } };
    },
    async evictIdleContexts() { record("evict"); },
    log(...args) { calls.push(["log", ...args]); },
    contexts, connectedClients,
    listConnectedClients() { record("clients"); return options.clients ?? [...connectedClients.values()]; },
    nextConnectedRequestId() { record("next-id"); return `fixture-${++nextId}`; },
    cleanupConnectedClient(...args) { record("cleanup", ...args); connectedClients.delete(args[0]); },
    handleConnectedClientMessage(...args) { record("message", ...args); },
    async readJsonBody(req) { const body = await app.readJsonBody(req); decodedBodies.push(body); return body; },
    async invokeGeminiCanvas(...args) { record("invoke", ...args); return result; },
    createServer(handler) { return create("http", handler); },
    createHttpsServer(tls, handler) { record("https-options", tls); return create("https", handler); },
    async loadOrCreateTlsCertificate(...args) {
      record("certificate", ...args);
      return options.certificatePromise ?? tlsBundle;
    },
  };
  const main = vm.runInNewContext(`(${app.main.toString()})`, dependencies, { timeout: 1000 });
  t.after(() => { for (const value of [...servers, ...sockets]) value.removeAllListeners(); connectedClients.clear(); });
  return {
    calls, servers, sockets, decodedBodies, intervals, contexts, connectedClients, failure, tlsBundle, result,
    start: () => main(),
    async request(method, url, raw = "", scheme = "http", requestError = null) {
      const req = new EventEmitter();
      req.method = method;
      req.url = url;
      let status, headers, bytes;
      const response = { writeHead(code, value) { status = code; headers = value; }, end(value) { bytes = value; } };
      const pending = servers.find((server) => server.scheme === scheme).handler(req, response);
      try {
        if (requestError) req.emit("error", requestError);
        else { if (raw) req.emit("data", Buffer.from(raw)); req.emit("end"); }
        await pending;
        return { status, headers, bytes, body: JSON.parse(bytes.toString("utf8")) };
      } finally { req.removeAllListeners(); }
    },
    upgrade(scheme, url, host) {
      const req = { url, headers: { host } }, head = Buffer.from("fixture head");
      const socket = { destroyed: false, destroy() { this.destroyed = true; } };
      servers.find((server) => server.scheme === scheme).emit("upgrade", req, socket, head);
      return { req, socket, head, ws: sockets.at(-1) };
    },
  };
}
