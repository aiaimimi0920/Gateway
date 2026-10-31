import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { canvasPreviewOrigin, selectCanvasHostFrame, createCanvasHostFetchOwner } from "../gemini-canvas-browser-pool-host-fetch.mjs";
import { executeCanvasProxyPreviewNoKeyFetch } from "../gemini-canvas-browser-pool-no-key-fetch.mjs";

const origin = "https://fixture.scf.usercontent.goog";
const href = `blob:${origin}/fixture-document`;
const request = { url: "https://generativelanguage.googleapis.com/v1beta/models/fixture:generateContent",
  method: "POST", bodyText: "synthetic", headers: { "x-fixture": "private-header" } };

function fixture(options = {}) {
  const calls = [], requests = [], logs = [];
  const sandbox = vm.createContext({ URL, AbortController, DOMException, TextDecoder, Uint8Array,
    setTimeout, clearTimeout, btoa, location: { origin, href },
    document: { querySelector: () => options.marker === false ? null : {},
      querySelectorAll: () => options.contract === false ? [] : options.scripts ?? [{ textContent:
        'const modelInformation = {"textModelName":"fixture"}; let googleLlmBaseApiUrls = [];' }] },
    fetch: async (...args) => { requests.push(args); return new Response('"synthetic"',
      { headers: { "content-type": "application/json" } }); } });
  vm.runInContext("window = globalThis", sandbox);
  if (options.native) vm.runInContext("fetch = Math.max", sandbox);
  const frame = { url: () => options.url ?? href, isDetached: () => options.detached ?? false,
    async frameElement() { return { isVisible: async () => options.visible !== false,
      async dispose() { calls.push("dispose"); if (options.disposeError) throw new Error("disposed"); } }; },
    async evaluate(callback, input) { sandbox.input = structuredClone(input);
      return await vm.runInContext(`(${callback.toString()})(input)`, sandbox); } };
  const page = { frames: () => [frame], url: () => "https://gemini.google.com/share/fixture",
    isClosed: () => false, async waitForTimeout() { options.onWait?.(); },
    getByRole: (role, { name }) => ({ first: () => ({
      isVisible: async () => options.guest === true && name.test("Sign in"),
      async click() { calls.push(["click", role]); },
    }) }) };
  const entry = { page };
  const owner = createCanvasHostFetchOwner({ async ensureSharePage() { calls.push("share"); },
    async ensureProgramPage() { calls.push("program"); }, buildProgramHandleState: () => ({}),
    log: (...args) => logs.push(args), ...options.owner });
  const run = (overrides = {}) => owner.runCanvasHostFetchOperation({ entry, args: { shareId: "fixture" }, ...request,
    requestedHeaders: request.headers, requestBodyText: request.bodyText, timeoutMs: 1000,
    capture: { state: { events: [], rpcCaptures: [] } }, ...overrides });
  return { frame, page, entry, sandbox, requests, calls, logs, run };
}

test("Canvas origin accepts only HTTPS Google preview origins, including blob documents", () => {
  assert.equal(canvasPreviewOrigin(origin + "/frame"), origin);
  assert.equal(canvasPreviewOrigin(href), origin);
  for (const url of ["blob:https://evil.invalid/frame", "https://fixture.scf.usercontent.goog.evil.invalid",
    "http://fixture.scf.usercontent.goog/frame", "https://scf.usercontent.goog/frame", "not a URL"]) {
    assert.equal(canvasPreviewOrigin(url), null);
  }
});

test("Canvas selection records document identity without generating and disposes its handle", async () => {
  const f = fixture();
  const selected = await selectCanvasHostFrame(f.page);
  assert.equal(selected.frame, f.frame);
  assert.equal(selected.identity.origin, origin);
  assert.equal(selected.identity.href, href);
  assert.ok(selected.identity.documentToken);
  assert.deepEqual(f.requests, []);
  assert.deepEqual(f.calls, ["dispose"]);
});

for (const options of [{ native: true }, { marker: false }, { detached: true }, { visible: false },
  { url: "https://fixture.scf.usercontent.goog.evil.invalid/frame" }]) {
  test(`Canvas selector rejects an unauthenticated or unavailable frame ${JSON.stringify(options)}`, async () => {
    const f = fixture(options);
    assert.equal(await selectCanvasHostFrame(f.page), null);
    assert.equal(f.requests.length, 0);
  });
}

test("Canvas host owner uses one current-frame wrapped fetch with safe completion logs", async () => {
  const f = fixture();
  const result = await f.run();
  assert.equal(result.status, 200);
  assert.equal(result.executionSurface, "canvas_preview_host");
  assert.equal(f.requests.length, 1);
  assert.deepEqual(f.logs, [["canvas host fetch completed", JSON.stringify({ status: 200,
    executionSurface: "canvas_preview_host", hostWrappedFetch: true })]]);
  assert.doesNotMatch(JSON.stringify(f.logs), /private-header|synthetic/);
});

