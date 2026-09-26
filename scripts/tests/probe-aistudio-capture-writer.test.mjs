import assert from "node:assert/strict";
import test from "node:test";
import { createCaptureWriter } from "../aistudio-live-probe/capture-writer.mjs";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test("capture writer coalesces bursts and never overlaps active writes", async () => {
  const firstWrite = deferred();
  let value = 1;
  let active = 0;
  let maximum = 0;
  const snapshots = [];
  const writer = createCaptureWriter(async () => {
    active++;
    maximum = Math.max(maximum, active);
    snapshots.push(value);
    if (snapshots.length === 1) await firstWrite.promise;
    active--;
  });
  const initial = writer.request();
  await Promise.resolve();
  for (let i = 2; i <= 10000; i++) {
    value = i;
    assert.equal(writer.request(), initial);
  }
  firstWrite.resolve();
  await initial;
  assert.deepEqual(snapshots, [1, 10000]);
  assert.equal(maximum, 1);
  await writer.stop();
});

test("capture writer samples the latest state at execution and restarts after draining", async () => {
  let value = "old";
  const snapshots = [];
  const writer = createCaptureWriter(async () => { snapshots.push(value); });
  const pending = writer.request();
  value = "latest";
  await pending;
  value = "next";
  await writer.request();
  assert.deepEqual(snapshots, ["latest", "next"]);
});

test("capture writer propagates write failure without poisoning future requests", async () => {
  let attempts = 0;
  const failure = new Error("synthetic write failure");
  const writer = createCaptureWriter(async () => {
    if (++attempts === 1) throw failure;
  });
  await assert.rejects(writer.request(), (error) => error === failure);
  await writer.request();
  assert.equal(attempts, 2);
});

test("capture writer stop waits for the active write and prevents late artifact overwrite", async () => {
  const gate = deferred();
  let writes = 0;
  const writer = createCaptureWriter(async () => { writes++; await gate.promise; });
  const first = writer.request();
  await Promise.resolve();
  writer.request();
  let finished = false;
  const stopping = writer.stop().then(() => { finished = true; });
  await writer.request();
  assert.equal(finished, false);
  gate.resolve();
  await Promise.all([first, stopping]);
  await writer.request();
  await writer.stop();
  assert.equal(writes, 1);
});

test("capture writer stop reports an in-flight failure and remains closed", async () => {
  const gate = deferred();
  const writer = createCaptureWriter(() => gate.promise);
  const pending = assert.rejects(writer.request(), /failed/);
  await Promise.resolve();
  const stopped = assert.rejects(writer.stop(), /failed/);
  gate.reject(new Error("failed"));
  await Promise.all([pending, stopped]);
  await writer.request();
});
