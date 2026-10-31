import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import test from "node:test";
import { S3Client } from "@aws-sdk/client-s3";
import { loadContextStateSnapshot } from "../gemini-canvas-browser-pool-state-snapshot.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

function remoteFixture(t) {
  const root = storageFixture(t);
  Object.assign(process.env, { AI_GATEWAY_OBJECT_STORAGE_DRIVER: "s3", AI_GATEWAY_OBJECT_STORAGE_BUCKET: "fixture",
    AI_GATEWAY_OBJECT_STORAGE_ENDPOINT: "http://127.0.0.1:1", AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID: "FIXTURE",
    AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY: "fixture-only" });
  return root;
}

test("local state snapshot admits BOM JSON and rejects invalid UTF-8, shape and oversized state", async t => {
  const root = storageFixture(t), file = path.join(root, "state.json");
  fs.writeFileSync(file, '\ufeff{"cookies":[],"origins":[]}', "utf8");
  const first = await loadContextStateSnapshot("state.json");
  assert.deepEqual(first.storageState, { cookies: [], origins: [] });
  for (const bytes of [Buffer.from([0xff, 0xfe]), Buffer.from('{"cookies":[],"origins":{}}')]) {
    fs.writeFileSync(file, bytes);
    await assert.rejects(loadContextStateSnapshot("state.json"), error => error.code === "gemini_canvas_runtime_state_invalid_json");
  }
  fs.writeFileSync(file, Buffer.alloc(16 * 1024 * 1024 + 1, 32));
  await assert.rejects(loadContextStateSnapshot("state.json"), error => error.status === 413);
});

test("relative JSON paths cannot traverse the storage root or create missing directories", async t => {
  storageFixture(t);
  await assert.rejects(loadContextStateSnapshot("../outside.json"), error => error.code === "gemini_canvas_runtime_state_invalid_path");
  await assert.rejects(loadContextStateSnapshot("missing/deleted.json"), error => error.code === "gemini_canvas_runtime_state_read_failed");
});

test("remote malformed JSON is classified without writing a mirror or exposing its body", async t => {
  const root = remoteFixture(t);
  t.mock.method(S3Client.prototype, "send", async () => ({ Body: Readable.from(['{"SECRET":"PRIVATE",']), ETag: '"1"' }));
  await assert.rejects(loadContextStateSnapshot("state.json"), error => error.code === "gemini_canvas_runtime_state_invalid_json"
    && !error.message.includes("PRIVATE"));
  assert.deepEqual(fs.readdirSync(root), []);
});

test("declared and streamed remote size limits destroy bodies and remain bounded", async t => {
  remoteFixture(t);
  let body = Readable.from(["unused"]), declaredLength = 16 * 1024 * 1024 + 1;
  t.mock.method(S3Client.prototype, "send", async () => ({ Body: body, ContentLength: declaredLength }));
  await assert.rejects(loadContextStateSnapshot("state.json"), error => error.status === 413);
  assert.equal(body.destroyed, true);
  const chunk = Buffer.alloc(65536, 32);
  body = Readable.from(Array(257).fill(chunk)); declaredLength = undefined;
  await assert.rejects(loadContextStateSnapshot("state.json"), error => error.status === 413);
  assert.equal(body.destroyed, true);
});

test("remote deadlines abort the SDK operation rather than serving old credentials", async t => {
  remoteFixture(t);
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal;
  t.mock.method(S3Client.prototype, "send", async (_command, options) => {
    signal = options.abortSignal;
    return new Promise((_resolve, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true }));
  });
  const read = loadContextStateSnapshot("state.json");
  await Promise.resolve(); await Promise.resolve();
  t.mock.timers.tick(30000);
  await assert.rejects(read, error => error.code === "gemini_canvas_runtime_state_read_timeout" && error.status === 504);
  assert.equal(signal.aborted, true);
});
