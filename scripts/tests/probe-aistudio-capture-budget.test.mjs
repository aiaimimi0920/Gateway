import assert from "node:assert/strict";
import test from "node:test";
import { createCaptureBudget } from "../aistudio-live-probe/capture-budget.mjs";

const budgetError = { code: "aistudio_probe_capture_budget_exceeded", status: 413 };

test("capture budget admits 4096 records across arrays without resetting on another stream", () => {
  const append = createCaptureBudget();
  const first = [];
  const second = [];
  for (let i = 0; i < 4096; i++) append(i % 2 ? first : second, "");
  assert.equal(first.length + second.length, 4096);
  assert.throws(() => append([], ""), budgetError);
});

test("capture budget checks encoded escaping and UTF8 bytes rather than string characters", () => {
  const append = createCaptureBudget();
  const entries = [];
  assert.throws(() => append(entries, "\0".repeat(180000)), budgetError);
  assert.throws(() => append(entries, "\u4f60".repeat(400000)), budgetError);
  assert.deepEqual(entries, []);
  append(entries, "valid");
  assert.deepEqual(entries, ["valid"]);
});

test("capture budget stops aggregate retained records above 16 MiB", () => {
  const append = createCaptureBudget();
  const entries = [];
  const record = "x".repeat(1024 * 1024 - 4);
  for (let i = 0; i < 16; i++) append(entries, record);
  assert.equal(entries.length, 16);
  assert.throws(() => append(entries, ""), budgetError);
  assert.equal(entries.length, 16);
});

test("capture budget rejects deeply nested, cyclic and excessively broad records", () => {
  const append = createCaptureBudget();
  const cycle = {};
  cycle.self = cycle;
  const broad = Object.fromEntries(Array.from({ length: 2048 }, (_, i) => [i, null]));
  let nested = "leaf";
  for (let i = 0; i < 10; i++) nested = { nested };
  for (const record of [cycle, broad, nested]) assert.throws(() => append([], record), budgetError);
});

test("capture budget charges nested mutations to their retained owner", () => {
  const append = createCaptureBudget();
  const owner = { frames: [] };
  append([], owner);
  append(owner.frames, "x".repeat(600000), owner);
  assert.throws(() => append(owner.frames, "y".repeat(600000), owner), budgetError);
  assert.equal(owner.frames.length, 1);
  append(owner.frames, "small", owner);
  assert.equal(owner.frames.length, 2);
});
