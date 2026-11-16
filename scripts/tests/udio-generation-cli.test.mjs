import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const fixture = fileURLToPath(new URL("./fixtures/udio-generation-cli.mjs", import.meta.url));
function run(mode, waitAudio = true) {
  const result = spawnSync(process.execPath, [fixture], { input: JSON.stringify({
    baseUrl: "https://www.udio.com", browserCdpUrl: "http://127.0.0.1:1", requestBody: { prompt: "local fixture" },
    waitAudio, waitTimeoutMs: 15000, pollIntervalMs: 1000, timeoutMs: 30000,
  }), env: { ...process.env, UDIO_GENERATION_FIXTURE: mode }, encoding: "utf8", timeout: 10000 });
  assert.ifError(result.error); assert.equal(result.status, 0, result.stderr);
  return { value: JSON.parse(result.stdout), submits: (result.stderr.match(/fixture:submit/g) ?? []).length,
    polls: (result.stderr.match(/fixture:poll/g) ?? []).length, stderr: result.stderr };
}
test("Udio CLI waits for the entire original track set and restores order", () => {
  const r = run("partial"); assert.equal(r.value.ok, true); assert.equal(r.polls, 2);
  assert.deepEqual(r.value.result.songs.map(s => s.id), ["A", "B"]);
});
for (const mode of ["bad-ids", "bad-song", "failed", "foreign"]) {
  test(`Udio CLI rejects ${mode} instead of completing unrelated or malformed songs`, () => {
    const r = run(mode, mode !== "bad-ids" && mode !== "bad-song"); assert.equal(r.value.ok, false);
    assert.equal(r.submits, 1);
  });
}
for (const mode of ["disjoint", "timeout"]) {
  test(`Udio CLI ${mode} is 504, not successful incomplete output`, () => {
    const r = run(mode); assert.equal(r.value.ok, false); assert.equal(r.value.error.status, 504);
  });
}
test("Udio CLI challenge never automatically resubmits generation", () => {
  const r = run("challenge"); assert.equal(r.value.ok, false); assert.equal(r.submits, 1);
  assert.equal(r.value.error.code, "udio_browser_challenge_required");
  assert.ok(!JSON.stringify(r.value).includes("Vercel Security Checkpoint"));
});
test("Udio CLI exhausted total deadline does not submit", () => {
  const r = run("late"); assert.equal(r.value.error?.status, 504); assert.equal(r.submits, 0);
});
test("Udio CLI explicit no-wait retains original pending identity and disconnects", () => {
  const r = run("pending", false); assert.equal(r.value.ok, true); assert.equal(r.value.result.completed, false);
  assert.deepEqual(r.value.result.trackIds, ["A", "B"]); assert.equal(r.polls, 0);
  assert.match(r.stderr, /fixture:close/);
});
test("Udio CLI cleanup failure cannot masquerade as success", () => {
  const r = run("close-error", false); assert.equal(r.value.ok, false);
  assert.ok(!JSON.stringify(r.value).includes("private close fixture"));
});
