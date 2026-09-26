import assert from "node:assert/strict";
import test from "node:test";
import { responseFixture } from "./gemini-canvas-program-handle.response-fixtures.mjs";
import { readPlaywrightResponseBody, readPlaywrightResponseText } from "../gemini-canvas-browser-pool-body.mjs";

test("native binary response preserves invalid UTF-8 and reads the protocol body only once", async (t) => {
  const f = await responseFixture(t, { readText: false, bodyBytes: 4, binaryBodyBytes: 16 });
  const bytes = Buffer.from([0, 255, 128, 240, 1]);
  f.session.bodies.set("one", { body: bytes.toString("base64"), base64Encoded: true });
  f.request(); f.response("one", 200, { "Content-Type": "image/png" }); f.bytes(bytes.length); f.finish();
  const response = f.rows[0].response;
  assert.deepEqual(await readPlaywrightResponseBody(response, 16), bytes);
  assert.deepEqual(await response.body(), bytes);
  assert.equal(f.bodyCalls().length, 1);
  assert.deepEqual(f.failures, []);
});

test("decoded text admission rejects compressed or understated bodies before native retrieval", async (t) => {
  const f = await responseFixture(t, { readText: false, bodyBytes: 4, binaryBodyBytes: 16 });
  f.request(); f.response("one", 200, {
    "Content-Type": "application/json", "Content-Encoding": "gzip", "Content-Length": "1",
  });
  const body = readPlaywrightResponseText(f.rows[0].response, 4);
  f.bytes(5); f.finish();
  assert.equal(await body, "");
  assert.equal(f.bodyCalls().length, 0);
  assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
});

test("unknown declared length remains readable using observed Chromium decoded bytes", async (t) => {
  const f = await responseFixture(t, { readText: false });
  f.request(); f.response(); f.bytes(5); f.finish();
  assert.equal(await readPlaywrightResponseText(f.rows[0].response, 16), "small");
  assert.equal(f.bodyCalls().length, 1);
  assert.deepEqual(f.failures, []);
});

test("native binary body over budget is rejected without a whole-body protocol request", async (t) => {
  const f = await responseFixture(t, { readText: false, bodyBytes: 4, binaryBodyBytes: 8 });
  f.request(); f.response("one", 200, { "Content-Type": "audio/wav", "Content-Length": "1" });
  const body = readPlaywrightResponseBody(f.rows[0].response, 8);
  f.bytes(9); f.finish();
  assert.deepEqual(await body, Buffer.alloc(0));
  assert.equal(f.bodyCalls().length, 0);
  assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
});

test("stopping capture settles an awaiting binary reader without requesting a body", async (t) => {
  const f = await responseFixture(t, { readText: false });
  f.request(); f.response();
  const body = f.rows[0].response.body();
  await f.capture.stop();
  assert.deepEqual(await body, Buffer.alloc(0));
  assert.equal(f.bodyCalls().length, 0);
});

test("metadata-only large video does not abort capture of the next program response", async (t) => {
  const f = await responseFixture(t, { readText: false, bodyBytes: 4, binaryBodyBytes: 8 });
  f.request(); f.response("one", 200, { "Content-Type": "video/mp4" }); f.bytes(1024); f.finish();
  assert.deepEqual(f.failures, []);
  f.session.bodies.set("two", { body: "ok", base64Encoded: false });
  f.request("two"); f.response("two"); f.bytes(2, "two"); f.finish("two");
  assert.equal(await f.rows[1].response.text(), "ok");
  assert.equal(f.bodyCalls().length, 1);
});

test("a lazy body reader invoked after completion and stop cannot issue a late native read", async (t) => {
  const f = await responseFixture(t, { readText: false });
  f.request(); f.response(); f.bytes(5); f.finish();
  await f.capture.stop();
  assert.deepEqual(await f.rows[0].response.body(), Buffer.alloc(0));
  assert.equal(f.bodyCalls().length, 0);
});
