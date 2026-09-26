import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { assertFetchReleased, fetchBaseUrl, fetchProgramUrl, fetchTargetUrl, fetchCalls, fetchOperationHarness } from "./gemini-canvas-browser-pool.fetch-operation-fixtures.mjs";

const app = await importTestableScript();

test("Fetch preview forwards normalized request and handle inputs, preserving capture identities", async (t) => {
  const h = fetchOperationHarness(t, app), result = await h.run({ shareId: " share " }, { method: " post ", headers: { "content-type": "application/json" }, jsonBody: { value: 1 } });
  const request = { url: fetchTargetUrl, method: "POST", headers: { "content-type": "application/json" }, bodyText: '{"value":1}' };
  assert.deepEqual(structuredClone(fetchCalls(h, "preview")[0].slice(2)), [fetchBaseUrl, "share", 5000, request]);
  const call = fetchCalls(h, "preview-fetch")[0];
  assert.equal(call[1], h.frame);
  assert.deepEqual(structuredClone(call.slice(2)), [request, 5000]);
  assert.equal(result.operation, "fetch");
  for (const [key, value] of Object.entries(h.result)) assert.equal(result[key], value);
  assert.equal(result.previewFrameUrl, h.frame.url());
  assert.equal(result.previewProbe, h.stampedFrames);
  assert.equal(result.networkEvents, h.state.events);
  assert.equal(result.rpcCaptures, h.state.rpcCaptures);
  const handle = fetchCalls(h, "handle")[0];
  assert.equal(handle[1], fetchBaseUrl);
  assert.equal(handle[3], h.attached.url());
  assert.equal(handle[4], h.state);
  assertFetchReleased(assert, h);
});

for (const noPreviewFrame of [false, true]) {
  test(`Fetch preview concrete program selects ${noPreviewFrame ? "page fallback" : "child frame"} without attached-page lookup`, async (t) => {
    const h = fetchOperationHarness(t, app, { programUrl: fetchProgramUrl, noPreviewFrame });
    const result = await h.run({ timeoutMs: 6200 });
    assert.deepEqual(fetchCalls(h, "attached"), []);
    assert.deepEqual(fetchCalls(h, "preview"), []);
    assert.deepEqual(fetchCalls(h, "program")[0], ["program", h.entry, fetchBaseUrl, fetchProgramUrl, 6200]);
    assert.equal(fetchCalls(h, "open")[0][1], h.original);
    assert.equal(fetchCalls(h, "preview-fetch")[0][1], noPreviewFrame ? h.original : h.frame);
    assert.equal(result.previewFrameUrl, noPreviewFrame ? h.original.url() : h.frame.url());
    assertFetchReleased(assert, h);
  });
}

test("Fetch preview fixture program uses frame preparation without navigating to program", async (t) => {
  const h = fetchOperationHarness(t, app, { fixture: true, programUrl: fetchProgramUrl });
  await h.run();
  assert.equal(fetchCalls(h, "preview").length, 1);
  assert.deepEqual(fetchCalls(h, "program"), []);
  assert.deepEqual(fetchCalls(h, "attached"), []);
  assertFetchReleased(assert, h);
});

for (const [name, probe, bodyText, bodyBase64, status] of [
  ["full empty body before preview", { status: 206, bodyText: "", bodyPreview: "ignored" }, "", "", 206],
  ["trimmed preview text with original bytes", { bodyPreview: " preview " }, "preview", Buffer.from(" preview ").toString("base64"), 599],
]) {
  test(`Fetch preview stamped probe preserves ${name} and bypasses executor`, async (t) => {
    const h = fetchOperationHarness(t, app, { stampedFrames: [{ stamped: false, probeFetchResult: { ok: false } }, { stamped: true, probeFetchResult: { ok: true, url: " ", contentType: " text/plain ", ...probe } }] });
    const result = await h.run();
    assert.equal(result.status, status);
    assert.equal(result.finalUrl, fetchTargetUrl);
    assert.equal(result.contentType, "text/plain");
    assert.equal(result.bodyText, bodyText);
    assert.equal(result.bodyBase64, bodyBase64);
    assert.equal(result.errorMessage, null);
    assert.deepEqual(fetchCalls(h, "preview-fetch"), []);
    assertFetchReleased(assert, h);
  });
}

