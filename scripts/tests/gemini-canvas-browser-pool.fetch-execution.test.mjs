import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { assertFetchReleased, fetchCalls, fetchOperationHarness } from "./gemini-canvas-browser-pool.fetch-operation-fixtures.mjs";

const app = await importTestableScript();

test("Fetch execution browser-download GET returns before websocket bootstrap and restores page", async (t) => {
  const h = fetchOperationHarness(t, app);
  const result = await h.run({ googleFetchMode: "canvas_proxy", timeoutMs: 6500 }, { url: "https://lh3.googleusercontent.com/generated.png" });
  assert.deepEqual(fetchCalls(h, "download")[0], ["download", h.entry, "https://lh3.googleusercontent.com/generated.png", 6500]);
  assert.deepEqual(fetchCalls(h, "connect"), []);
  assert.deepEqual(fetchCalls(h, "proxy"), []);
  assert.equal(result.bodyText, "fixture");
  assert.equal(result.networkEvents, h.state.events);
  assertFetchReleased(assert, h);
});

test("Fetch execution browser-download failure preserves identity and releases capture", async (t) => {
  const h = fetchOperationHarness(t, app, { failureAt: "download" });
  await assert.rejects(h.run({ googleFetchMode: "canvas_proxy" }, { url: "https://lh3.googleusercontent.com/generated.png" }), (error) => error === h.failure);
  assertFetchReleased(assert, h);
});

for (const failureAt of ["connect", "proxy"]) {
  test(`Fetch execution ${failureAt} failure retains websocket-unavailable diagnostics`, async (t) => {
    const h = fetchOperationHarness(t, app, { failureAt, connected: failureAt === "proxy" });
    await assert.rejects(h.run({ googleFetchMode: "canvas_proxy" }), (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.code, "gemini_canvas_program_ws_bootstrap_failed");
      assert.equal(error.bodyText, h.failure.message);
      assert.equal(error.message, `Gemini Canvas program websocket path is unavailable: ${h.failure.message}`);
      return true;
    });
    assert.deepEqual(fetchCalls(h, "evaluate"), []);
    assertFetchReleased(assert, h);
  });
}

test("Fetch execution preserves connected-client body-limit status and code", async (t) => {
  const failure = Object.assign(new Error("connected response exceeded the body budget"), {
    status: 413,
    code: "gemini_canvas_browser_body_too_large",
  });
  const h = fetchOperationHarness(t, app, { failureAt: "proxy", failure, connected: true });
  await assert.rejects(h.run({ googleFetchMode: "canvas_proxy" }), (error) => error === failure);
  assert.deepEqual(fetchCalls(h, "evaluate"), []);
  assertFetchReleased(assert, h);
});
