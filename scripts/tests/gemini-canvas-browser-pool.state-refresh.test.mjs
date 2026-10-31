import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { refreshFixture } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";

test("same-key replacement is detected even when size and mtime are unchanged", async t => {
  const f = refreshFixture(t), file = f.write("old");
  const before = fs.statSync(file), original = await f.owner.ensureContext(f.args());
  assert.equal(await f.owner.ensureContext(f.args()), original);
  f.write("new");
  fs.utimesSync(file, before.atime, before.mtime);
  const replacement = await f.owner.ensureContext(f.args());
  assert.notEqual(replacement, original);
  assert.equal(original.contextClosed, true);
  assert.deepEqual(f.sessions[0].closes, ["context", "browser"]);
  assert.equal(f.sessions[1].loaded.cookies[0].value, "new");
  assert.equal(f.launch.mock.callCount(), 2);
});

test("new context consumes the parsed snapshot, not a path that can change after hashing", async t => {
  const f = refreshFixture(t);
  f.write("one");
  f.hooks.beforeContext = () => f.write("two");
  await f.owner.ensureContext(f.args());
  assert.equal(typeof f.sessions[0].input, "object");
  assert.equal(f.sessions[0].loaded.cookies[0].value, "one");
  f.hooks.beforeContext = undefined;
  await f.owner.ensureContext(f.args());
  assert.equal(f.sessions[1].loaded.cookies[0].value, "two");
});

test("busy context is not closed by source changes; refresh happens after release", async t => {
  const f = refreshFixture(t);
  f.write("old");
  const original = await f.owner.ensureContext(f.args());
  original.busy = true;
  f.write("new");
  assert.equal(await f.owner.ensureContext(f.args()), original);
  assert.deepEqual(f.sessions[0].closes, []);
  assert.equal(original.busy, true);
  original.busy = false;
  assert.notEqual(await f.owner.ensureContext(f.args()), original);
});

test("concurrent refresh callers share one replacement and never duplicate context creation", async t => {
  const f = refreshFixture(t);
  f.write("old");
  const original = await f.owner.ensureContext(f.args());
  f.write("new");
  const entries = await Promise.all(Array.from({ length: 8 }, () => f.owner.ensureContext(f.args())));
  assert.notEqual(entries[0], original);
  assert.ok(entries.every(entry => entry === entries[0]));
  assert.equal(f.launch.mock.callCount(), 2);
  assert.equal(f.owner.initializingContexts.size, 0);
});

test("invalid replacement fails before reuse without logging credential contents", async t => {
  const f = refreshFixture(t), file = f.write("old");
  const original = await f.owner.ensureContext(f.args());
  fs.writeFileSync(file, '{"private":"DO-NOT-LOG",', "utf8");
  await assert.rejects(f.owner.ensureContext(f.args()), error =>
    error.code === "gemini_canvas_runtime_state_invalid_json" && !error.message.includes("DO-NOT-LOG"));
  assert.equal(f.owner.contexts.get("state.json"), original);
  assert.equal(original.busy, false);
  assert.deepEqual(f.sessions[0].closes, []);
  assert.equal(JSON.stringify(f.logs).includes("DO-NOT-LOG"), false);
});

test("identical contents under different keys retain independent contexts", async t => {
  const f = refreshFixture(t);
  f.write("same", "a.json"); f.write("same", "b.json");
  assert.notEqual(await f.owner.ensureContext(f.args("a.json")), await f.owner.ensureContext(f.args("b.json")));
  assert.equal(f.launch.mock.callCount(), 2);
});

test("removed JSON fails closed and is never recreated as a profile directory", async t => {
  const f = refreshFixture(t), file = f.write("old");
  const original = await f.owner.ensureContext(f.args());
  fs.unlinkSync(file);
  await assert.rejects(f.owner.ensureContext(f.args()), error => error.code === "gemini_canvas_runtime_state_read_failed");
  assert.equal(fs.existsSync(file), false);
  assert.equal(f.owner.contexts.get("state.json"), original);
  assert.equal(original.busy, false);
  assert.equal(f.launch.mock.callCount(), 1);
});

test("invalid source cannot reach invocation or silently fall back to a cached context", async t => {
  const { createInvocationDispatcher } = await import("../gemini-canvas-browser-pool-dispatcher.mjs");
  const f = refreshFixture(t), file = f.write("old"), calls = [];
  const { invokeGeminiCanvas } = createInvocationDispatcher({ DEFAULT_TIMEOUT_MS: 1000, log: () => {},
    ensureContext: f.owner.ensureContext, closeContext: f.owner.closeContext,
    contextHasGeminiAuthCookies: async () => true, syncEmbeddedStorageStateIntoContext: async () => 0,
    syncCookieHeaderIntoContext: async () => 0, resolveProgramPageUrl: () => null,
    runTextOperation: async entry => { calls.push(entry); return { text: "synthetic" }; } });
  const args = { ...f.args(), operation: "text", requireAppPage: false };
  assert.equal((await invokeGeminiCanvas(args)).ok, true);
  fs.writeFileSync(file, "invalid", "utf8");
  const result = await invokeGeminiCanvas(args);
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "gemini_canvas_runtime_state_invalid_json");
  assert.equal(calls.length, 1);
  assert.equal(f.launch.mock.callCount(), 1);
});

test("removed embedded JSON cannot silently switch its existing context to an empty profile", async t => {
  const f = refreshFixture(t), file = f.write("old", "bundle/storage-state.json");
  const args = f.args("bundle"), original = await f.owner.ensureContext(args);
  fs.unlinkSync(file);
  await assert.rejects(f.owner.ensureContext(args), error => error.code === "gemini_canvas_runtime_state_read_failed");
  assert.equal(f.owner.contexts.get("bundle"), original);
  assert.equal(original.busy, false);
  assert.equal(fs.existsSync(path.join(f.root, "bundle", "Default")), false);
  assert.equal(f.launch.mock.callCount(), 1);
});
