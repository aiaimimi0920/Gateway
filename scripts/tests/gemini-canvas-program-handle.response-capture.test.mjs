import assert from "node:assert/strict";
import test from "node:test";
import { responseFixture, deferred, turn } from "./gemini-canvas-program-handle.response-fixtures.mjs";

test("native response capture waits for completion and preserves response identity", async (t) => {
  const f = await responseFixture(t);
  f.request("first", "POST"); f.response("first"); f.bytes(5, "first");
  f.request("second", "GET"); f.response("second"); f.bytes(6, "second");
  f.session.bodies.set("second", { body: "second", base64Encoded: false });
  assert.equal(f.bodyCalls().length, 0);
  f.finish("second"); f.finish("first");
  assert.deepEqual(await Promise.all(f.rows.map((row) => row.promise)), ["small", "second"]);
  assert.deepEqual(f.rows.map(({ method, status, contentType }) => [method, status, contentType]),
    [["POST", 200, "text/plain"], ["GET", 200, "text/plain"]]);
  assert.deepEqual(f.bodyCalls().map(({ params }) => params.requestId), ["second", "first"]);
  assert.deepEqual(f.failures, []);
});

for (const headers of [{ "content-length": "32" }, { "content-encoding": "gzip" }]) {
  test("oversize response fails before native read and closes its listeners " + JSON.stringify(headers), async (t) => {
    const f = await responseFixture(t);
    f.request(); f.response("one", 200, headers); f.bytes(12); f.bytes(5); f.finish();
    await turn();
    assert.equal(f.bodyCalls().length, 0);
    assert.equal(f.failures.length, 1);
    assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
    assert.equal(f.session.listenerCount("Network.responseReceived"), 0);
    assert.equal(await f.rows[0].promise, "");
  });
}

for (const bytes of [null, 0]) {
  test("unproven zero-size payload cannot request a native body " + bytes, async (t) => {
    const f = await responseFixture(t);
    f.request(); f.response(); if (bytes !== null) f.bytes(bytes); f.finish();
    await turn();
    assert.equal(f.bodyCalls().length, 0);
    assert.equal(f.failures.length, 1);
    assert.equal(await f.rows[0].promise, "");
  });
}

for (const [method, status, headers] of [["HEAD", 200, {}], ["GET", 204, {}], ["GET", 304, {}], ["POST", 200, { "Content-Length": "0" }]]) {
  test(`known empty response needs no native read: ${method} ${status} ${JSON.stringify(headers)}`, async (t) => {
    const f = await responseFixture(t);
    f.request("one", method); f.response("one", status, headers); f.finish();
    assert.equal(await f.rows[0].promise, "");
    assert.equal(f.bodyCalls().length, 0);
    assert.deepEqual(f.failures, []);
  });
}

test("exact byte boundary and binary UTF-8 conversion retain original semantics", async (t) => {
  const f = await responseFixture(t, { bodyBytes: 5 });
  f.session.bodies.set("one", { body: Buffer.from("small").toString("base64"), base64Encoded: true });
  f.request(); f.response(); f.bytes(2); f.bytes(3); f.finish();
  assert.equal(await f.rows[0].promise, "small");
  assert.deepEqual(f.failures, []);
});

test("redirect reuses a protocol id without losing the original response waiter", async (t) => {
  const f = await responseFixture(t);
  f.request(); f.response("one", 302);
  f.session.emit("Network.requestWillBeSent", { requestId: "one", request: { url: "https://fixture.invalid/app/next", method: "GET" },
    redirectResponse: { url: "https://fixture.invalid/app/request", status: 302, headers: {} } });
  f.response(); f.bytes(5); f.finish();
  assert.equal(f.rows.length, 2);
  assert.equal(await f.rows[0].promise, "");
  assert.equal(await f.rows[1].promise, "small");
  assert.deepEqual(f.rows.map((row) => row.method), ["POST", "GET"]);
});

test("failed network responses preserve empty body behavior", async (t) => {
  const f = await responseFixture(t);
  f.request(); f.response();
  f.session.emit("Network.loadingFailed", { requestId: "one" });
  assert.equal(await f.rows[0].promise, "");
  assert.equal(f.bodyCalls().length, 0);
  assert.deepEqual(f.failures, []);
});

test("stop blocks late body publication without releasing the pending native read early", async (t) => {
  const native = deferred(), f = await responseFixture(t);
  f.session.bodies.set("one", () => native.promise);
  f.request(); f.response(); f.bytes(5); f.finish();
  await turn();
  assert.equal(f.bodyCalls().length, 1);
  let settled = false;
  void f.rows[0].promise.then(() => { settled = true; });
  await f.capture.stop();
  assert.equal(settled, false);
  native.resolve({ body: "small", base64Encoded: false });
  assert.equal(await f.rows[0].promise, "");
  assert.equal(f.session.detached, 1);
  assert.equal(f.session.listenerCount("Network.loadingFinished"), 0);
});

test("startup failure rolls back listeners and hides native diagnostic content", async (t) => {
  const f = await responseFixture(t, { deferReady: true, enable: async () => { throw new Error("secret fixture diagnostic"); } });
  await assert.rejects(f.capture.ready, (error) => !error.message.includes("secret"));
  await f.capture.stop();
  assert.equal(f.failures.length, 1);
  assert.equal(f.session.listenerCount("Network.responseReceived"), 0);
  assert.equal(f.session.detached, 1);
});

test("a selected response without request identity fails closed", async (t) => {
  const f = await responseFixture(t);
  f.response(); f.bytes(5); f.finish();
  await turn();
  assert.equal(f.failures.length, 1);
  assert.equal(f.bodyCalls().length, 0);
});

test("a rejected response consumer closes the owner without exposing diagnostics", async (t) => {
  const f = await responseFixture(t, { onResponse: async () => { throw new Error("secret diagnostic"); } });
  f.request(); f.response();
  await turn();
  assert.equal(f.failures.length, 1);
  assert.ok(!f.failures[0].message.includes("secret"));
  assert.equal(await f.rows[0].promise, "");
  assert.equal(f.session.listenerCount("Network.responseReceived"), 0);
});