for (const [name, request, expected] of [
  ["json priority", { jsonBody: { prompt: "json" }, bodyText: '{"prompt":"body"}' }, { prompt: "json" }],
  ["body JSON", { bodyText: '{"prompt":"body"}' }, { prompt: "body" }],
  ["invalid body", { bodyText: "invalid" }, null],
  ["absent body", {}, null],
]) {
  test(`Fetch preview music ${name} ignores probe and preserves websocket request`, async (t) => {
    const h = fetchOperationHarness(t, app, { stampedFrames: [{ stamped: true, probeFetchResult: { ok: true } }] });
    await h.run({ googleFetchMode: "canvas_preview_music_no_key" }, request);
    assert.equal(fetchCalls(h, "preview")[0].at(-1), null);
    assert.deepEqual(structuredClone(fetchCalls(h, "preview-music")[0].slice(2)), [{ url: fetchTargetUrl, jsonBody: expected }, 5000]);
    assert.deepEqual(fetchCalls(h, "preview-fetch"), []);
    assertFetchReleased(assert, h);
  });
}

for (const music of [false, true]) {
  test(`Fetch preview ${music ? "music" : "ordinary"} error retains code, fallback status and bounded diagnostics`, async (t) => {
    const h = fetchOperationHarness(t, app, { events: 55, rpcs: 31, previewResult: { ok: false, status: 0, errorMessage: "fixture failure", errorName: "FixtureError", bodyText: " " } });
    await assert.rejects(h.run({ googleFetchMode: music ? "canvas_preview_music_no_key" : "canvas_preview_no_key" }), (error) => {
      assert.equal(error.status, 599);
      assert.equal(error.code, music ? "gemini_canvas_preview_music_no_key_fetch_failed" : "gemini_canvas_preview_no_key_fetch_failed");
      assert.match(error.message, /FixtureError fixture failure/);
      const body = JSON.parse(error.bodyText);
      assert.equal(body.previewFrameUrl, h.frame.url());
      assert.deepEqual(body.networkEvents, Array.from({ length: 40 }, (_, i) => ({ i: i + 15 })));
      assert.deepEqual(body.rpcCaptures, Array.from({ length: 20 }, (_, i) => ({ i: i + 11 })));
      return true;
    });
    assertFetchReleased(assert, h);
  });
}

test("Fetch preview music forwards the bounded audio error code and status", async (t) => {
  const h = fetchOperationHarness(t, app, {
    previewResult: {
      ok: false,
      status: 413,
      code: "gemini_canvas_browser_body_too_large",
      errorMessage: "audio body exceeded limit",
      errorName: "PreviewMusicNoKeyBodyTooLarge",
    },
  });
  await assert.rejects(
    h.run({ googleFetchMode: "canvas_preview_music_no_key" }),
    (error) => {
      assert.equal(error.status, 413);
      assert.equal(error.code, "gemini_canvas_browser_body_too_large");
      assert.equal(JSON.parse(error.bodyText).probeResult.code, "gemini_canvas_browser_body_too_large");
      return true;
    },
  );
  assertFetchReleased(assert, h);
});

test("Fetch preview error preserves nonblank provider body verbatim", async (t) => {
  const h = fetchOperationHarness(t, app, { previewResult: { ok: false, status: 429, errorMessage: "quota", bodyText: " provider body " } });
  await assert.rejects(h.run(), { status: 429, bodyText: " provider body " });
  assertFetchReleased(assert, h);
});

test("Fetch preview HTTP failure without transport error remains a response", async (t) => {
  const h = fetchOperationHarness(t, app, { previewResult: { ok: false, status: 503, bodyText: "upstream unavailable" } });
  assert.equal((await h.run()).status, 503);
  assertFetchReleased(assert, h);
});

for (const failureAt of ["preview", "preview-fetch", "preview-music", "program", "open", "frames", "handle"]) {
  test(`Fetch preview ${failureAt} rejection restores page and stops acquired capture`, async (t) => {
    const h = fetchOperationHarness(t, app, { failureAt, programUrl: ["program", "open", "frames"].includes(failureAt) ? fetchProgramUrl : null });
    await assert.rejects(h.run({ googleFetchMode: failureAt === "preview-music" ? "canvas_preview_music_no_key" : "canvas_preview_no_key" }), (error) => error === h.failure);
    assertFetchReleased(assert, h);
  });
}

test("Fetch preview awaits pending result before restoring page and stopping capture", async (t) => {
  let finish, entered;
  const pending = new Promise((resolve) => { finish = resolve; });
  const started = new Promise((resolve) => { entered = resolve; });
  const h = fetchOperationHarness(t, app, { onPreview: async () => { entered(); return await pending; } });
  const run = h.run();
  try {
    await started;
    assert.equal(h.entry.page, h.attached);
    assert.equal(h.stops, 0);
    assert.equal(h.attached.listenerCount("request"), 2);
  } finally { finish(h.result); }
  assert.equal((await run).bodyText, "fixture");
  assertFetchReleased(assert, h);
});
