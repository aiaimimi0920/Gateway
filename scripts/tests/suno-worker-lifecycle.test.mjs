import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const worker = fileURLToPath(new URL("../suno-browser-worker.mjs", import.meta.url));
const preload = new URL("./fixtures/suno-worker-lifecycle-preload.mjs", import.meta.url).href;
function run(mode, owned = false) {
  const input = {
    baseUrl: "https://suno.fixture.invalid", referer: "https://suno.fixture.invalid/create",
    requestBody: { model: "chirp-auk-turbo", prompt: "synthetic fixture" }, waitCompletion: false,
    browserExecutablePath: process.execPath,
    easyBrowserBearerToken: "synthetic-fixture",
    ...(!owned && { easyBrowserBaseUrl: "https://easybrowser.fixture.invalid" }),
  };
  const child = spawnSync(process.execPath, ["--import", preload, worker], {
    input: JSON.stringify(input), encoding: "utf8", timeout: 10000,
    env: { ...process.env, SUNO_LIFECYCLE_FIXTURE: mode,
      SUNO_BROWSER_CDP_URL: "", SUNO_EASYBROWSER_BASE_URL: "" },
  });
  assert.ifError(child.error);
  assert.equal(child.status, 0, child.stderr);
  return { payload: JSON.parse(child.stdout), events: child.stderr.trim().split(/\r?\n/) };
}

test("Suno borrowed success disconnects then releases lease before emitting", () => {
  const { payload, events } = run("success");
  assert.equal(payload.ok, true);
  assert.deepEqual(events, ["fixture:close", "fixture:release", "fixture:emit"]);
});
test("Suno challenged session still disconnects and releases before emitting error", () => {
  const { payload, events } = run("challenge");
  assert.equal(payload.error.code, "suno_browser_challenge_required");
  assert.deepEqual(events, ["fixture:close", "fixture:release", "fixture:emit"]);
});
test("Suno owned browser closes before emitting success", () => {
  const { payload, events } = run("success", true);
  assert.equal(payload.ok, true);
  assert.deepEqual(events, ["fixture:close", "fixture:emit"]);
});
test("Suno close failure still releases lease and cannot report success", () => {
  const { payload, events } = run("close-error");
  assert.equal(payload.ok, false);
  assert.equal(payload.error.code, "suno_browser_cleanup_failed");
  assert.ok(!JSON.stringify(payload).includes("private fixture"));
  assert.deepEqual(events, ["fixture:close", "fixture:release", "fixture:emit"]);
});
test("Suno release HTTP failure cannot report success", () => {
  const { payload, events } = run("release-error");
  assert.equal(payload.error.code, "suno_browser_cleanup_failed");
  assert.deepEqual(events, ["fixture:close", "fixture:release", "fixture:emit"]);
});
test("Suno acquired lease is released when CDP attach fails", () => {
  const { payload, events } = run("connect-error");
  assert.equal(payload.ok, false);
  assert.deepEqual(events, ["fixture:release", "fixture:emit"]);
});
test("Suno CLI bounds input before browser access", () => {
  const child = spawnSync(process.execPath, [worker], {
    input: JSON.stringify({ padding: "x".repeat(1024 * 1024) }),
    encoding: "utf8", timeout: 10000,
  });
  assert.ifError(child.error);
  assert.equal(child.status, 0, child.stderr);
  assert.equal(JSON.parse(child.stdout).error.code, "suno_browser_worker_input_too_large");
});
