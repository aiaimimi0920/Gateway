import assert from "node:assert/strict";
import test, { mock } from "node:test";
import { createPendingBufferPool, MAX_PENDING_BYTES } from "../aistudio-live-probe/pending-buffer.mjs";

test("pending buffer preserves partial frame tails and exposes only initialized bytes", () => {
  const pool = createPendingBufferPool();
  const buffer = pool.createBuffer();
  buffer.append(Buffer.from("first-tail"));
  buffer.consume(6);
  buffer.append(Buffer.from("-next"));
  assert.equal(buffer.bytes().toString(), "tail-next");
  assert.equal(buffer.length, 9);
  assert.throws(() => buffer.consume(10), /Invalid/);
  buffer.consume(9);
  assert.equal(pool.allocatedBytes, 0);
  assert.equal(buffer.length, 0);
  buffer.release();
  assert.equal(pool.allocatedBytes, 0);
});

test("pending buffer grows geometrically for one-byte fragments", () => {
  const pool = createPendingBufferPool();
  const buffer = pool.createBuffer();
  const original = Buffer.allocUnsafe;
  let allocations = 0;
  const allocation = mock.method(Buffer, "allocUnsafe", (size) => {
    allocations++;
    return original(size);
  });
  try {
    const chunk = Buffer.from([42]);
    for (let i = 0; i < 100000; i++) buffer.append(chunk);
    assert.ok(allocations <= 4, `allocations=${allocations}`);
    assert.equal(buffer.length, 100000);
    assert.ok(buffer.bytes().every((byte) => byte === 42));
  } finally {
    allocation.mock.restore();
    buffer.release();
  }
});

test("pending buffer caps per-connection bytes before mutation", () => {
  const pool = createPendingBufferPool();
  const buffer = pool.createBuffer();
  buffer.append(Buffer.alloc(MAX_PENDING_BYTES, 1));
  assert.throws(() => buffer.append(Buffer.from([2])), /capture buffer limit/);
  assert.equal(buffer.length, MAX_PENDING_BYTES);
  buffer.release();
  assert.equal(pool.allocatedBytes, 0);
});

test("pending pool shares capacity across connections and returns released reservations", () => {
  const pool = createPendingBufferPool();
  const first = pool.createBuffer();
  const second = pool.createBuffer();
  const third = pool.createBuffer();
  const chunk = Buffer.alloc(16 * 1024 * 1024);
  first.append(chunk);
  second.append(chunk);
  assert.equal(pool.allocatedBytes, 32 * 1024 * 1024);
  assert.throws(() => third.append(Buffer.from([1])), /aggregate pending capacity/);
  assert.equal(third.length, 0);
  first.release();
  third.append(Buffer.from([1]));
  assert.equal(third.length, 1);
  second.release();
  third.release();
  assert.equal(pool.allocatedBytes, 0);
});

test("pending pool accounts for old capacity during growth and rolls back allocation failures", () => {
  const pool = createPendingBufferPool();
  const first = pool.createBuffer();
  const second = pool.createBuffer();
  first.append(Buffer.alloc(8 * 1024 * 1024));
  second.append(Buffer.alloc(16 * 1024 * 1024));
  assert.throws(() => first.append(Buffer.from([1])), /aggregate pending capacity/);
  assert.equal(pool.allocatedBytes, 24 * 1024 * 1024);
  second.release();
  const allocation = mock.method(Buffer, "allocUnsafe", () => { throw new Error("allocation failed"); });
  try {
    assert.throws(() => first.append(Buffer.from([1])), /allocation failed/);
    assert.equal(pool.allocatedBytes, 8 * 1024 * 1024);
    assert.equal(first.length, 8 * 1024 * 1024);
  } finally {
    allocation.mock.restore();
    first.release();
  }
});
