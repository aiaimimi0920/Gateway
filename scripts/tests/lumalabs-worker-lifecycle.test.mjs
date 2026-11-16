import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const worker = fileURLToPath(new URL("../lumalabs-browser-worker.mjs", import.meta.url));
const preload = new URL("./fixtures/lumalabs-worker-lifecycle-preload.mjs", import.meta.url).href;
function run(mode, overrides = {}) {
  const child = spawnSync(process.execPath, ["--import", preload, worker], {
    input: JSON.stringify({ baseUrl: "https://app.lumalabs.ai", realmId: "realm-fixture",
      sessionToken: "synthetic-private", actionBody: {}, browserExecutablePath: process.execPath, ...overrides }),
    encoding: "utf8", timeout: 10000,
    env: { ...process.env, LUMA_LIFECYCLE_FIXTURE: mode },
  });
  assert.ifError(child.error);
  const payload = JSON.parse(child.stdout);
  assert.equal(child.status, 0, child.stderr);
  return { payload, events: child.stderr.trim().split(/\r?\n/) };
}
test("Luma Web closes owned browser before emitting success", () => {
  const { payload, events } = run("success");
  assert.equal(payload.ok, true);
  assert.deepEqual(events, ["fixture:launch", "fixture:goto", "fixture:generate", "fixture:close", "fixture:emit"]);
});
test("Luma Web navigation exception is redacted after closing the browser", () => {
  const { payload, events } = run("goto-error");
  assert.equal(payload.error.code, "lumalabs_browser_worker_failed");
  assert.ok(!JSON.stringify(payload).includes("synthetic-private"));
  assert.deepEqual(events, ["fixture:launch", "fixture:goto", "fixture:close", "fixture:emit"]);
});
test("Luma Web launch/navigation consume the operation deadline before submission", () => {
  const { payload, events } = run("deadline", { timeoutMs: 10 });
  assert.equal(payload.error.code, "lumalabs_browser_timeout");
  assert.equal(payload.error.status, 504);
  assert.ok(!events.includes("fixture:generate"));
  assert.equal(events.at(-2), "fixture:close");
});
test("Luma Web CLI bounds input before browser access", () => {
  const { payload, events } = run("success", { padding: "x".repeat(1024 * 1024) });
  assert.equal(payload.error.code, "lumalabs_browser_worker_input_too_large");
  assert.deepEqual(events, ["fixture:emit"]);
});
test("Luma Web Playwright TimeoutError retains timeout classification after cleanup", () => {
  const { payload, events } = run("playwright-timeout");
  assert.equal(payload.error.code, "lumalabs_browser_timeout");
  assert.equal(payload.error.status, 504);
  assert.ok(!JSON.stringify(payload).includes("synthetic-private"));
  assert.deepEqual(events, ["fixture:launch", "fixture:goto", "fixture:close", "fixture:emit"]);
});
test("Luma Web auth redirect closes browser and does not submit or expose final URL", () => {
  const { payload, events } = run("auth");
  assert.equal(payload.error.code, "lumalabs_browser_auth_redirect");
  assert.ok(!JSON.stringify(payload).includes("synthetic-private"));
  assert.deepEqual(events, ["fixture:launch", "fixture:goto", "fixture:close", "fixture:emit"]);
});
test("Luma Web close failure cannot report generation success", () => {
  const { payload, events } = run("close-error");
  assert.equal(payload.ok, false);
  assert.equal(payload.error.code, "lumalabs_browser_cleanup_failed");
  assert.ok(!JSON.stringify(payload).includes("synthetic-private"));
  assert.equal(events.at(-2), "fixture:close");
  assert.equal(events.at(-1), "fixture:emit");
});
test("Luma Web business error is emitted only after browser cleanup", () => {
  const { payload, events } = run("operation-error");
  assert.equal(payload.error.code, "lumalabs_fixture_error");
  assert.equal(events.at(-2), "fixture:close");
  assert.equal(events.at(-1), "fixture:emit");
});
test("Luma Web requires exact board origin and pathname, not a substring", () => {
  for (const mode of ["foreign-board", "query-board"]) {
    const { payload, events } = run(mode);
    assert.equal(payload.error?.code, "lumalabs_browser_auth_redirect");
    assert.ok(!events.includes("fixture:generate"));
    assert.equal(events.at(-2), "fixture:close");
  }
});
