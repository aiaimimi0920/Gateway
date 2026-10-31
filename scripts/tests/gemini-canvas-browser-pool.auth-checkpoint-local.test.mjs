import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { DatabaseSync } from "node:sqlite";
import { loadAuthCheckpoint, saveAuthCheckpoint } from "../gemini-canvas-browser-pool-auth-checkpoint.mjs";
import { checkpointFixture, state } from "./gemini-canvas-browser-pool.auth-checkpoint-fixtures.mjs";
import { checkpointProcess } from "./gemini-canvas-browser-pool.auth-checkpoint-process-fixtures.mjs";

test("two independent processes sharing one expected revision have exactly one CAS winner", async t => {
  const f = checkpointFixture(t);
  const entry = await f.owner.ensureContext(f.args());
  const workers = [checkpointProcess(t, entry.sourceSnapshot), checkpointProcess(t, entry.sourceSnapshot)];
  const ready = await Promise.all(workers.map(worker => worker.next()));
  assert.ok(ready.every(receipt => receipt.stage === "ready"));
  for (const worker of workers) worker.child.send("write");
  const results = await Promise.all(workers.map(worker => worker.next()));
  assert.deepEqual(results.map(receipt => receipt.result).sort(), ["gemini_canvas_auth_checkpoint_conflict", "saved"]);
  const restored = await loadAuthCheckpoint(entry.sourceSnapshot);
  assert.equal(restored.storageState.cookies[0].value, "child-rotated-fixture");
});

test("fresh process restores committed state after the original writer has exited", async t => {
  const f = checkpointFixture(t);
  const entry = await f.owner.ensureContext(f.args());
  const writer = checkpointProcess(t, entry.sourceSnapshot);
  await writer.next(); writer.child.send("write");
  assert.equal((await writer.next()).result, "saved");
  await writer.exit;
  const reader = checkpointProcess(t, entry.sourceSnapshot);
  assert.equal((await reader.next()).value, "child-rotated-fixture");
  reader.child.send("read");
  assert.equal((await reader.next()).result, "saved");
});

test("forced process exit during a transaction rolls back without stale locks or seed mutation", async t => {
  const f = checkpointFixture(t);
  await f.invoke();
  const entry = await f.owner.ensureContext(f.args());
  const file = path.join(f.root, "checkpoints", "auth-checkpoints-v1.sqlite3");
  const script = `import { DatabaseSync } from "node:sqlite";
    const db = new DatabaseSync(process.env.AUTH_CHECKPOINT_TEST_DB);
    db.exec("BEGIN IMMEDIATE");
    db.prepare("UPDATE auth_checkpoint_v1 SET payload=?").run(Buffer.from("uncommitted-fixture"));
    process.send({stage:"ready"});
    setInterval(() => {}, 1000);`;
  const writer = checkpointProcess(t, entry.sourceSnapshot, script, { AUTH_CHECKPOINT_TEST_DB: file });
  assert.equal((await writer.next()).stage, "ready");
  writer.child.kill(); await writer.exit;
  const restored = await loadAuthCheckpoint(entry.sourceSnapshot);
  assert.equal(restored.storageState.cookies[0].value, "rotated-sensitive-fixture");
  await saveAuthCheckpoint(restored, state("after-crash"));
  assert.equal((await loadAuthCheckpoint(entry.sourceSnapshot)).storageState.cookies[0].value, "after-crash");
  assert.deepEqual(fs.readFileSync(f.seed), f.sourceBytes);
});

test("corrupt ciphertext is rejected before another business operation", async t => {
  const f = checkpointFixture(t);
  await f.invoke();
  const db = new DatabaseSync(path.join(f.root, "checkpoints", "auth-checkpoints-v1.sqlite3"));
  try { db.prepare("UPDATE auth_checkpoint_v1 SET payload=?").run(Buffer.from("private-corrupt-record")); }
  finally { db.close(); }
  const result = await f.invoke();
  assert.equal(result.error.code, "gemini_canvas_auth_checkpoint_invalid_record");
  assert.equal(f.counters.operations, 1);
  assert.equal(JSON.stringify(result).includes("private-corrupt-record"), false);
});

test("checkpoint key rotation creates a new namespace instead of rewriting old records", async t => {
  const f = checkpointFixture(t);
  await f.invoke();
  const oldBinding = f.owner.contexts.get("state.json").authCheckpoint.binding;
  process.env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_KEY = Buffer.alloc(32, 8).toString("base64");
  const fresh = await f.owner.ensureContext(f.args());
  assert.notEqual(fresh.authCheckpoint.binding, oldBinding);
  assert.equal(f.sessions.at(-1).loaded.cookies[0].value, "seed-sensitive-fixture");
});

test("stale auth error cannot tombstone a newer successful worker revision", async t => {
  const f = checkpointFixture(t);
  const entry = await f.owner.ensureContext(f.args());
  const winner = await loadAuthCheckpoint(entry.sourceSnapshot);
  f.checkpointHooks.operation = async () => {
    await saveAuthCheckpoint(winner, state("newer-valid-auth"));
    throw Object.assign(new Error("old auth rejected"), { status: 403 });
  };
  assert.equal((await f.invoke()).error.status, 403);
  assert.equal((await loadAuthCheckpoint(entry.sourceSnapshot)).storageState.cookies[0].value, "newer-valid-auth");
});

test("local checkpoint capacity is bounded without evicting active credentials", async t => {
  const f = checkpointFixture(t);
  const entry = await f.owner.ensureContext(f.args());
  const db = new DatabaseSync(path.join(f.root, "checkpoints", "auth-checkpoints-v1.sqlite3"));
  try {
    db.exec("BEGIN IMMEDIATE");
    const insert = db.prepare("INSERT INTO auth_checkpoint_v1 VALUES (?,?,?)");
    for (let i = 0; i < 1024; i++) insert.run(`fixture-${i}`, `fixture-revision-${i}`, Buffer.from("fixture"));
    db.exec("COMMIT");
  } finally { db.close(); }
  await assert.rejects(saveAuthCheckpoint(entry.authCheckpoint, state("new-state")), { code: "gemini_canvas_auth_checkpoint_capacity" });
  assert.equal((await loadAuthCheckpoint(entry.sourceSnapshot)).token, null);
  assert.deepEqual(fs.readFileSync(f.seed), f.sourceBytes);
});
