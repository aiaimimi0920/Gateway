import assert from "node:assert/strict";
import test from "node:test";
import { loadAuthCheckpoint, saveAuthCheckpoint, settleAuthCheckpoint } from "../gemini-canvas-browser-pool-auth-checkpoint.mjs";
import { MAX_CHECKPOINT_BYTES } from "../gemini-canvas-browser-pool-auth-checkpoint-codec.mjs";
import { state } from "./gemini-canvas-browser-pool.auth-checkpoint-fixtures.mjs";
import { checkpointS3 } from "./gemini-canvas-browser-pool.auth-checkpoint-s3-fixtures.mjs";
import { S3Client } from "@aws-sdk/client-s3";

test("S3 roundtrip writes only encrypted dedicated objects using create/update preconditions", async t => {
  const f = await checkpointS3(t);
  const initial = await loadAuthCheckpoint(f.source);
  assert.equal(initial.token, null);
  await saveAuthCheckpoint(initial, state("private-rotated-auth"));
  const restored = await loadAuthCheckpoint(f.source);
  assert.equal(restored.storageState.cookies[0].value, "private-rotated-auth");
  await saveAuthCheckpoint(restored, state("next-rotation"));
  const puts = f.calls.filter(call => call.method === "PUT");
  assert.equal(puts[0].headers["if-none-match"], "*");
  assert.equal(puts[1].headers["if-match"], '"checkpoint-1"');
  assert.ok(f.calls.every(call => /^\/fixture-bucket\/private-auth-checkpoints\/[a-f0-9]{64}\.checkpoint/.test(call.url)));
  assert.equal(puts[0].bytes.includes(Buffer.from("private-rotated-auth")), false);
  assert.equal(puts[0].headers["content-type"], "application/octet-stream");
});

test("S3 conditional create and update conflicts never retry or overwrite winner", async t => {
  const f = await checkpointS3(t);
  const a = await loadAuthCheckpoint(f.source), b = await loadAuthCheckpoint(f.source);
  await saveAuthCheckpoint(a, state("winner-v1"));
  await assert.rejects(saveAuthCheckpoint(b, state("stale-create")), { code: "gemini_canvas_auth_checkpoint_conflict" });
  const c = await loadAuthCheckpoint(f.source);
  await saveAuthCheckpoint(a, state("winner-v2"));
  await assert.rejects(saveAuthCheckpoint(c, state("stale-update")), { code: "gemini_canvas_auth_checkpoint_conflict" });
  assert.equal(f.calls.filter(call => call.method === "PUT").length, 4);
  assert.equal((await loadAuthCheckpoint(f.source)).storageState.cookies[0].value, "winner-v2");
});

test("S3 writeback errors do not retry accepted business or leak response bodies", async t => {
  const f = await checkpointS3(t);
  const checkpoint = await loadAuthCheckpoint(f.source);
  const logs = [], closed = [];
  f.hooks.respond = async (req, res) => {
    res.writeHead(503); res.end("private-upstream-token-cookie"); return true;
  };
  await settleAuthCheckpoint({ authCheckpoint: checkpoint, context: { storageState: async () => state("private-state") } },
    { succeeded: true, runtimeStateObjectKey: "fixture-key" }, { log: (...args) => logs.push(args),
      contextHasGeminiAuthCookies: async () => true, closeContext: async key => closed.push(key) });
  assert.equal(f.calls.filter(call => call.method === "PUT").length, 1);
  assert.deepEqual(closed, ["fixture-key"]);
  assert.equal(JSON.stringify(logs).includes("private-upstream-token-cookie"), false);
  assert.match(JSON.stringify(logs), /gemini_canvas_auth_checkpoint_remote_storage_failed/);
});

