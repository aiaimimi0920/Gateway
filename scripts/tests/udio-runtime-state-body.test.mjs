import assert from "node:assert/strict";
import test from "node:test";
import { Readable } from "node:stream";
import { readRuntimeStateBody } from "../udio-browser/state-body.mjs";

test("runtime state accepts exact byte limit across chunks and multibyte strings", async () => {
  const body = Readable.from([Buffer.from("ab"), "\u00e9"]);
  assert.equal((await readRuntimeStateBody(body, 4)).toString("utf8"), "ab\u00e9");
  assert.equal((await readRuntimeStateBody(null, 0)).length, 0);
});

test("runtime overflow closes the iterator and does not consume later chunks", async () => {
  let closed = false;
  let reads = 0;
  async function* body() {
    try {
      reads += 1;
      yield Buffer.alloc(4);
      reads += 1;
      yield Buffer.alloc(1);
      reads += 1;
      yield Buffer.alloc(1);
    } finally {
      closed = true;
    }
  }
  await assert.rejects(readRuntimeStateBody(body(), 4),
    (error) => error.code === "udio_runtime_state_too_large");
  assert.equal(reads, 2);
  assert.equal(closed, true);
});

test("SDK streaming takes precedence over unbounded conversion", async () => {
  const body = Readable.from([Buffer.from("{}")]);
  body.transformToByteArray = () => { throw new Error("Unbounded conversion invoked"); };
  assert.equal((await readRuntimeStateBody(body, 2)).toString(), "{}");
  await assert.rejects(readRuntimeStateBody({
    transformToByteArray: () => { throw new Error("Unbounded conversion invoked"); },
  }), /must support bounded streaming/);
});

test("direct buffers and typed arrays enforce the same byte limit", async () => {
  assert.deepEqual(await readRuntimeStateBody(new Uint8Array([1, 2]), 2), Buffer.from([1, 2]));
  for (const body of [Buffer.alloc(3), new Uint8Array(3)]) {
    await assert.rejects(readRuntimeStateBody(body, 2),
      (error) => error.code === "udio_runtime_state_too_large");
  }
  await assert.rejects(readRuntimeStateBody(Buffer.alloc(0), -1), RangeError);
});

test("tiny and empty chunks retain exact bytes across fixed-size blocks", async () => {
  async function* body() {
    for (let index = 0; index < 65538; index += 1) {
      yield Buffer.alloc(0);
      yield Buffer.from([index % 251]);
    }
  }
  const result = await readRuntimeStateBody(body(), 65538);
  assert.equal(result.length, 65538);
  for (let index = 0; index < result.length; index += 1) {
    assert.equal(result[index], index % 251);
  }
});

test("unsupported chunk types fail closed and release the iterator", async () => {
  let closed = false;
  async function* body() {
    try { yield {}; } finally { closed = true; }
  }
  await assert.rejects(readRuntimeStateBody(body()), /non-byte chunk/);
  assert.equal(closed, true);
});
