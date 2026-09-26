import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { assertFetchReleased, fetchProgramUrl, fetchTargetUrl, fetchCalls, fetchOperationHarness } from "./gemini-canvas-browser-pool.fetch-operation-fixtures.mjs";
import { BROWSER_POOL_TEXT_BODY_LIMIT_BYTES } from "../gemini-canvas-browser-pool-body.mjs";

const app = await importTestableScript();

test("Fetch native validation rejects missing and blank URLs before entry access", async () => {
  for (const args of [{}, { fetchRequest: {} }, { fetchRequest: { url: " " } }]) {
    await assert.rejects(app.runFetchOperation(null, args), { status: 400, code: "gemini_canvas_invalid_fetch_request" });
  }
});

for (const fixture of [true, false]) {
  for (const binary of [false, true]) {
    test(`Fetch ${fixture ? "fixture bypass" : "browser callback"} preserves ${binary ? "large binary" : "UTF-8 text"} response and cleanup`, async (t) => {
      const body = binary ? Buffer.alloc(70001, 0xa5) : "fixture \u4e2d\u6587";
      const h = fetchOperationHarness(t, app, { fixture, body, status: 503, contentType: binary ? "application/octet-stream" : "text/plain" });
      const result = await h.run({ googleFetchMode: "", requireAppPage: false }, { method: "post", bodyText: "request", referrer: " https://fixture.invalid/source ", referrerPolicy: " no-referrer " });
      assert.equal(result.status, 503);
      assert.equal(result.ok, false);
      assert.equal(result.finalUrl, fetchTargetUrl);
      assert.equal(result.bodyText, binary ? null : body);
      assert.equal(result.bodyBase64, Buffer.from(body).toString("base64"));
      const request = fetchCalls(h, "fetch")[0][2];
      assert.equal(request.method, "POST");
      assert.equal(request.referrer, "https://fixture.invalid/source");
      assert.equal(request.referrerPolicy, "no-referrer");
      assert.equal(fixture ? request.body : new TextDecoder().decode(request.body), "request");
      assert.equal(fetchCalls(h, "evaluate").length, fixture ? 0 : 1);
      assert.equal(fetchCalls(h, "clear-timer").length, fixture ? 0 : 1);
      if (!fixture) assert.equal(request.credentials, "include");
      assertFetchReleased(assert, h);
    });
  }
}

test("Fetch browser callback cancels a streamed text body at the text limit", async (t) => {
  const h = fetchOperationHarness(t, app, {
    contentType: "text/plain",
    streamChunks: [
      Buffer.alloc(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES),
      Buffer.from("x"),
      Buffer.from("must not be read"),
    ],
  });
  await assert.rejects(
    h.run({ googleFetchMode: "", requireAppPage: false }),
    { status: 413, code: "gemini_canvas_browser_body_too_large" },
  );
  assert.equal(fetchCalls(h, "stream-read").length, 2);
  assert.equal(fetchCalls(h, "stream-cancel").length, 1);
  assert.equal(fetchCalls(h, "stream-release").length, 1);
  assert.equal(fetchCalls(h, "body").length, 0);
  assert.equal(fetchCalls(h, "clear-timer").length, 1);
  assertFetchReleased(assert, h);
});

test("Fetch browser callback ignores representation length for a bodyless response", async (t) => {
  const h = fetchOperationHarness(t, app, {
    status: 304,
    contentLength: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES + 1,
    noBody: true,
  });
  const result = await h.run({ googleFetchMode: "", requireAppPage: false }, { method: "HEAD" });
  assert.equal(result.status, 304);
  assert.equal(result.bodyText, "");
  assert.equal(result.bodyBase64, "");
  assert.equal(fetchCalls(h, "body").length, 0);
  assertFetchReleased(assert, h);
});

for (const fixture of [true, false]) {
  test(`Fetch ${fixture ? "fixture" : "browser"} fallback refuses unknown length before arrayBuffer`, async (t) => {
    const h = fetchOperationHarness(t, app, { fixture, omitContentLength: true });
    await assert.rejects(
      h.run({ googleFetchMode: "", requireAppPage: false }),
      { status: 502, code: "gemini_canvas_browser_body_size_unknown" },
    );
    assert.equal(fetchCalls(h, "body").length, 0);
    assertFetchReleased(assert, h);
  });
}

