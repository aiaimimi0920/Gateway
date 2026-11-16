import test from "node:test";
import assert from "node:assert/strict";
import { bindNativeBody, boundNativeModel, createNativeModelGate } from "../suno-browser/native-model.mjs";
import { createSunoResponses } from "../suno-browser/responses.mjs";
const prompt = "synthetic fixture", model = "chirp-auk";
const native = () => ({ mv: "page-selection", token: "synthetic-token", prompt, generation_type: "TEXT",
  make_instrumental: false, metadata: { create_mode: "simple", user_tier: "synthetic-tier" } });
test("Suno binds only mv and preserves all admitted native fields", () => {
  const body = native();
  assert.deepEqual(JSON.parse(bindNativeBody(JSON.stringify(body), model, prompt)), { ...body, mv: model });
  assert.equal(boundNativeModel({ model }), model);
  for (const value of [{}, { model: "public-alias" }, { model: " chirp-auk " }, { model, lyrics: "other task" }]) {
    assert.throws(() => boundNativeModel(value), { code: "suno_browser_request_mismatch" });
  }
});
test("Suno known basic prompt wire variants bind; other task shapes are rejected", () => {
  const body = { ...native(), prompt: "", gpt_description_prompt: prompt };
  assert.equal(JSON.parse(bindNativeBody(JSON.stringify(body), model, prompt)).mv, model);
  for (const change of [
    { prompt: "other" }, { prompt, gpt_description_prompt: prompt }, { mv: null },
    { generation_type: "EXTEND" }, { make_instrumental: true }, { title: "other task" },
    { continue_clip_id: "other-song" }, { override_fields: ["prompt"] }, { unknown: true },
    { metadata: { create_mode: "custom" } }, { metadata: { create_mode: "simple", is_max_mode: true } },
    { metadata: { create_mode: "simple", control_sliders: { style_weight: 0.6 } } },
    { metadata: { create_mode: "simple", unknown: true } },
  ]) assert.throws(() => bindNativeBody(JSON.stringify({ ...native(), ...change }), model, prompt), { code: "suno_browser_request_mismatch" });
  for (const text of ["not-json", "[]", "null", JSON.stringify({ ...native(), padding: "x".repeat(128 * 1024) })]) {
    assert.throws(() => bindNativeBody(text, model, prompt), { code: "suno_browser_request_mismatch" });
  }
});
function fixture(overrides = {}) {
  let handler, matcher;
  const actions = [], failures = [];
  const page = { route: async (match, run) => { matcher = match; handler = run; },
    unroute: async (match, run) => { assert.equal(match, matcher); assert.equal(run, handler); actions.push("unroute"); } };
  const gate = createNativeModelGate(page, "https://suno.fixture.invalid", model, prompt, reason => failures.push(reason));
  const route = { request: () => ({ method: () => "POST", postData: () => JSON.stringify(native()) }),
    continue: async options => actions.push(JSON.parse(options.postData)), abort: async () => actions.push("abort"),
    fallback: async () => actions.push("fallback"), ...overrides };
  return { gate, actions, failures, send: () => handler(route), matches: url => matcher(new URL(url)) };
}
test("Suno native gate matches one origin/path and leaves non-POST traffic alone", async () => {
  const f = fixture({ request: () => ({ method: () => "GET" }) }); await f.gate.ready;
  assert.equal(f.matches("https://suno.fixture.invalid/api/generate/v2-web/?x=1"), true);
  for (const url of ["https://other.invalid/api/generate/v2-web", "https://suno.fixture.invalid/api/generate/v2-web/extra"]) {
    assert.equal(f.matches(url), false);
  }
  await f.send(); await f.gate.stop(); assert.deepEqual(f.actions, ["fallback", "unroute"]);
});
test("Suno native POST before arm aborts and poisons later submission", async () => {
  const f = fixture(); await f.gate.ready; await f.send();
  assert.deepEqual(f.actions, ["abort"]); assert.equal(f.gate.admitted(), false);
  assert.throws(() => f.gate.arm(), { code: "suno_browser_ambiguous_generation" });
  await assert.rejects(f.gate.stop(), { code: "suno_browser_ambiguous_generation" });
});
test("Suno native gate continues one bound POST; a second POST is not sent", async () => {
  const f = fixture(); await f.gate.ready; f.gate.arm(); await f.send();
  assert.equal(f.actions[0].mv, model); assert.equal(f.gate.admitted(), true);
  await f.send(); assert.equal(f.actions[1], "abort"); assert.equal(f.gate.admitted(), false);
  await assert.rejects(f.gate.stop(), { code: "suno_browser_ambiguous_generation" });
});
test("Suno native continuation failure cannot become an admitted success", async () => {
  const f = fixture({ continue: async () => { throw new Error("private native failure"); } });
  await f.gate.ready; f.gate.arm(); await f.send();
  assert.equal(f.gate.admitted(), false); assert.equal(f.failures[0].code, "suno_browser_request_mismatch");
  assert.ok(!f.failures[0].message.includes("private")); await assert.rejects(f.gate.stop());
});
test("Suno cleanup waits for its active route and removes only its own handler", async () => {
  let finish;
  const pending = new Promise(resolve => { finish = resolve; });
  const f = fixture({ continue: () => pending }); await f.gate.ready; f.gate.arm();
  const send = f.send(); let ended = false;
  const stop = f.gate.stop().then(() => { ended = true; });
  await new Promise(resolve => setImmediate(resolve)); assert.equal(ended, false);
  finish(); await send; await stop; assert.deepEqual(f.actions, ["unroute"]);
});
test("Suno response without an admitted bound POST is rejected before body access", async () => {
  let options;
  const capture = createSunoResponses({}, "https://suno.fixture.invalid", (_, value) => {
    options = value; return { ready: Promise.resolve(), stop: async () => {} };
  }, () => false);
  await capture.ready;
  options.onResponse({ request: () => ({ method: () => "POST" }),
    url: () => "https://suno.fixture.invalid/api/generate/v2-web/", text: () => assert.fail("unbound body read") });
  await assert.rejects(capture.next("generate", 100), { code: "suno_browser_request_mismatch" });
  await capture.stop();
});