for (const overrides of [
  { url: "https://generativelanguage.googleapis.com/v1beta/models/lyria-3-clip-preview:generateContent" },
  { url: request.url.replace(":generateContent", ":predictLongRunning") },
  { url: request.url.replace("https://", "https://fixture:private@") },
  { method: "GET" },
]) {
  test("unadvertised Canvas model/operation never falls through to native fetch", async () => {
    const f = fixture();
    await assert.rejects(f.run(overrides), error => error.code === "gemini_canvas_host_model_unavailable" && error.status === 503);
    assert.equal(f.requests.length, 0);
    assert.equal(f.logs.length, 0);
  });
}

for (const [field, model, operations] of [
  ["deprecatedTextModelNames", "gemini-2.5-flash", ["generateContent", "streamGenerateContent"]],
  ["deprecatedImageModelNames", "imagen-3.0-generate-002", ["predict", "predictLongRunning", "generateContent", "streamGenerateContent"]],
  ["deprecatedGenerateImageModelNames", "gemini-2.5-flash-image", ["generateContent", "streamGenerateContent"]],
  ["deprecatedImageTransformModelNames", "gemini-3-pro-image-preview-11-2025", ["generateContent", "streamGenerateContent"]],
]) {
  test(`host explicitly advertised ${field} retain only their permitted operations`, async () => {
    for (const operation of operations) {
      const f = fixture({ scripts: [{ textContent: `googleLlmBaseApiUrls; const models = ${JSON.stringify({ [field]: [model, "another-advertised-model"] })};` }] });
      const result = await f.run({ url: `https://generativelanguage.googleapis.com/v1beta/models/${model}:${operation}` });
      assert.equal(result.status, 200);
      assert.equal(f.requests.length, 1);
    }
    const f = fixture({ scripts: [{ textContent: `googleLlmBaseApiUrls; const models = ${JSON.stringify({ [field]: [model] })};` }] });
    await assert.rejects(f.run({ url: `https://generativelanguage.googleapis.com/v1beta/models/${model}:delete` }),
      error => error.code === "gemini_canvas_host_model_unavailable");
    assert.equal(f.requests.length, 0);
  });
}

test("ambiguous, malformed or unbounded legacy host model lists cannot dispatch", async () => {
  for (const textContent of [
    'googleLlmBaseApiUrls; const a = {"deprecatedGenerateImageModelNames":["legacy",null]};',
    'googleLlmBaseApiUrls; const a = {"deprecatedGenerateImageModelNames":["legacy","../escape"]};',
    `googleLlmBaseApiUrls; const a = ${JSON.stringify({ deprecatedGenerateImageModelNames: ["legacy", ...Array.from({ length: 64 }, (_, i) => `legacy-${i}`)] })};`,
    'googleLlmBaseApiUrls; const a = {"deprecatedGenerateImageModelNames":["legacy"]}; const b = {"deprecatedGenerateImageModelNames":["different"]};',
    'googleLlmBaseApiUrls; const a = {"deprecatedGenerateImageModelNames":["legacy",]};',
  ]) {
    const f = fixture({ scripts: [{ textContent }] });
    await assert.rejects(f.run({ url: "https://generativelanguage.googleapis.com/v1beta/models/legacy:generateContent" }),
      error => error.code === "gemini_canvas_host_model_unavailable");
    assert.equal(f.requests.length, 0);
  }
});

test("legacy aliases do not admit music, operation polling, downloads or unadvertised models", async () => {
  for (const overrides of [
    { url: "https://generativelanguage.googleapis.com/v1beta/models/lyria-3-clip-preview:generateContent" },
    { url: "https://generativelanguage.googleapis.com/v1beta/models/not-advertised:generateContent" },
    { url: "https://generativelanguage.googleapis.com/v1beta/operations/fixture", method: "GET" },
    { url: "https://generativelanguage.googleapis.com/v1beta/files/fixture:download", method: "GET" },
  ]) {
    const f = fixture({ scripts: [{ textContent: 'googleLlmBaseApiUrls; const a = {"deprecatedGenerateImageModelNames":["legacy"]};' }] });
    await assert.rejects(f.run(overrides), error => error.code === "gemini_canvas_host_model_unavailable");
    assert.equal(f.requests.length, 0);
  }
});

test("repeated identical alias metadata is stable across ordering and separate injected scripts", async () => {
  const f = fixture({ scripts: [
    { textContent: 'googleLlmBaseApiUrls; const a = {"deprecatedGenerateImageModelNames":["legacy","other"]};' },
    { textContent: 'googleLlmBaseApiUrls; const b = {"deprecatedGenerateImageModelNames":["other","legacy","legacy"]};' },
  ] });
  const result = await f.run({ url: "https://generativelanguage.googleapis.com/v1beta/models/legacy:generateContent" });
  assert.equal(result.status, 200);
  assert.equal(f.requests.length, 1);
});

