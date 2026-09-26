import assert from "node:assert/strict";
import test from "node:test";
import { createProgramCaptureBudget } from "../gemini-canvas-program-handle-capture-budget.mjs";
import { networkFixture, requestFixture, responseFixture, deferred, turn, streamUrl, pairText } from "./gemini-canvas-program-handle.network-fixtures.mjs";
import { executionFixture } from "./gemini-canvas-program-handle.execution-fixtures.mjs";

const limitError = { code: "gateway_program_handle_capture_limit_exceeded" };
const websocket = () => ({ url: () => "wss://generativelanguage.googleapis.com/ws" });

test("capture budgets reject UTF-8 bytes and cumulative input before evidence parsing", () => {
  const budget = createProgramCaptureBudget({ inputBytes: 4, sourceBytes: 6 });
  budget.inspectSource("éé");
  assert.throws(() => budget.inspectSource("ééé"), limitError);
  budget.inspectSource("ok");
  assert.throws(() => budget.inspectSource("x"), limitError);
});

test("capture retention charges escaped JSON and cumulative evidence", () => {
  const budget = createProgramCaptureBudget({ recordBytes: 10, retainedBytes: 10 });
  assert.throws(() => budget.accept({ x: "\n\n" }), limitError);
  budget.accept({ x: "a" });
  assert.throws(() => budget.accept({}), limitError);
});

test("capture traversal rejects deeply nested and wide evidence without serialization", () => {
  const budget = createProgramCaptureBudget();
  let deep = {};
  for (let i = 0; i < 10; i++) deep = { child: deep };
  assert.throws(() => budget.accept(deep), limitError);
  assert.throws(() => budget.accept(Array(2048).fill("x")), limitError);
});

test("capture limits cannot be disabled or expanded by injected options", () => {
  for (const limits of [{ pendingReads: 0 }, { records: Infinity }, { inputBytes: 8 * 1024 * 1024 }, { unknown: 1 }]) {
    assert.throws(() => createProgramCaptureBudget(limits), TypeError);
  }
});

test("capture permits a completed read to release its admission once", () => {
  const budget = createProgramCaptureBudget({ pendingReads: 1 });
  const release = budget.beginRead();
  assert.throws(() => budget.beginRead(), limitError);
  release(); release();
  const next = budget.beginRead();
  assert.throws(() => budget.beginRead(), limitError);
  next();
});

test("capture record exhaustion detaches listeners and does not evict evidence", async (t) => {
  const f = await networkFixture(t, "text", { limits: { records: 2 } });
  await f.emit("websocket", websocket());
  await f.emit("websocket", websocket());
  const before = structuredClone(f.state);
  await f.emit("websocket", websocket());
  assert.throws(f.capture.assertHealthy, limitError);
  assert.deepEqual(f.state, before);
  for (const event of ["request", "response", "websocket"]) assert.equal(f.page.listenerCount(event), 0);
});

test("capture page adoption shares its accumulated evidence budget", async (t) => {
  const first = await networkFixture(t, "text", { limits: { records: 1 } });
  await first.emit("websocket", websocket());
  first.capture.stop();
  const second = await networkFixture(t, "text", { state: first.state });
  await second.emit("websocket", websocket());
  assert.throws(second.capture.assertHealthy, limitError);
  assert.equal(second.state.requests.length, 1);
  await assert.rejects(() => networkFixture(t, "text", { state: second.state }), limitError);
});

test("capture rejects an oversized request before publishing handles or credentials", async (t) => {
  const f = await networkFixture(t, "text", { limits: { inputBytes: 128 } });
  const before = structuredClone(f.state);
  await f.emit("request", requestFixture({ data: pairText + "x".repeat(128), headers: { Cookie: "secret=fixture" } }));
  assert.throws(f.capture.assertHealthy, (error) => error.code === limitError.code && !error.message.includes("secret"));
  assert.deepEqual(f.state, before);
});

test("capture rejects an oversized returned body without publishing truncated success", async (t) => {
  const f = await networkFixture(t, "text", { limits: { inputBytes: 128 } });
  const before = structuredClone(f.state);
  await f.emit("response", responseFixture({ text: async () => pairText + "x".repeat(128) }));
  assert.throws(f.capture.assertHealthy, limitError);
  assert.deepEqual(f.state, before);
});

test("capture bounds native body admission and shares pending reads across page adoption", async (t) => {
  const body = deferred();
  const first = await networkFixture(t, "text", { limits: { pendingReads: 1 } });
  const pending = first.emit("response", responseFixture({ text: () => body.promise }));
  first.capture.stop();
  const second = await networkFixture(t, "text", { state: first.state });
  let reads = 0;
  await second.emit("response", responseFixture({ text: async () => { reads++; return pairText; } }));
  assert.equal(reads, 0);
  assert.throws(second.capture.assertHealthy, limitError);
  body.resolve(pairText);
  await pending;
  assert.equal(second.state.responses.length, 0);
});

test("capture shares cookie/body admission and consumes late cookie failure", async (t) => {
  const cookies = deferred();
  const f = await networkFixture(t, "text", { limits: { pendingReads: 1 }, cookies: () => cookies.promise });
  await f.emit("request", requestFixture({ url: streamUrl }));
  let reads = 0;
  await f.emit("response", responseFixture({ text: async () => { reads++; return pairText; } }));
  assert.equal(reads, 0);
  assert.throws(f.capture.assertHealthy, limitError);
  cookies.reject(new Error("secret cookie failure"));
  await turn();
  assert.equal(f.state.cookieHeader, undefined);
});

test("stopped capture cannot publish a queued async failure into an adopted page", async (t) => {
  const first = await networkFixture(t);
  const pending = first.emit("response", { url() { throw new Error("old page failure"); } });
  first.capture.stop();
  const second = await networkFixture(t, "text", { state: first.state });
  await pending;
  assert.doesNotThrow(second.capture.assertHealthy);
  await second.emit("websocket", websocket());
  assert.equal(second.state.requests.length, 1);
});

test("capture processing failures redact input while stopping publication", async (t) => {
  const f = await networkFixture(t);
  await f.emit("request", { url: () => streamUrl, postData() { throw new Error("secret=fixture"); } });
  assert.throws(f.capture.assertHealthy, (error) => error.code === "gateway_program_handle_capture_failed"
    && !error.message.includes("secret"));
  assert.equal(f.state.requests.length, 0);
});

test("probe refuses success output and closes context after capture exhaustion", async (t) => {
  const f = await executionFixture(t);
  const original = f.runtime.collectSnapshot;
  f.runtime.collectSnapshot = async (page, label) => {
    const result = await original(page, label);
    for (let i = 0; i < 1025; i++) page.context().emit("websocket", websocket());
    return result;
  };
  await assert.rejects(f.run(), limitError);
  assert.equal(f.printed.length, 0);
  assert.equal(f.writes.length, 0);
  assert.ok(f.calls.some(([name]) => name === "context close"));
  for (const event of ["request", "response", "websocket"]) assert.equal(f.initial.listenerCount(event), 0);
});