for (const fixture of [true, false]) {
  test(`Fetch ${fixture ? "fixture" : "browser"} body rejection preserves error and clears resources`, async (t) => {
    const h = fetchOperationHarness(t, app, { fixture, failureAt: "body" });
    await assert.rejects(h.run({ googleFetchMode: "", requireAppPage: false }), (error) => error === h.failure);
    assertFetchReleased(assert, h);
  });
}

for (const message of ["Execution context was destroyed", "frame was detached", "navigation interrupted"]) {
  test(`Fetch page no-key retries ${message} once and preserves auth/referrer request`, async (t) => {
    const h = fetchOperationHarness(t, app, { programUrl: fetchProgramUrl, evaluateFailures: [new Error(message)] });
    assert.equal((await h.run({ googleFetchMode: "canvas_page_no_key" })).status, 201);
    assert.deepEqual(fetchCalls(h, "wait"), [["wait", 1200]]);
    assert.equal(fetchCalls(h, "evaluate").length, 2);
    const input = fetchCalls(h, "evaluate")[0][1];
    assert.equal(input.headers["x-origin"], undefined);
    assert.equal(input.headers["x-goog-authuser"], "1");
    assert.equal(input.referrer, fetchProgramUrl);
    assertFetchReleased(assert, h);
  });
}

test("Fetch page no-key retry tolerates wait rejection but propagates second evaluation error", async (t) => {
  const failure = new Error("second evaluation failed");
  const h = fetchOperationHarness(t, app, { failureAt: "wait", evaluateFailures: [new Error("navigation"), failure] });
  await assert.rejects(h.run({ googleFetchMode: "canvas_page_no_key" }), (error) => error === failure);
  assert.equal(fetchCalls(h, "evaluate").length, 2);
  assertFetchReleased(assert, h);
});

test("Fetch connected fallback preserves full request and response after POST transport failure", async (t) => {
  const h = fetchOperationHarness(t, app, { connected: true, programUrl: fetchProgramUrl, evaluateFailures: [new Error("Failed to fetch")] });
  const result = await h.run({ googleFetchMode: "" }, { method: "POST", bodyText: "request", referrerPolicy: "origin" });
  const spec = fetchCalls(h, "proxy")[0][1];
  assert.equal(spec.method, "POST");
  assert.equal(spec.url, fetchTargetUrl);
  assert.equal(spec.body, "request");
  assert.equal(spec.referrer, fetchProgramUrl);
  assert.equal(spec.referrerPolicy, "origin");
  assert.equal(result.status, 202);
  assert.equal(result.contentType, "text/plain");
  assert.equal(result.bodyText, "proxy result");
  assert.equal(result.bodyBase64, Buffer.from("proxy result").toString("base64"));
  assertFetchReleased(assert, h);
});

test("Fetch nonretryable evaluation rejection preserves identity without connected fallback", async (t) => {
  const failure = new Error("unrelated evaluation failure");
  const h = fetchOperationHarness(t, app, { evaluateFailures: [failure] });
  await assert.rejects(h.run({ googleFetchMode: "" }), (error) => error === failure);
  assert.deepEqual(fetchCalls(h, "proxy"), []);
  assert.deepEqual(fetchCalls(h, "wait"), []);
  assertFetchReleased(assert, h);
});

test("Fetch missing proxy client preserves 503 envelope and capture cleanup", async (t) => {
  const h = fetchOperationHarness(t, app);
  await assert.rejects(h.run({ googleFetchMode: "canvas_proxy" }), { status: 503, code: "gemini_canvas_program_ws_client_unavailable", bodyText: null });
  assertFetchReleased(assert, h);
});

test("Fetch page music branch retains original page without an attached page", async (t) => {
  const h = fetchOperationHarness(t, app, { attached: false });
  assert.equal((await h.run({ googleFetchMode: "canvas_page_music_no_key" }, { jsonBody: { prompt: "music" } })).bodyText, "fixture");
  assert.equal(fetchCalls(h, "page-music")[0][1], h.original);
  assertFetchReleased(assert, h);
});
