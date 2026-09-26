import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { callsAt, dispatchBaseUrl, dispatchProgramUrl, dispatcherHarness } from "./gemini-canvas-browser-pool.dispatcher-fixtures.mjs";

const app = await importTestableScript();
const stages = (h) => h.calls.filter(([name]) => name !== "log").map(([name]) => name);
function released(h) {
  assert.equal(h.entry.busy, false);
  assert.equal(h.entry.lastUsedAt, h.now);
}

test("Dispatcher native missing runtime key rejects without allocating a context", async () => {
  for (const args of [undefined, null, {}, { runtimeStateObjectKey: " " }]) {
    const before = app.contexts.size;
    assert.deepEqual(await app.invokeGeminiCanvas(args), {
      ok: false, error: { code: "gemini_canvas_invalid_request", message: "runtimeStateObjectKey is required.", status: 400 },
    });
    assert.equal(app.contexts.size, before);
  }
});

test("Dispatcher busy entry keeps the foreign lease and timestamp untouched", async () => {
  const h = dispatcherHarness(app, { entry: { busy: true } }), result = await h.run();
  assert.equal(result.error.status, 429);
  assert.equal(result.error.code, "gemini_canvas_context_busy");
  assert.equal(h.entry.busy, true);
  assert.equal(h.entry.lastUsedAt, 17);
  assert.deepEqual(stages(h), ["scope", "context"]);
});

test("Dispatcher awaits the operation while rejecting overlapping invocation without releasing its lease", async () => {
  let resolveResult, entered;
  const pending = new Promise((resolve) => { resolveResult = resolve; });
  const started = new Promise((resolve) => { entered = resolve; });
  const h = dispatcherHarness(app, { onOperation: async () => { entered(); return await pending; } });
  const first = h.run();
  try {
    await started;
    assert.equal(h.entry.busy, true);
    h.advance(50);
    assert.equal((await h.run()).error.status, 429);
    assert.equal(h.entry.busy, true);
    assert.equal(h.entry.lastUsedAt, 1000);
  } finally { resolveResult(h.operationResult); }
  const result = await first;
  assert.equal(result.result, h.operationResult);
  released(h);
  assert.equal(callsAt(h, "text").length, 1);
});

for (const mode of ["missingPage", "closedPage"]) {
  test(`Dispatcher recreates ${mode} and tolerates focus failure`, async () => {
    const h = dispatcherHarness(app, { [mode]: true, failureAt: "front" });
    assert.equal((await h.run()).ok, true);
    assert.equal(h.entry.page, h.replacement);
    assert.deepEqual(stages(h).slice(0, 5), ["scope", "context", "new-page", "front", "auth"]);
    released(h);
  });
}

for (const [operation, fetchRequest, expected] of [
  ["bootstrap_program", {}, "bootstrap"], ["text", {}, "fetch"],
  ["text", null, "text"], ["tts", null, "tts"], ["debug", null, "debug"],
  ["image", null, "media"], ["music", null, "media"], ["video", null, "media"],
]) {
  test(`Dispatcher ${operation} with fetch ${Boolean(fetchRequest)} preserves routing and argument identities`, async () => {
    const h = dispatcherHarness(app), result = await h.run({ operation, fetchRequest, requireAppPage: false, enforceProgramOwner: operation === "bootstrap_program" });
    assert.equal(result.ok, true);
    assert.equal(result.result, h.operationResult);
    const routed = h.calls.filter(([name]) => ["bootstrap", "fetch", "text", "tts", "debug", "media"].includes(name));
    assert.equal(routed.length, 1);
    assert.equal(routed[0][0], expected);
    assert.equal(routed[0][1], h.entry);
    assert.equal(routed[0][2], h.scopedArgs);
    assert.deepEqual(callsAt(h, "app-page"), []);
    assert.equal(callsAt(h, "resolve").length, expected === "bootstrap" ? 0 : 1);
    released(h);
  });
}

test("Dispatcher account scope precedes context acquisition and is forwarded to every consumer", async () => {
  const h = dispatcherHarness(app);
  await h.run({ authUser: "2", baseUrl: "https://gemini.google.com", locale: "en", browserExecutablePath: "fixture-browser", browserCdpUrl: "http://fixture.invalid", timeoutMs: 3210 });
  assert.deepEqual(structuredClone(callsAt(h, "context")[0][1]), {
    runtimeStateObjectKey: "fixture-key", baseUrl: "https://gemini.google.com/u/2/", locale: "en",
    browserExecutablePath: "fixture-browser", browserCdpUrl: "http://fixture.invalid", timeoutMs: 3210,
  });
  assert.equal(callsAt(h, "auth")[0][2], "https://gemini.google.com/u/2/");
  assert.equal(callsAt(h, "resolve")[0][1], "https://gemini.google.com/u/2/");
  assert.equal(callsAt(h, "text")[0][2], h.scopedArgs);
});

