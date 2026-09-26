import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { serverCalls, serverHarness } from "./gemini-canvas-browser-pool.server-fixtures.mjs";

const app = await importTestableScript();

test("Server JSON response uses UTF-8 byte length and disables caching", () => {
  let status, headers, bytes;
  app.sendJson({ writeHead(code, value) { status = code; headers = value; }, end(value) { bytes = value; } }, 201, { text: "\u4e2d\u6587" });
  assert.equal(status, 201);
  assert.ok(Buffer.isBuffer(bytes));
  assert.equal(bytes.toString("utf8"), JSON.stringify({ text: "\u4e2d\u6587" }));
  assert.deepEqual(headers, {
    "content-type": "application/json; charset=utf-8",
    "content-length": String(bytes.length),
    "cache-control": "no-store",
  });
});

test("Server body reader decodes multibyte characters across chunks", async () => {
  const req = new EventEmitter(), payload = Buffer.from('{"text":"\u4e2d\u6587"}');
  const pending = app.readJsonBody(req);
  for (const byte of payload) req.emit("data", Buffer.from([byte]));
  req.emit("end");
  assert.deepEqual(await pending, { text: "\u4e2d\u6587" });
});

test("Server body reader accepts an empty body", async () => {
  const req = new EventEmitter(), pending = app.readJsonBody(req);
  req.emit("end");
  assert.deepEqual(await pending, {});
});

test("Server body reader rejects invalid JSON", async () => {
  const req = new EventEmitter(), pending = app.readJsonBody(req);
  req.emit("data", "{");
  req.emit("end");
  await assert.rejects(pending, SyntaxError);
});

test("Server body reader preserves request error identity", async () => {
  const req = new EventEmitter(), failure = new Error("fixture request failed"), pending = app.readJsonBody(req);
  req.emit("error", failure);
  await assert.rejects(pending, (error) => error === failure);
});

test("Server health shares HTTP/HTTPS handler and exposes only client summary", async (t) => {
  const h = serverHarness(t, app, { clients: [{ clientLabel: "fixture", connectedAt: "fixture-time", connectionId: "private-id", ws: {}, authenticated: true }] });
  await h.start();
  assert.equal(h.servers[0].handler, h.servers[1].handler);
  const expected = {
    ok: true, contexts: 1,
    endpoints: { httpBaseUrl: "http://127.0.0.1:42321", httpsBaseUrl: "https://127.0.0.1:42322", wsEndpoint: "ws://127.0.0.1:42321/ws", wssEndpoint: "wss://127.0.0.1:42322/ws" },
    connectedClients: [{ clientLabel: "fixture", connectedAt: "fixture-time" }],
  };
  for (const scheme of ["http", "https"]) {
    const response = await h.request("GET", "/health", "", scheme);
    assert.equal(response.status, 200);
    assert.deepEqual(response.body, expected);
  }
  assert.deepEqual(serverCalls(h, "invoke"), []);
});

for (const [method, url] of [["GET", "/health?ready=1"], ["POST", "/health"], ["GET", "/invoke"], ["GET", "/fetch"], ["POST", "/fetch/"], ["POST", "/invoke?x=1"], ["GET", "/unknown"]]) {
  test(`Server rejects unmatched route ${method} ${url}`, async (t) => {
    const h = serverHarness(t, app);
    await h.start();
    const response = await h.request(method, url);
    assert.equal(response.status, 404);
    assert.deepEqual(response.body, { ok: false, error: { code: "not_found", message: "Unknown browser pool endpoint.", status: 404 } });
    assert.deepEqual(serverCalls(h, "invoke"), []);
    assert.equal(h.decodedBodies.length, 0);
  });
}

test("Server invoke forwards parsed body identity and result", async (t) => {
  const h = serverHarness(t, app);
  await h.start();
  const response = await h.request("POST", "/invoke", '{"prompt":"fixture","options":{"timeout":10}}');
  assert.equal(serverCalls(h, "invoke")[0][1], h.decodedBodies[0]);
  assert.equal(response.status, 200);
  assert.deepEqual(response.body, h.result);
});

for (const nested of [true, false, null]) {
  test(`Server fetch preserves fields and selects fetch request for nested=${nested}`, async (t) => {
    const h = serverHarness(t, app);
    await h.start();
    const input = { marker: "fixture", metadata: { keep: true }, ...(nested === true ? { fetchRequest: { url: "https://fixture.test" } } : nested === null ? { fetchRequest: null } : {}) };
    const response = await h.request("POST", "/fetch", JSON.stringify(input));
    const body = h.decodedBodies[0], forwarded = serverCalls(h, "invoke")[0][1];
    assert.notEqual(forwarded, body);
    assert.equal(forwarded.marker, body.marker);
    assert.equal(forwarded.metadata, body.metadata);
    assert.equal(forwarded.fetchRequest, nested === true ? body.fetchRequest : body);
    assert.equal(response.status, 200);
    assert.deepEqual(response.body, h.result);
  });
}

for (const route of ["/invoke", "/fetch"]) {
  for (const status of ["429", undefined]) {
    test(`Server ${route} maps dispatcher error status ${status}`, async (t) => {
      const result = { ok: false, error: { code: "fixture_failure", ...(status ? { status } : {}) } };
      const h = serverHarness(t, app, { result });
      await h.start();
      const response = await h.request("POST", route, "{}");
      assert.equal(response.status, status ? 429 : 500);
      assert.deepEqual(response.body, result);
    });
  }
  for (const failure of ["parse", "request", "invoke"]) {
    test(`Server ${route} wraps ${failure} exception`, async (t) => {
      const h = serverHarness(t, app, { failureAt: failure === "invoke" ? "invoke" : undefined });
      await h.start();
      const response = await h.request("POST", route, failure === "parse" ? "{" : "{}", "http", failure === "request" ? h.failure : null);
      assert.equal(response.status, 500);
      assert.deepEqual(Object.keys(response.body).sort(), ["error", "ok"]);
      assert.equal(response.body.ok, false);
      assert.deepEqual(Object.keys(response.body.error).sort(), ["code", "message", "status"]);
      assert.equal(response.body.error.code, "gemini_canvas_browser_pool_server_error");
      assert.equal(response.body.error.status, 500);
      if (failure === "parse") assert.match(response.body.error.message, /JSON/);
      else assert.equal(response.body.error.message, h.failure.message);
      assert.equal(serverCalls(h, "invoke").length, failure === "invoke" ? 1 : 0);
    });
  }
}
