import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import path from "node:path";
import { loadAuthCheckpoint, saveAuthCheckpoint } from "../gemini-canvas-browser-pool-auth-checkpoint.mjs";
import { checkpointFixture, state } from "./gemini-canvas-browser-pool.auth-checkpoint-fixtures.mjs";

test("successful invocation restores rotated auth after context restart without changing import seed", async t => {
  const f = checkpointFixture(t);
  assert.equal((await f.invoke()).ok, true);
  await f.owner.closeContext("state.json");
  await f.owner.ensureContext(f.args());
  assert.equal(f.sessions[1].loaded.cookies[0].value, "rotated-sensitive-fixture");
  assert.deepEqual(fs.readFileSync(f.seed), f.sourceBytes);
  assert.equal(f.counters.exports, 1);
  const bytes = fs.readFileSync(path.join(f.root, "checkpoints", "auth-checkpoints-v1.sqlite3"));
  assert.equal(bytes.includes(Buffer.from("rotated-sensitive-fixture")), false);
});

test("unchanged checkpoint reuses a context; external CAS commit invalidates idle cached context", async t => {
  const f = checkpointFixture(t);
  assert.equal((await f.invoke()).ok, true);
  const previousCheckpoint = f.owner.contexts.get("state.json").authCheckpoint;
  const first = await f.owner.ensureContext(f.args());
  assert.equal(first.context, f.sessions[0].context);
  assert.notEqual(first.authCheckpoint, previousCheckpoint);
  const external = await loadAuthCheckpoint(first.sourceSnapshot);
  await saveAuthCheckpoint(external, state("external-rotation"));
  const second = await f.owner.ensureContext(f.args());
  assert.notEqual(second, first);
  assert.equal(f.sessions[1].loaded.cookies[0].value, "external-rotation");
  assert.deepEqual(f.sessions[0].closes, ["context", "browser"]);
});

test("reimported JSON and another account cannot resurrect the old checkpoint", async t => {
  const f = checkpointFixture(t);
  await f.invoke();
  f.write("new-import");
  await f.owner.ensureContext(f.args());
  assert.equal(f.sessions[1].loaded.cookies[0].value, "new-import");
  f.write("other-account", "other.json");
  await f.owner.ensureContext(f.args("other.json"));
  assert.equal(f.sessions[2].loaded.cookies[0].value, "other-account");
});

test("stale worker save preserves winner and successful business result without retry", async t => {
  const f = checkpointFixture(t);
  const entry = await f.owner.ensureContext(f.args());
  const winner = await loadAuthCheckpoint(entry.sourceSnapshot);
  f.checkpointHooks.operation = async () => saveAuthCheckpoint(winner, state("winner"));
  const result = await f.invoke();
  assert.deepEqual(result, { ok: true, result: { text: "accepted once" } });
  assert.equal(f.counters.operations, 1);
  assert.equal(f.owner.contexts.size, 0);
  assert.match(JSON.stringify(f.logs), /gemini_canvas_auth_checkpoint_conflict/);
  await f.owner.ensureContext(f.args());
  assert.equal(f.sessions.at(-1).loaded.cookies[0].value, "winner");
});

test("auth failure tombstones only the leased revision until a new seed is imported", async t => {
  const f = checkpointFixture(t);
  await f.invoke();
  f.checkpointHooks.failure = Object.assign(new Error("synthetic auth failure"), { status: 401 });
  assert.equal((await f.invoke()).error.status, 401);
  assert.equal(f.counters.exports, 1);
  const blocked = await f.invoke();
  assert.equal(blocked.error.code, "gemini_canvas_auth_checkpoint_session_invalid");
  assert.equal(f.counters.operations, 2);
  f.write("new-login-seed");
  delete f.checkpointHooks.failure;
  assert.equal((await f.invoke()).ok, true);
});