test("missing Canvas model metadata fails closed before fetch", async () => {
  const f = fixture({ contract: false });
  await assert.rejects(f.run(), error => error.code === "gemini_canvas_host_model_unavailable");
  assert.equal(f.requests.length, 0);
});

for (const scripts of [
  Array.from({ length: 33 }, () => ({ textContent: "" })),
  [{ textContent: "x".repeat(512 * 1024 + 1) }],
  [{ textContent: 'googleLlmBaseApiUrls; const a = {"textModelName":"fixture","textModelName":"other"};' }],
]) {
  test("oversized or ambiguous host metadata cannot admit a model", async () => {
    const f = fixture({ scripts });
    await assert.rejects(f.run(), error => error.code === "gemini_canvas_host_model_unavailable");
    assert.equal(f.requests.length, 0);
  });
}

for (const mutation of ["location.origin = 'https://evil.invalid'", "location.href += '-new'",
  "document = { querySelector: () => ({}) }", "document.querySelector = () => null", "fetch = Math.max"]) {
  test(`Canvas atomic fetch guard rejects changed document state: ${mutation}`, async () => {
    const f = fixture({ owner: { async executeHostFetch(frame, input, timeout) {
      vm.runInContext(mutation, f.sandbox);
      return executeCanvasProxyPreviewNoKeyFetch(frame, input, timeout);
    } } });
    await assert.rejects(f.run(), error => error.code === "gemini_canvas_host_identity_changed" && error.status === 503);
    assert.equal(f.requests.length, 0);
    assert.equal(f.logs.length, 0);
  });
}

for (const guest of [false, true]) {
  test(`Canvas missing host fails closed without a readiness probe (guest=${guest})`, async t => {
    let now = 0;
    t.mock.method(Date, "now", () => now);
    const f = fixture({ guest, onWait: () => { now += 1001; }, owner: { selectHostFrame: async () => null } });
    await assert.rejects(f.run(), error => error.code === (guest
      ? "gemini_canvas_browser_session_required" : "gemini_canvas_host_identity_unavailable"));
    assert.equal(f.requests.length, 0);
  });
}

test("Canvas source-page replacement is rejected before fetch", async () => {
  const f = fixture({ owner: { async selectHostFrame(page) {
    const selected = await selectCanvasHostFrame(page);
    f.entry.page = {};
    return selected;
  } } });
  await assert.rejects(f.run(), error => error.code === "gemini_canvas_host_identity_unavailable");
  assert.equal(f.requests.length, 0);
});

test("Concurrent Canvas owners keep their own document and frame", async () => {
  const a = fixture(), b = fixture();
  await Promise.all([a.run(), b.run()]);
  assert.equal(a.requests.length, 1);
  assert.equal(b.requests.length, 1);
});

test("Canvas host errors do not log or expose upstream secrets and are not retried", async () => {
  let executions = 0;
  const f = fixture({ owner: { async executeHostFetch() {
    executions++;
    return { status: 599, errorMessage: "Cookie=private-header synthetic" };
  } } });
  await assert.rejects(f.run(), error => error.code === "gemini_canvas_host_fetch_failed"
    && !/private-header|synthetic/.test(error.message));
  assert.equal(executions, 1);
  assert.equal(f.logs.length, 0);
});

test("empty synthetic Canvas host 401 does not invalidate the selected browser account", async () => {
  let executions = 0;
  const f = fixture({ owner: { async executeHostFetch() {
    executions++;
    return { status: 401, ok: false, bodyText: "", bodyBase64: "", contentType: "application/json" };
  } } });
  await assert.rejects(f.run(), error => error.status === 502
    && error.code === "gemini_canvas_host_operation_rejected"
    && error.message.includes("host status 401")
    && !/refresh|sign.?in|invalid.*session/i.test(error.message));
  assert.equal(executions, 1);
  assert.equal(f.logs.length, 0);
});

test("explicit host authentication error bodies keep their original HTTP classification", async () => {
  const f = fixture({ owner: { async executeHostFetch() {
    return { status: 401, ok: false, bodyText: '{"error":{"status":"UNAUTHENTICATED"}}' };
  } } });
  const result = await f.run();
  assert.equal(result.status, 401);
});

test("Canvas element disposal during navigation does not mask a usable selection", async () => {
  const f = fixture({ disposeError: true });
  const selected = await selectCanvasHostFrame(f.page);
  assert.equal(selected.frame, f.frame);
});

for (const overrides of [
  { url: request.url + "?key=fixture" },
  { requestedHeaders: { "X-Goog-Api-Key": "fixture" } },
  { requestedHeaders: { Authorization: "Bearer fixture" } },
  { requestBodyText: JSON.stringify({ apiKey: "fixture" }) },
]) {
  test("Canvas host cannot substitute explicit API identity for its selected account", async () => {
    const f = fixture();
    await assert.rejects(f.run(overrides), error => error.code === "gemini_canvas_host_api_identity_forbidden");
    assert.equal(f.requests.length, 0);
    assert.equal(f.calls.length, 0);
  });
}