test("missing ETags, redirects and remote read errors fail closed without stale fallback", async t => {
  const f = await checkpointS3(t);
  const checkpoint = await loadAuthCheckpoint(f.source);
  await saveAuthCheckpoint(checkpoint, state("stored"));
  for (const status of [200, 307, 403, 500]) {
    f.hooks.respond = async (req, res) => {
      res.writeHead(status, { Location: `${f.endpoint}/must-not-follow` });
      res.end("private-read-response"); return true;
    };
    const count = f.calls.length;
    await assert.rejects(loadAuthCheckpoint(f.source), error => {
      assert.equal(error.message.includes("private-read-response"), false);
      return error.code.startsWith("gemini_canvas_auth_checkpoint_");
    });
    assert.equal(f.calls.length, count + 1);
  }
});

test("oversized and tampered remote ciphertext cannot become usable auth", async t => {
  const f = await checkpointS3(t);
  const checkpoint = await loadAuthCheckpoint(f.source);
  await saveAuthCheckpoint(checkpoint, state("stored"));
  f.records.values().next().value.bytes[40] ^= 1;
  await assert.rejects(loadAuthCheckpoint(f.source), { code: "gemini_canvas_auth_checkpoint_invalid_record" });
  f.hooks.respond = async (req, res) => {
    res.writeHead(200, { ETag: '"oversized"', "Content-Length": MAX_CHECKPOINT_BYTES + 1 });
    res.end(); return true;
  };
  await assert.rejects(loadAuthCheckpoint(f.source), { code: "gemini_canvas_auth_checkpoint_remote_storage_failed" });
});

test("S3 tombstone, source revision and storage namespace remain independent", async t => {
  const f = await checkpointS3(t);
  const checkpoint = await loadAuthCheckpoint(f.source);
  await saveAuthCheckpoint(checkpoint, null, { blocked: true });
  await assert.rejects(loadAuthCheckpoint(f.source), { code: "gemini_canvas_auth_checkpoint_session_invalid" });
  assert.equal((await loadAuthCheckpoint({ ...f.source, fingerprint: "new-seed" })).token, null);
  process.env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_PREFIX = "different-namespace";
  assert.equal((await loadAuthCheckpoint(f.source)).token, null);
});

test("S3 config rejects unsafe prefix and non-loopback plaintext endpoints before network", async t => {
  const f = await checkpointS3(t);
  for (const prefix of ["../seed", "/absolute", "prefix//empty", ""]) {
    process.env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_PREFIX = prefix;
    await assert.rejects(loadAuthCheckpoint(f.source), { code: "gemini_canvas_auth_checkpoint_config_invalid" });
  }
  process.env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_PREFIX = "valid";
  process.env.AI_GATEWAY_OBJECT_STORAGE_ENDPOINT = "http://example.invalid";
  await assert.rejects(loadAuthCheckpoint(f.source), { code: "gemini_canvas_auth_checkpoint_config_invalid" });
  assert.equal(f.calls.length, 0);
});

test("S3 deadline aborts the SDK request and destroys the request-scoped client", async t => {
  const f = await checkpointS3(t);
  let entered, requestSignal;
  const started = new Promise(resolve => { entered = resolve; });
  const destroy = t.mock.method(S3Client.prototype, "destroy", () => {});
  t.mock.method(S3Client.prototype, "send", async (command, options) => {
    requestSignal = options.abortSignal;
    entered();
    return new Promise(() => {});
  });
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const read = loadAuthCheckpoint(f.source);
  const rejected = assert.rejects(read, { code: "gemini_canvas_auth_checkpoint_timeout" });
  await started;
  t.mock.timers.tick(5000);
  await rejected;
  assert.equal(requestSignal.aborted, true);
  assert.equal(destroy.mock.callCount(), 1);
  assert.equal(f.calls.length, 0);
});

test("streamed S3 size admission destroys the body instead of trusting Content-Length", async t => {
  const f = await checkpointS3(t);
  let destroyed = false;
  const body = { async *[Symbol.asyncIterator]() {
    for (let i = 0; i < 17; i++) yield Buffer.alloc(1024 * 1024);
  }, destroy() { destroyed = true; } };
  t.mock.method(S3Client.prototype, "send", async () => ({ Body: body, ETag: '"fixture"' }));
  await assert.rejects(loadAuthCheckpoint(f.source), { code: "gemini_canvas_auth_checkpoint_remote_storage_failed" });
  assert.equal(destroyed, true);
});