test("Dispatcher enforced owner without concrete handle returns 400 and releases lease before navigation", async () => {
  const h = dispatcherHarness(app), result = await h.run({ enforceProgramOwner: true });
  assert.equal(result.error.code, "gemini_canvas_program_handle_required");
  assert.equal(result.error.status, 400);
  assert.deepEqual(callsAt(h, "app-page"), []);
  assert.deepEqual(callsAt(h, "text"), []);
  released(h);
});

test("Dispatcher program page receives its handle and explicit timeout before operation", async () => {
  const h = dispatcherHarness(app, { programUrl: dispatchProgramUrl });
  await h.run({ timeoutMs: "2345" });
  assert.deepEqual(callsAt(h, "program-page")[0], ["program-page", h.entry, dispatchBaseUrl, dispatchProgramUrl, 2345]);
  assert.deepEqual(callsAt(h, "app-page"), []);
  assert.ok(stages(h).indexOf("program-page") < stages(h).indexOf("text"));
});

test("Dispatcher app page receives normalized default URL, timeout, cookie and attached navigation hint", async () => {
  const h = dispatcherHarness(app, { entry: { attachedCdp: true } });
  await h.run({ baseUrl: " ", timeoutMs: 0, cookieHeader: "fixture=value" });
  const call = callsAt(h, "app-page")[0];
  assert.equal(call[1], h.entry);
  assert.deepEqual(structuredClone(call.slice(2)), ["https://gemini.google.com", 5000, { cookieHeader: "fixture=value", skipInitialNavigationWhenAppSurfaceReady: true }]);
});

test("Dispatcher attached CDP fetch defers outer page ensure even with a concrete program handle", async () => {
  const h = dispatcherHarness(app, { entry: { attachedCdp: true }, programUrl: dispatchProgramUrl });
  await h.run({ fetchRequest: {}, enforceProgramOwner: true });
  assert.deepEqual(callsAt(h, "app-page"), []);
  assert.deepEqual(callsAt(h, "program-page"), []);
  assert.equal(callsAt(h, "fetch").length, 1);
});

for (const failureAt of ["context", "new-page", "auth", "resolve", "app-page", "program-page", "text"]) {
  test(`Dispatcher ${failureAt} failure preserves error envelope and local lease ownership`, async () => {
    const failure = Object.assign(new Error("fixture failure"), { status: 502, code: "fixture_error", bodyText: "fixture body" });
    const h = dispatcherHarness(app, { failureAt, failure, missingPage: failureAt === "new-page", programUrl: failureAt === "program-page" ? dispatchProgramUrl : null });
    assert.deepEqual(structuredClone(await h.run()), { ok: false, error: { code: "fixture_error", message: "fixture failure", status: 502, body: "fixture body" } });
    assert.equal(h.entry.busy, false);
    assert.equal(h.entry.lastUsedAt, failureAt === "context" ? 17 : h.now);
    assert.deepEqual(callsAt(h, "close"), []);
  });
}

for (const status of [401, "403"]) {
  test(`Dispatcher authentication ${status} closes context before finally releases the lease`, async () => {
    const h = dispatcherHarness(app, { failureAt: "text", failure: Object.assign(new Error("fixture auth"), { status }) });
    assert.equal((await h.run()).error.status, Number(status));
    assert.deepEqual(callsAt(h, "close"), [["close", "fixture-key"]]);
    assert.ok(h.calls.findIndex(([name]) => name === "close") < h.calls.findIndex(([name, message]) => name === "log" && message === "invokeGeminiCanvas finally release"));
    released(h);
  });
}

test("Dispatcher non-Error failure retains the default worker envelope", async () => {
  const h = dispatcherHarness(app, { failureAt: "text", failure: "fixture rejection" });
  assert.deepEqual(structuredClone(await h.run()), { ok: false, error: { code: "gemini_canvas_browser_worker_failed", message: "fixture rejection", status: 500, body: null } });
  released(h);
});
