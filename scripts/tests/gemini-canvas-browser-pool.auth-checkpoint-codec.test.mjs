import assert from "node:assert/strict";
import test from "node:test";
import { encryptCheckpoint, decryptCheckpoint, boundedCheckpointTask, MAX_CHECKPOINT_BYTES }
  from "../gemini-canvas-browser-pool-auth-checkpoint-codec.mjs";
import { state } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";

const config = { key: Buffer.alloc(32, 7) }, binding = "synthetic-seed-binding";

test("AES-GCM roundtrip covers cookies, localStorage and IndexedDB without plaintext at rest", () => {
  const input = state("private-cookie-中文");
  input.origins.push({ origin: "https://example.invalid", localStorage: [{ name: "auth", value: "private-token" }],
    indexedDB: [{ name: "fixture", version: 1, stores: [] }] });
  const encrypted = encryptCheckpoint(config, binding, input);
  const record = decryptCheckpoint(config, binding, encrypted.bytes);
  assert.deepEqual(record.storageState, input);
  assert.equal(record.revision, encrypted.revision);
  for (const value of ["private-cookie", "private-token", "example.invalid"]) assert.equal(encrypted.bytes.includes(Buffer.from(value)), false);
  assert.notDeepEqual(encryptCheckpoint(config, binding, input).bytes, encrypted.bytes);
});

test("tampering, wrong key, seed binding and truncation are fail-closed fixed errors", () => {
  const encrypted = encryptCheckpoint(config, binding, state("secret"));
  const tampered = Buffer.from(encrypted.bytes); tampered[tampered.length - 1] ^= 1;
  for (const [key, identity, bytes] of [[config, binding, tampered], [{ key: Buffer.alloc(32, 8) }, binding, encrypted.bytes],
    [config, "other-seed", encrypted.bytes], [config, binding, encrypted.bytes.subarray(0, 20)]]) {
    assert.throws(() => decryptCheckpoint(key, identity, bytes), {
      code: "gemini_canvas_auth_checkpoint_invalid_record", message: "Gemini auth checkpoint invalid record.",
    });
  }
});

test("invalid state and oversized checkpoint cannot be committed", () => {
  assert.throws(() => encryptCheckpoint(config, binding, {}), { code: "gemini_canvas_auth_checkpoint_invalid_state" });
  assert.throws(() => encryptCheckpoint(config, binding, state("x".repeat(MAX_CHECKPOINT_BYTES))), { code: "gemini_canvas_auth_checkpoint_too_large" });
  assert.throws(() => decryptCheckpoint(config, binding, Buffer.alloc(MAX_CHECKPOINT_BYTES + 1)), { code: "gemini_canvas_auth_checkpoint_invalid_record" });
});

test("auth invalidation is an encrypted tombstone, not reusable credential state", () => {
  const record = encryptCheckpoint(config, binding, null, true);
  const decrypted = decryptCheckpoint(config, binding, record.bytes);
  assert.equal(decrypted.blocked, true);
  assert.equal(decrypted.storageState, null);
});

test("bounded checkpoint task aborts its owner and clears the deadline", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal;
  const pending = boundedCheckpointTask(async value => { signal = value; return new Promise(() => {}); });
  const rejected = assert.rejects(pending, { code: "gemini_canvas_auth_checkpoint_timeout" });
  await Promise.resolve();
  t.mock.timers.tick(5000);
  await rejected;
  assert.equal(signal.aborted, true);
  assert.equal(await boundedCheckpointTask(async () => "ok"), "ok");
});
