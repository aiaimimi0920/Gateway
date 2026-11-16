import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const worker = fileURLToPath(new URL("../suno-browser-worker.mjs", import.meta.url));
const preload = new URL("./fixtures/suno-browser-generation-preload.mjs", import.meta.url).href;
function run(mode, waitCompletion = true, lifecycleMode = "success") {
  const child = spawnSync(process.execPath, ["--import", preload, worker], {
    input: JSON.stringify({
      baseUrl: "https://suno.fixture.invalid", referer: "https://suno.fixture.invalid/create",
      requestBody: { model: "chirp-auk-turbo", prompt: "synthetic fixture" }, waitCompletion, waitTimeoutMs: 10000,
      easyBrowserBaseUrl: "https://easybrowser.fixture.invalid", easyBrowserBearerToken: "synthetic-fixture",
    }), encoding: "utf8", timeout: 15000,
    env: { ...process.env, SUNO_GENERATION_FIXTURE: mode,
      SUNO_LIFECYCLE_FIXTURE: lifecycleMode, SUNO_BROWSER_CDP_URL: "", SUNO_EASYBROWSER_BASE_URL: "" },
  });
  assert.ifError(child.error);
  assert.equal(child.status, 0, child.stderr);
  return JSON.parse(child.stdout);
}
test("Suno browser does not complete from a partial snapshot and restores original ID order", () => {
  const value = run("partial");
  assert.equal(value.ok, true);
  assert.equal(value.result.completed, true);
  assert.deepEqual(value.result.clips.map((c) => c.id), ["A", "B"]);
});
test("Suno browser rejects malformed generation IDs before no-wait success", () => {
  assert.equal(run("malformed-submit", false).error?.code, "suno_browser_clip_identity");
});
test("Suno browser rejects duplicate feed identity", () => {
  assert.equal(run("duplicate").error?.code, "suno_browser_clip_identity");
});
test("Suno browser explicit failed submission is not no-wait success", () => {
  assert.equal(run("failed-submit", false).error?.code, "suno_browser_generation_failed");
});
test("Suno browser explicit failed original clip cannot hide behind complete sibling", () => {
  assert.equal(run("failed-feed").error?.code, "suno_browser_generation_failed");
});
test("Suno browser disjoint partial snapshots never combine into completion", () => {
  assert.equal(run("disjoint").error?.status, 504);
});
test("Suno browser wait timeout is an error, not a successful incomplete envelope", () => {
  const value = run("timeout");
  assert.equal(value.ok, false);
  assert.equal(value.error?.status, 504);
});
test("Suno browser unrelated account feed never completes this generation", () => {
  assert.equal(run("foreign").error?.status, 504);
});
test("Suno browser explicit no-wait preserves the complete pending ID set", () => {
  const value = run("pending", false);
  assert.equal(value.ok, true);
  assert.equal(value.result.completed, false);
  assert.deepEqual(value.result.clips.map((c) => c.id), ["A", "B"]);
});
test("Suno browser failed click cancels its captured response waiter without unhandled rejection", () => {
  const value = run("pending", true, "click-error");
  assert.equal(value.ok, false);
  assert.equal(value.error.code, "suno_browser_generation_failed");
  assert.ok(!value.error.message.includes("private fixture"));
});
