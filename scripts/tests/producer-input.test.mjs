import assert from "node:assert/strict";
import test from "node:test";
import * as api from "../producer-browser/input.mjs";

test("Producer input normalization preserves defaults, timeout floor and boolean fallbacks", () => {
  assert.equal(api.normalizeBaseUrl(" https://producer.invalid/// "), "https://producer.invalid");
  assert.throws(() => api.normalizeBaseUrl(" "), { code: "producer_browser_missing_base_url" });
  for (const value of [undefined, null, NaN, Infinity, -1, "60"]) assert.equal(api.normalizeTimeoutMs(value), 1200000);
  assert.equal(api.normalizeTimeoutMs(1), 30000);
  assert.equal(api.normalizeTimeoutMs(45000.9), 45000);
  assert.equal(api.parseBoolean(" YES ", false), true);
  assert.equal(api.parseBoolean(" off ", true), false);
  assert.equal(api.parseBoolean(true, false), false);
  assert.equal(api.parseBoolean("unknown", true), true);
});

test("Producer input validation preserves distinct missing-field errors", () => {
  assert.throws(() => api.validateInput(null), { code: "producer_browser_invalid_input", status: 400 });
  assert.throws(() => api.validateInput({}), { code: "producer_browser_missing_base_url" });
  assert.throws(() => api.validateInput({ baseUrl: "https://producer.invalid" }), { code: "producer_browser_missing_request_body" });
  assert.doesNotThrow(() => api.validateInput({ baseUrl: "https://producer.invalid", requestBody: {} }));
});

test("Producer input parsing preserves valid payloads and separates malformed JSON from field errors", () => {
  const payload = { baseUrl: "https://producer.invalid", authToken: "synthetic credential", requestBody: { prompt: "fixture" } };
  assert.deepEqual(api.parseInput(` ${JSON.stringify(payload)} `), payload);
  for (const raw of ["", " ", "CANARY77", '{"authToken":"CANARY77"']) {
    assert.throws(() => api.parseInput(raw), (error) => {
      assert.equal(error.status, 400);
      assert.equal(error.code, "producer_browser_invalid_json");
      assert.equal(error.message, "Producer worker input must be valid JSON.");
      assert.equal(error.cause, undefined);
      return true;
    });
  }
  assert.throws(() => api.parseInput("null"), { code: "producer_browser_invalid_input" });
  assert.throws(() => api.parseInput("{}"), { code: "producer_browser_missing_base_url" });
});
