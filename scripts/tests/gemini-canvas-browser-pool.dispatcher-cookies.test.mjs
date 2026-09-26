import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { callsAt, dispatchBaseUrl, dispatcherHarness } from "./gemini-canvas-browser-pool.dispatcher-fixtures.mjs";

const app = await importTestableScript();
for (const { name, options, args, syncs, authChecks = 1, embedded = 1 } of [
  { name: "authoritative profile auth", options: {}, syncs: 0 },
  { name: "missing profile auth", options: { authCookies: [false] }, syncs: 1 },
  { name: "storage state file", options: { entry: { runtimeStateMode: "storage_state_file" } }, syncs: 1, embedded: 0 },
  { name: "cloned profile", options: { entry: { launchClonedProfile: true } }, syncs: 1 },
  { name: "embedded auth restoration", options: { authCookies: [false, true], embeddedCount: 2 }, syncs: 0, authChecks: 2 },
  { name: "embedded non-auth cookies", options: { authCookies: [false, false], embeddedCount: 2 }, syncs: 1, authChecks: 2 },
  { name: "explicit false suppresses missing-auth sync", options: { authCookies: [false] }, args: { forceCookieSync: "false" }, syncs: 0 },
  { name: "forced true preserves authoritative auth", options: {}, args: { forceCookieSync: "true" }, syncs: 0 },
  { name: "environment false", options: { authCookies: [false], env: { GEMINI_CANVAS_BROWSER_FORCE_COOKIE_SYNC: "false" } }, syncs: 0 },
  { name: "argument overrides environment", options: { authCookies: [false], env: { GEMINI_CANVAS_BROWSER_FORCE_COOKIE_SYNC: "false" } }, args: { forceCookieSync: "true" }, syncs: 1 },
  { name: "invalid override uses default", options: { authCookies: [false] }, args: { forceCookieSync: "invalid" }, syncs: 1 },
]) {
  test(`Dispatcher cookie policy preserves ${name}`, async () => {
    const h = dispatcherHarness(app, options);
    assert.equal((await h.run({ cookieHeader: "fixture=value", ...args })).ok, true);
    assert.equal(callsAt(h, "cookies").length, syncs);
    assert.equal(callsAt(h, "auth").length, authChecks);
    assert.equal(callsAt(h, "embedded").length, embedded);
    if (syncs) assert.deepEqual(callsAt(h, "cookies")[0], ["cookies", h.entry.context, "fixture=value", dispatchBaseUrl]);
    if (embedded) assert.deepEqual(callsAt(h, "embedded")[0], ["embedded", h.entry.context, "fixture-profile"]);
    assert.equal(h.entry.busy, false);
  });
}

for (const failureAt of ["embedded", "cookies"]) {
  test(`Dispatcher ${failureAt} sync rejection is tolerated without skipping operation or leaking lease`, async () => {
    const h = dispatcherHarness(app, { authCookies: [false], failureAt });
    const result = await h.run();
    assert.equal(result.result, h.operationResult);
    assert.equal(callsAt(h, "auth").length, 1);
    assert.equal(callsAt(h, "cookies").length, 1);
    assert.equal(callsAt(h, "text").length, 1);
    assert.deepEqual(callsAt(h, "close"), []);
    assert.equal(h.entry.busy, false);
    assert.equal(h.entry.lastUsedAt, h.now);
  });
}

test("Dispatcher auth recheck failure after embedded sync stops cookie mirroring and releases lease", async () => {
  const h = dispatcherHarness(app, { authCookies: [false], embeddedCount: 2, failureAt: "auth", failureCall: 2 });
  assert.equal((await h.run()).error.status, 500);
  assert.equal(callsAt(h, "auth").length, 2);
  assert.deepEqual(callsAt(h, "cookies"), []);
  assert.deepEqual(callsAt(h, "text"), []);
  assert.equal(h.entry.busy, false);
});
