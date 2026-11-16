import test from "node:test";
import assert from "node:assert/strict";
import { submitNativeGeneration } from "../udio-browser/native-submit.mjs";
function fixture(mode = "success") {
  const calls = []; let captureOptions;
  const response = { url: () => "https://www.udio.com/api/generate-proxy",
    request: () => ({ method: () => "POST" }), status: () => 200,
    headers: () => ({ "content-type": mode === "html" ? "text/html" : "application/json" }),
    text: async () => {
      calls.push("text");
      if (mode === "native-error") captureOptions.onError(Object.assign(new Error("private diagnostic"), { code: "gateway_program_handle_capture_limit_exceeded" }));
      return mode === "native-error" ? "" : "{}";
    } };
  const input = { first() { return this; }, waitFor: async () => { calls.push("visible"); }, fill: async () => { calls.push("fill"); } };
  let routeHandler;
  const button = { click: async () => {
    calls.push("click");
    if (mode === "click-error") throw new Error("private click diagnostic");
    if (mode === "timeout") return;
    await routeHandler({ request: () => ({ method: () => "POST", postData: () => JSON.stringify({ gen_params: { model_type: "udio-fixture-native" } }) }),
      continue: async () => {}, abort: async () => {} });
    if (mode === "foreign") {
      captureOptions.onResponse({ ...response, url: () => "https://other.invalid/api/generate-proxy" });
      captureOptions.onResponse({ ...response, request: () => ({ method: () => "GET" }) });
    }
    captureOptions.onResponse(response);
    if (mode === "duplicate") captureOptions.onResponse(response);
  } };
  const page = { locator: () => input, evaluate: async () => false,
    route: async (_matches, handler) => { routeHandler = handler; },
    unroute: async (_matches, handler) => { assert.equal(handler, routeHandler); routeHandler = null; } };
  const create = (_page, options) => {
    captureOptions = options;
    return { ready: Promise.resolve().then(() => { calls.push("ready"); if (mode === "ready-error") throw new Error("private ready diagnostic"); }),
      stop: async () => { calls.push("stop"); } };
  };
  const run = () => submitNativeGeneration(page, { baseUrl: "https://www.udio.com", requestBody: { gen_params: { prompt: "fixture", model_type: "udio-fixture-native" } }, timeoutMs: mode === "timeout" ? 20 : 1000 }, async () => button, create);
  return { run, calls, get options() { return captureOptions; } };
}
test("Udio native prepares bounded page-only owner before the only click", async () => {
  const f = fixture(); const value = await f.run(); assert.equal(value.ok, true);
  assert.deepEqual(f.calls, ["ready", "visible", "fill", "click", "text", "stop"]);
  assert.equal(f.options.pageOnly, true); assert.equal(f.options.failOnBodyError, true);
  assert.equal(f.options.bodyBytes, 128 * 1024); assert.equal(f.options.binaryBodyBytes, 128 * 1024);
  assert.equal(f.options.requestFilter("https://www.udio.com/api/generate-proxy/?x=1"), true);
  for (const url of ["https://other.invalid/api/generate-proxy", "https://www.udio.com/api/generate-proxy/other", "invalid"]) assert.equal(f.options.requestFilter(url), false);
});
for (const [mode, code] of [["html", "udio_browser_content_type"], ["native-error", "udio_browser_metadata_limit"],
  ["duplicate", "udio_browser_ambiguous_generation"], ["timeout", "udio_browser_wait_timeout"],
  ["click-error", "udio_browser_capture_failed"], ["ready-error", "udio_browser_capture_failed"]]) {
  test(`Udio native ${mode} stops capture and does not expose private diagnostics`, async () => {
    const f = fixture(mode);
    await assert.rejects(f.run(), reason => {
      assert.equal(reason.code, code); assert.ok(!reason.message.includes("private")); return true;
    });
    assert.equal(f.calls.at(-1), "stop"); assert.ok(f.calls.filter(c => c === "click").length <= 1);
    if (mode === "html") assert.ok(!f.calls.includes("text"));
  });
}
test("Udio native ignores non-POST and foreign origin before selecting response", async () => {
  const f = fixture("foreign"); assert.equal((await f.run()).text, "{}");
  assert.equal(f.calls.filter(c => c === "text").length, 1);
});
