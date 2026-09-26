import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { assertFetchReleased, fetchBaseUrl, fetchProgramUrl, fetchTargetUrl, fetchCalls, fetchOperationHarness } from "./gemini-canvas-browser-pool.fetch-operation-fixtures.mjs";

const app = await importTestableScript();
const args = { googleFetchMode: "canvas_page_music_no_key" };

for (const [name, request, expected] of [
  ["explicit JSON", { jsonBody: { prompt: "json" }, bodyText: '{"prompt":"body"}' }, { prompt: "json" }],
  ["explicit null", { jsonBody: null, bodyText: '{"prompt":"body"}' }, null],
  ["body JSON", { bodyText: '{"prompt":"body"}' }, { prompt: "body" }],
  ["invalid body", { bodyText: "invalid" }, null],
  ["absent body", {}, null],
]) {
  test(`Fetch page music ${name} preserves payload priority, handle inputs and capture identity`, async (t) => {
    const h = fetchOperationHarness(t, app, { programUrl: fetchProgramUrl });
    const result = await h.run({ ...args, timeoutMs: 7100 }, request);
    const call = fetchCalls(h, "page-music")[0];
    assert.equal(call[1], h.attached);
    assert.deepEqual(structuredClone(call.slice(2)), [{ url: fetchTargetUrl, jsonBody: expected }, 7100]);
    assert.equal(fetchCalls(h, "program").length, 1);
    assert.ok(h.calls.findIndex(([stage]) => stage === "program") < h.calls.findIndex(([stage]) => stage === "page-music"));
    assert.deepEqual(fetchCalls(h, "share"), []);
    assert.deepEqual(fetchCalls(h, "evaluate"), []);
    assert.equal(result.operation, "fetch");
    for (const [key, value] of Object.entries(h.result)) assert.equal(result[key], value);
    assert.equal(result.networkEvents, h.state.events);
    assert.equal(result.rpcCaptures, h.state.rpcCaptures);
    const handle = fetchCalls(h, "handle")[0];
    assert.equal(handle[1], fetchBaseUrl);
    assert.equal(handle[3], h.attached.url());
    assert.equal(handle[4], h.state);
    assertFetchReleased(assert, h);
  });
}

for (const status of [0, 429]) {
  test(`Fetch page music error status ${status} preserves provider diagnostics and releases capture`, async (t) => {
    const h = fetchOperationHarness(t, app, { previewResult: { ok: false, status, errorMessage: "fixture failure", errorName: "FixtureError", bodyText: " provider body " } });
    await assert.rejects(h.run(args), {
      status: status || 599, code: "gemini_canvas_page_music_no_key_fetch_failed", bodyText: " provider body ",
      message: "Gemini Canvas page-context no-key music websocket failed: FixtureError fixture failure",
    });
    assertFetchReleased(assert, h);
  });
}

test("Fetch page music forwards the bounded audio error code and status", async (t) => {
  const h = fetchOperationHarness(t, app, {
    previewResult: {
      ok: false,
      status: 413,
      code: "gemini_canvas_browser_body_too_large",
      errorMessage: "audio body exceeded limit",
      errorName: "PageMusicNoKeyBodyTooLarge",
    },
  });
  await assert.rejects(h.run(args), {
    status: 413,
    code: "gemini_canvas_browser_body_too_large",
  });
  assertFetchReleased(assert, h);
});

test("Fetch page music HTTP failure without transport error remains a response", async (t) => {
  const h = fetchOperationHarness(t, app, { previewResult: { ok: false, status: 503, bodyText: "unavailable" } });
  assert.equal((await h.run(args)).status, 503);
  assertFetchReleased(assert, h);
});

for (const failureAt of ["page-music", "handle", "program"]) {
  test(`Fetch page music ${failureAt} rejection retains error identity and releases resources`, async (t) => {
    const h = fetchOperationHarness(t, app, { failureAt, programUrl: fetchProgramUrl });
    await assert.rejects(h.run(args), (error) => error === h.failure);
    assertFetchReleased(assert, h);
  });
}

test("Fetch page music awaits pending websocket result before restoring the shared page", async (t) => {
  let finish, entered;
  const pending = new Promise((resolve) => { finish = resolve; });
  const started = new Promise((resolve) => { entered = resolve; });
  const h = fetchOperationHarness(t, app, { onPageMusic: async () => { entered(); return await pending; } });
  const run = h.run(args);
  try {
    await started;
    assert.equal(h.entry.page, h.attached);
    assert.equal(h.stops, 0);
    assert.equal(h.attached.listenerCount("request"), 2);
  } finally { finish(h.result); }
  assert.equal((await run).bodyText, "fixture");
  assertFetchReleased(assert, h);
});
