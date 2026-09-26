import assert from "node:assert/strict";
import test from "node:test";
import { NativeSession, deferred, responseFixture, turn } from "./gemini-canvas-program-handle.response-fixtures.mjs";

test("native body retrieval starts on consumer demand and runs at most once", async (t) => {
  const f = await responseFixture(t, { readText: false });
  f.request(); f.response(); f.bytes(5); f.finish();
  await turn();
  assert.equal(f.bodyCalls().length, 0);
  const text = f.rows[0].response.text();
  assert.equal(text, f.rows[0].response.text());
  assert.equal(await text, "small");
  assert.equal(f.bodyCalls().length, 1);
});

test("native read admission stops at eight and keeps outstanding reads charged until settlement", async (t) => {
  const pending = deferred();
  const f = await responseFixture(t);
  for (let index = 0; index < 9; index++) {
    const id = `read-${index}`;
    f.session.bodies.set(id, () => pending.promise);
    f.request(id); f.response(id); f.bytes(5, id); f.finish(id);
  }
  await turn();
  let settled = 0;
  for (const row of f.rows.slice(0, 8)) void row.promise.then(() => { settled++; });
  try {
    assert.equal(f.bodyCalls().length, 8);
    assert.equal(f.failures.length, 1);
    assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
    await f.capture.stop();
    assert.equal(settled, 0);
  } finally { pending.resolve({ body: "small", base64Encoded: false }); }
  assert.deepEqual(await Promise.all(f.rows.map((row) => row.promise)), Array(9).fill(""));
});

test("equal request ids in separate sessions retain separate methods and bodies", async (t) => {
  const sessions = [new NativeSession(), new NativeSession()];
  sessions[0].bodies.set("same", { body: "first", base64Encoded: false });
  sessions[1].bodies.set("same", { body: "other", base64Encoded: false });
  const f = await responseFixture(t, {
    createSessions: (_page, { configure }) => {
      const controllers = sessions.map(() => new AbortController());
      const ready = Promise.all(sessions.map((session, index) => configure(session, controllers[index].signal)));
      return { ready, async stop() { controllers.forEach((controller) => controller.abort()); await ready; } };
    },
  });
  for (const [index, session] of sessions.entries()) {
    session.emit("Network.requestWillBeSent", { requestId: "same", request: {
      url: "https://fixture.invalid/app/same", method: index === 0 ? "POST" : "GET",
    } });
    session.emit("Network.responseReceived", { requestId: "same", response: { url: "https://fixture.invalid/app/same", status: 200, headers: {} } });
    session.emit("Network.dataReceived", { requestId: "same", dataLength: 5 });
  }
  for (const session of sessions) session.emit("Network.loadingFinished", { requestId: "same" });
  assert.deepEqual(await Promise.all(f.rows.map((row) => row.promise)), ["first", "other"]);
  assert.deepEqual(f.rows.map((row) => row.method), ["POST", "GET"]);
  assert.deepEqual(f.failures, []);
});

test("UTF-8 expansion beyond the text budget fails without publishing partial content", async (t) => {
  const f = await responseFixture(t, { bodyBytes: 1 });
  f.session.bodies.set("one", { body: Buffer.from([255]).toString("base64"), base64Encoded: true });
  f.request(); f.response(); f.bytes(1); f.finish();
  assert.equal(await f.rows[0].promise, "");
  assert.equal(f.failures.length, 1);
  assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
});

test("a selected request without a protocol id fails before body admission", async (t) => {
  const f = await responseFixture(t);
  f.session.emit("Network.requestWillBeSent", { request: { url: "https://fixture.invalid/app/request", method: "GET" } });
  await turn();
  assert.equal(f.failures.length, 1);
  assert.equal(f.bodyCalls().length, 0);
});

test("nested protocol escaping is bounded before a body is requested", async (t) => {
  const f = await responseFixture(t, { bodyBytes: 4 * 1024 * 1024 });
  f.session.nestingDepth = 2;
  f.request(); f.response(); f.bytes(2 * 1024 * 1024); f.finish();
  assert.equal(await f.rows[0].promise, "");
  assert.equal(f.bodyCalls().length, 0);
  assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
});

test("concurrent native reads share a protocol envelope budget", async (t) => {
  const body = deferred(), f = await responseFixture(t, { bodyBytes: 4 * 1024 * 1024 });
  for (let index = 0; index < 3; index++) {
    const id = String(index);
    f.session.bodies.set(id, () => body.promise);
    f.request(id); f.response(id); f.bytes(4 * 1024 * 1024, id); f.finish(id);
  }
  try {
    await turn();
    assert.equal(f.bodyCalls().length, 2);
    assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
  } finally { body.resolve({ body: "ignored after failure", base64Encoded: false }); }
  assert.deepEqual(await Promise.all(f.rows.map((row) => row.promise)), ["", "", ""]);
});