test("busy lease remains owned through checkpoint capture and prevents overlapping generation", async t => {
  const f = checkpointFixture(t);
  let release, entered;
  const capture = new Promise(resolve => { entered = resolve; });
  f.checkpointHooks.capture = () => { entered(); return new Promise(resolve => { release = resolve; }); };
  const first = f.invoke();
  await capture;
  assert.equal(f.owner.contexts.get("state.json").busy, true);
  assert.equal((await f.invoke()).error.status, 429);
  release();
  assert.equal((await first).ok, true);
  assert.equal(f.owner.contexts.get("state.json").busy, false);
  assert.equal(f.counters.operations, 1);
});

test("capture failure remains redacted and cannot replace successful operation result", async t => {
  const f = checkpointFixture(t);
  f.checkpointHooks.capture = async () => { throw new Error("private-cookie-token-must-not-leak"); };
  assert.equal((await f.invoke()).ok, true);
  assert.equal(f.counters.operations, 1);
  assert.equal(f.owner.contexts.size, 0);
  assert.equal(JSON.stringify(f.logs).includes("private-cookie-token-must-not-leak"), false);
});

test("non-auth failures and contexts without auth cookies do not export state", async t => {
  const f = checkpointFixture(t);
  f.checkpointHooks.failure = Object.assign(new Error("network failed"), { status: 502 });
  assert.equal((await f.invoke()).ok, false);
  delete f.checkpointHooks.failure;
  f.checkpointHooks.auth = false;
  assert.equal((await f.invoke()).ok, true);
  assert.equal(f.counters.exports, 0);
});

test("native persistent profiles and external CDP snapshots bypass checkpoint config", async t => {
  const f = checkpointFixture(t);
  process.env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_KEY = "invalid";
  assert.equal(await loadAuthCheckpoint(null), null);
  assert.equal(await loadAuthCheckpoint({ mode: "profile_dir" }), null);
  assert.equal(f.counters.exports, 0);
});

test("all successful operation branches including bootstrap persist auth", async t => {
  const f = checkpointFixture(t);
  for (const operation of ["bootstrap_program", "text", "tts", "debug", "image"]) {
    assert.equal((await f.invoke({ operation })).ok, true);
  }
  f.checkpointHooks.result = { ok: true, status: 200 };
  assert.equal((await f.invoke({ fetchRequest: { url: "https://example.invalid" } })).ok, true);
  assert.equal(f.counters.operations, 6);
  assert.equal(f.counters.exports, 6);
});

test("proxied fetch auth errors tombstone the checkpoint without changing response semantics", async t => {
  const f = checkpointFixture(t);
  f.checkpointHooks.result = { ok: false, status: 401 };
  const result = await f.invoke({ fetchRequest: { url: "https://example.invalid" } });
  assert.deepEqual(result, { ok: true, result: { ok: false, status: 401 } });
  assert.equal(f.counters.exports, 0);
  assert.equal(f.owner.contexts.size, 0);
  assert.equal((await f.invoke()).error.code, "gemini_canvas_auth_checkpoint_session_invalid");
});

test("proxied non-auth HTTP errors do not produce a successful auth checkpoint", async t => {
  const f = checkpointFixture(t);
  f.checkpointHooks.result = { ok: false, status: 503 };
  assert.equal((await f.invoke({ fetchRequest: { url: "https://example.invalid" } })).ok, true);
  assert.equal(f.counters.exports, 0);
  assert.equal(f.owner.contexts.get("state.json").authCheckpoint.token, null);
});

test("timed-out capture cannot publish late state or leave the entry leased", async t => {
  const f = checkpointFixture(t);
  let entered, release;
  const captured = new Promise(resolve => { entered = resolve; });
  f.checkpointHooks.capture = () => { entered(); return new Promise(resolve => { release = resolve; }); };
  const entry = await f.owner.ensureContext(f.args());
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const operation = f.invoke();
  await captured;
  t.mock.timers.tick(5000);
  assert.equal((await operation).ok, true);
  assert.equal(entry.busy, false);
  assert.equal(f.owner.contexts.size, 0);
  release();
  await Promise.resolve(); await Promise.resolve();
  assert.equal((await loadAuthCheckpoint(entry.sourceSnapshot)).token, null);
  assert.equal(f.counters.operations, 1);
});
