// No late setup/UI work or cleanup-time capture failure may escape the invocation owner.
import test from "node:test";
import assert from "node:assert/strict";
import { submitNativeGeneration } from "../udio-browser/native-submit.mjs";
function fixture(late) {
  let handler, capture, release;
  const calls = [];
  const deferred = new Promise(resolve => { release = resolve; });
  const input = { first() { return this; }, waitFor: async () => calls.push("visible"), fill: async () => calls.push("fill") };
  const page = { locator: () => input, evaluate: async () => late ? deferred : false,
    route: async (_matches, fn) => { handler = fn; }, unroute: async () => {} };
  const response = { url: () => "https://www.udio.com/api/generate-proxy", request: () => ({ method: () => "POST" }),
    status: () => 200, headers: () => ({ "content-type": "application/json" }), text: async () => "{}" };
  const button = { click: async () => {
    await handler({ request: () => ({ method: () => "POST", postData: () => '{"gen_params":{"model_type":"fixture-native"}}' }),
      continue: async () => {}, abort: async () => {} }); capture.onResponse(response);
  } };
  const run = () => submitNativeGeneration(page, { baseUrl: "https://www.udio.com", timeoutMs: late ? 15 : 1000,
    requestBody: { gen_params: { prompt: "fixture", model_type: "fixture-native" } } }, async () => button,
  (_page, options) => { capture = options; return { ready: Promise.resolve(), stop: async () => {
    if (late) release(false);
    else options.onError(Object.assign(new Error("private-cleanup"), { code: "gateway_program_handle_capture_limit_exceeded" }));
  } }; });
  return { run, calls };
}
test("cleanup-time capture failure cannot return native success", async () => {
  const f = fixture(false);
  await assert.rejects(f.run(), reason => reason.code === "udio_browser_metadata_limit");
});
test("late Service Worker preflight cannot fill borrowed page after timeout", async () => {
  const f = fixture(true);
  await assert.rejects(f.run(), reason => reason.code === "udio_browser_wait_timeout");
  await new Promise(resolve => setImmediate(resolve));
  assert.ok(!f.calls.includes("fill"), JSON.stringify(f.calls));
});
