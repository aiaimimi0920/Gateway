// Isolated Page/Route seam: no browser, credentials, platform or media downloads.
import test from "node:test";
import assert from "node:assert/strict";
import { submitNativeGeneration } from "../udio-browser/native-submit.mjs";

function fixture(actualModel, { expected = "udio-fixture-native", raw, duplicate = false, controlled = false } = {}) {
  let handler, capture, sent = 0, aborted = 0, clicks = 0, stopped = false, unroute = false;
  const actual = raw ?? JSON.stringify({ gen_params: { prompt: "fixture", model_type: actualModel } });
  const request = { method: () => "POST", url: () => "https://www.udio.com/api/generate-proxy",
    postData: () => actual };
  const value = { request: () => request, url: request.url, status: () => 200,
    headers: () => ({ "content-type": "application/json" }), text: async () => "{}" };
  const route = { request: () => request, abort: async () => { aborted++; },
    continue: async options => { assert.equal(options, undefined, "native body must never be rewritten"); sent++; capture.onResponse(value); },
    fallback: async () => { throw new Error("own POST must not fall through"); } };
  const input = { first() { return this; }, waitFor: async () => {}, fill: async () => {} };
  const page = { locator: () => input, evaluate: async () => controlled, route: async (_matches, fn) => { handler = fn; },
    unroute: async (_matches, fn) => { assert.equal(handler, fn); unroute = true; handler = null; } };
  const button = { click: async () => { clicks++; if (handler) {
    await handler(route); if (duplicate) await handler(route);
  } else { sent++; capture.onResponse(value); } } };
  const run = () => submitNativeGeneration(page, { baseUrl: "https://www.udio.com",
    requestBody: { gen_params: { prompt: "fixture", model_type: expected } }, timeoutMs: 1000 },
  async () => button, (_page, options) => { capture = options; return { ready: Promise.resolve(), stop: async () => { stopped = true; } }; });
  return { run, counts: () => ({ sent, aborted, clicks, stopped, unroute }) };
}

test("Udio native model gate admits exact wire identity without rewriting", async () => {
  const f = fixture("udio-fixture-native"); assert.equal((await f.run()).ok, true);
  assert.deepEqual(f.counts(), { sent: 1, aborted: 0, clicks: 1, stopped: true, unroute: true });
});
for (const [name, actual, raw] of [["different", "foreign-private-model"], ["missing", undefined],
  ["wrong-type", 42], ["bad-json", undefined, "private-body {"], ["oversized", undefined, "x".repeat(1024 * 1024 + 1)]]) {
  test(`Udio native model gate blocks ${name} before network`, async () => {
    const f = fixture(actual, { raw });
    await assert.rejects(f.run(), reason => reason.code === "udio_browser_model_mismatch" && !reason.message.includes("private"));
    assert.deepEqual(f.counts(), { sent: 0, aborted: 1, clicks: 1, stopped: true, unroute: true });
  });
}
test("Udio native model gate stops a second matching submission", async () => {
  const f = fixture("udio-fixture-native", { duplicate: true });
  await assert.rejects(f.run(), reason => reason.code === "udio_browser_ambiguous_generation");
  assert.equal(f.counts().sent, 1); assert.equal(f.counts().aborted, 1);
});
test("Udio native requires bound model before click or routing", async () => {
  const f = fixture("udio-fixture-native", { expected: " " });
  await assert.rejects(f.run(), reason => reason.code === "udio_browser_model_mismatch");
  assert.equal(f.counts().clicks, 0); assert.equal(f.counts().sent, 0);
});
test("Udio native rejects a known Service Worker controlled page before click", async () => {
  const f = fixture("udio-fixture-native", { controlled: true });
  await assert.rejects(f.run(), reason => reason.code === "udio_browser_model_mismatch");
  assert.equal(f.counts().clicks, 0); assert.equal(f.counts().sent, 0); assert.equal(f.counts().unroute, true);
});
test("Udio native rejects conflicting aliases even when canonical model matches", async () => {
  const f = fixture("udio-fixture-native", { raw: JSON.stringify({ gen_params: {
    model_type: "udio-fixture-native", modelType: "foreign-private-model" } }) });
  await assert.rejects(f.run(), reason => reason.code === "udio_browser_model_mismatch");
  assert.equal(f.counts().sent, 0); assert.equal(f.counts().aborted, 1);
});
