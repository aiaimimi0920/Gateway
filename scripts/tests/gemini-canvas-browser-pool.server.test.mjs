import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES } from "../gemini-canvas-browser-pool-body.mjs";
import { serverCalls, serverHarness } from "./gemini-canvas-browser-pool.server-fixtures.mjs";

const app = await importTestableScript();

test("Server startup retains default listeners and unreferenced eviction interval", async (t) => {
  const h = serverHarness(t, app);
  await h.start();
  assert.equal(JSON.stringify(serverCalls(h, "mkdir")), JSON.stringify([["mkdir", "fixture-storage", { recursive: true }]]));
  assert.equal(JSON.stringify(serverCalls(h, "websocket-server")), JSON.stringify([["websocket-server", {
    noServer: true,
    maxPayload: BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES,
  }]]));
  assert.deepEqual(serverCalls(h, "listen"), [["listen", "http", 42321, "127.0.0.1"], ["listen", "https", 42322, "127.0.0.1"]]);
  assert.deepEqual(serverCalls(h, "interval"), [["interval", 300000]]);
  assert.deepEqual(serverCalls(h, "unref"), [["unref"]]);
  assert.equal(h.intervals.length, 1);
  h.intervals[0]();
  await Promise.resolve();
  assert.deepEqual(serverCalls(h, "evict"), [["evict"]]);
});

test("Server startup parses configured host and numeric ports", async (t) => {
  const h = serverHarness(t, app, { env: { GEMINI_CANVAS_BROWSER_HOST: "  fixture.test  ", GEMINI_CANVAS_BROWSER_POOL_PORT: "43121", GEMINI_CANVAS_BROWSER_POOL_TLS_PORT: "43122" }, contexts: new Map() });
  await h.start();
  assert.deepEqual(serverCalls(h, "listen"), [["listen", "http", 43121, "fixture.test"], ["listen", "https", 43122, "fixture.test"]]);
  const response = await h.request("GET", "/health");
  assert.equal(response.body.contexts, 0);
  assert.deepEqual(response.body.endpoints, { httpBaseUrl: "http://fixture.test:43121", httpsBaseUrl: "https://fixture.test:43122", wsEndpoint: "ws://fixture.test:43121/ws", wssEndpoint: "wss://fixture.test:43122/ws" });
});

test("Server empty environment values retain defaults", async (t) => {
  const h = serverHarness(t, app, { env: { GEMINI_CANVAS_BROWSER_HOST: "  ", GEMINI_CANVAS_BROWSER_POOL_PORT: "", GEMINI_CANVAS_BROWSER_POOL_TLS_PORT: "" } });
  await h.start();
  assert.deepEqual(serverCalls(h, "listen"), [["listen", "http", 42321, "127.0.0.1"], ["listen", "https", 42322, "127.0.0.1"]]);
});

test("Server startup tolerates storage mkdir rejection", async (t) => {
  const h = serverHarness(t, app, { failureAt: "mkdir" });
  await h.start();
  assert.equal(serverCalls(h, "listen").length, 2);
});

test("Server idle eviction rejection is logged", async (t) => {
  const h = serverHarness(t, app, { failureAt: "evict" });
  await h.start();
  h.intervals[0]();
  await Promise.resolve();
  assert.deepEqual(serverCalls(h, "log").at(-1), ["log", "idle eviction failed", h.failure.message]);
});

for (const generated of [true, false]) {
  test(`Server TLS bundle preserves values and generated=${generated} log`, async (t) => {
    const h = serverHarness(t, app, { generated });
    await h.start();
    assert.deepEqual(serverCalls(h, "certificate"), [["certificate", "127.0.0.1"]]);
    const tls = serverCalls(h, "https-options")[0][1];
    assert.equal(tls.key, h.tlsBundle.key);
    assert.equal(tls.cert, h.tlsBundle.cert);
    assert.deepEqual(Object.keys(tls).sort(), ["cert", "key"]);
    assert.deepEqual(serverCalls(h, "log"), [["log", "listening on http://127.0.0.1:42321"], ["log", "listening on https://127.0.0.1:42322"], ["log", `${generated ? "generated" : "loaded"} TLS certificate`, h.tlsBundle.certPath, h.tlsBundle.keyPath]]);
  });
}

test("Server certificate failure retains rejection after HTTP listen", async (t) => {
  const h = serverHarness(t, app, { failureAt: "certificate" });
  await assert.rejects(h.start(), (error) => error === h.failure);
  assert.deepEqual(serverCalls(h, "listen"), [["listen", "http", 42321, "127.0.0.1"]]);
  assert.deepEqual(serverCalls(h, "create-https"), []);
  assert.equal(h.servers[0].listenerCount("upgrade"), 1);
});

for (const scheme of ["http", "https"]) {
  test(`Server ${scheme} upgrade accepts ws pathname and registers unauthenticated client`, async (t) => {
    const h = serverHarness(t, app);
    await h.start();
    const upgrade = h.upgrade(scheme, "/ws?fixture=1");
    assert.equal(upgrade.socket.destroyed, false);
    assert.deepEqual(serverCalls(h, "upgrade")[0], ["upgrade", upgrade.req, upgrade.socket, upgrade.head]);
    assert.equal(h.connectedClients.size, 1);
    const entry = h.connectedClients.get("fixture-1");
    assert.deepEqual(Object.keys(entry).sort(), ["authenticated", "clientLabel", "connectedAt", "connectionId", "scheme", "ws"]);
    assert.equal(entry.connectionId, "fixture-1");
    assert.equal(entry.ws, upgrade.ws);
    assert.equal(entry.authenticated, false);
    assert.equal(entry.clientLabel, null);
    assert.equal(entry.connectedAt, "2026-01-01T00:00:00.000Z");
    assert.equal(entry.scheme, scheme);
    const message = Buffer.from("fixture-message");
    upgrade.ws.emit("message", message);
    assert.deepEqual(serverCalls(h, "message"), [["message", "fixture-1", message]]);
    upgrade.ws.emit("close");
    assert.deepEqual(serverCalls(h, "cleanup"), [["cleanup", "fixture-1"]]);
    assert.equal(h.connectedClients.size, 0);
  });
}

test("Server websocket error forwards cleanup for its own connection", async (t) => {
  const h = serverHarness(t, app);
  await h.start();
  h.upgrade("http", "/ws", "fixture.test:42321");
  const second = h.upgrade("https", "/ws", "fixture.test:42322");
  second.ws.emit("error", new Error("fixture socket failed"));
  assert.deepEqual(serverCalls(h, "cleanup"), [["cleanup", "fixture-2"]]);
  assert.deepEqual([...h.connectedClients.keys()], ["fixture-1"]);
});

for (const [url, host] of [["/other", undefined], ["/ws/", undefined], [undefined, undefined], ["/ws", "["]]) {
  test(`Server upgrade destroys unsupported URL ${url} host ${host}`, async (t) => {
    const h = serverHarness(t, app);
    await h.start();
    assert.equal(h.upgrade("http", url, host).socket.destroyed, true);
    assert.deepEqual(serverCalls(h, "upgrade"), []);
    assert.equal(h.connectedClients.size, 0);
  });
}

for (const failureAt of ["upgrade", "next-id"]) {
  test(`Server upgrade destroys socket when ${failureAt} throws`, async (t) => {
    const h = serverHarness(t, app, { failureAt });
    await h.start();
    assert.equal(h.upgrade("https", "/ws").socket.destroyed, true);
    assert.equal(h.connectedClients.size, 0);
  });
}
