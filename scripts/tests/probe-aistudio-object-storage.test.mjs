import assert from "node:assert/strict";
import { PassThrough, Readable } from "node:stream";
import fs from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test, { mock } from "node:test";
import { S3Client } from "@aws-sdk/client-s3";
import { readRuntimeStateBody, withObjectStorageRequest, MAX_RUNTIME_STATE_BYTES } from "../aistudio-live-probe/object-storage-request.mjs";
import { resolveRuntimeStateSource, persistJsonObjectMirror, closeObjectStorageClient } from "../aistudio-live-probe/runtime-storage.mjs";

test("object storage body accepts split bytes, empty input and exact limit", async () => {
  const signal = new AbortController().signal;
  assert.equal((await readRuntimeStateBody(null, signal)).length, 0);
  const source = Buffer.from("split-\u4f60\u597d");
  const body = Readable.from([...source].map((byte) => Buffer.from([byte])));
  assert.deepEqual(await readRuntimeStateBody(body, signal), source);
  const exact = Buffer.alloc(MAX_RUNTIME_STATE_BYTES, 1);
  assert.equal(await readRuntimeStateBody(exact, signal), exact);
});

test("object storage rejects declared and streamed overflow and destroys the body", async () => {
  const signal = new AbortController().signal;
  const declared = new PassThrough();
  await assert.rejects(readRuntimeStateBody(declared, signal, MAX_RUNTIME_STATE_BYTES + 1), { status: 413 });
  assert.equal(declared.destroyed, true);
  const streamed = Readable.from([Buffer.alloc(MAX_RUNTIME_STATE_BYTES), Buffer.from([1])]);
  await assert.rejects(readRuntimeStateBody(streamed, signal), { code: "aistudio_runtime_state_too_large" });
  assert.equal(streamed.destroyed, true);
});

test("small object storage bodies do not allocate the full 16 MiB admission budget", async () => {
  const body = Readable.from([Buffer.from("small")]);
  const original = Buffer.allocUnsafe;
  const allocations = [];
  const allocation = mock.method(Buffer, "allocUnsafe", (size) => {
    allocations.push(size);
    return original(size);
  });
  try {
    assert.equal((await readRuntimeStateBody(body, new AbortController().signal)).toString(), "small");
    assert.ok(allocations.length > 0);
    assert.ok(allocations.every((size) => size <= 65536));
  } finally { allocation.mock.restore(); }
});

test("object storage deadline aborts a stalled body and clears its timer", async () => {
  let expire;
  let cleared = false;
  const timer = mock.method(globalThis, "setTimeout", (callback, ms) => {
    assert.equal(ms, 30000);
    expire = callback;
    return 41;
  });
  const cleanup = mock.method(globalThis, "clearTimeout", (token) => {
    assert.equal(token, 41);
    cleared = true;
  });
  const body = new PassThrough();
  try {
    const pending = withObjectStorageRequest((signal) => readRuntimeStateBody(body, signal));
    const rejected = assert.rejects(pending, { code: "aistudio_object_storage_timeout", status: 504 });
    await Promise.resolve();
    expire();
    await rejected;
    assert.equal(body.destroyed, true);
    assert.equal(cleared, true);
  } finally {
    timer.mock.restore();
    cleanup.mock.restore();
    body.destroy();
  }
});

test("object storage rejects unbounded body adapters and redacts upstream diagnostics", async () => {
  let called = false;
  await assert.rejects(readRuntimeStateBody({ transformToByteArray: () => { called = true; } },
    new AbortController().signal), /cancellable Node readable/);
  assert.equal(called, false);
  await assert.rejects(withObjectStorageRequest(async () => { throw new Error("synthetic-secret"); }), {
    code: "aistudio_object_storage_failed", message: "AI Studio object storage request failed.", status: 502,
  });
});

test("S3 mirror integration passes abort signals and releases its cached client", async () => {
  const directory = await fs.mkdtemp(path.join(tmpdir(), "aistudio-s3-mock-"));
  const settings = {
    AI_GATEWAY_OBJECT_STORAGE_DRIVER: "s3-compatible", AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: directory,
    AI_GATEWAY_OBJECT_STORAGE_BUCKET: "synthetic", AI_GATEWAY_OBJECT_STORAGE_ENDPOINT: "http://127.0.0.1:1",
    AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID: "synthetic", AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY: "synthetic",
  };
  const previous = Object.fromEntries(Object.keys(settings).map((key) => [key, process.env[key]]));
  Object.assign(process.env, settings);
  const calls = [];
  const send = mock.method(S3Client.prototype, "send", async (command, options) => {
    assert.ok(options.abortSignal instanceof AbortSignal);
    assert.equal(options.abortSignal.aborted, false);
    calls.push(command.constructor.name);
    return { Body: Readable.from([Buffer.from('{"cookies":[],"origins":[]}')]) };
  });
  let destroyed = 0;
  const originalDestroy = S3Client.prototype.destroy;
  const destroy = mock.method(S3Client.prototype, "destroy", function () {
    destroyed++;
    return originalDestroy.call(this);
  });
  try {
    const key = "credential-runtime/account/storage-state.json";
    const result = await resolveRuntimeStateSource(key);
    assert.equal(result.mode, "storage_state_file");
    assert.deepEqual(JSON.parse(await fs.readFile(result.absolutePath, "utf8")), { cookies: [], origins: [] });
    await persistJsonObjectMirror("credential-runtime/account/contract.json", { contract: true });
    assert.deepEqual(calls, ["GetObjectCommand", "PutObjectCommand"]);
    closeObjectStorageClient();
    closeObjectStorageClient();
    assert.equal(destroyed, 1);
  } finally {
    closeObjectStorageClient();
    send.mock.restore();
    destroy.mock.restore();
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    await fs.rm(directory, { recursive: true, force: true });
  }
});
