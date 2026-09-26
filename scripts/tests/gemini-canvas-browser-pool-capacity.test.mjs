import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  normalizeMaxContexts,
  removeLaunchProfileClonePath,
  selectContextCapacityVictims,
} from "../gemini-canvas-browser-pool-resources.mjs";

test("browser context capacity normalizes invalid configuration to a safe hard limit", () => {
  assert.equal(normalizeMaxContexts(undefined), 4);
  assert.equal(normalizeMaxContexts("invalid"), 4);
  assert.equal(normalizeMaxContexts(0), 1);
  assert.equal(normalizeMaxContexts(-5), 1);
  assert.equal(normalizeMaxContexts("3.9"), 3);
});

test("browser context capacity counts in-flight creation and rejects an all-busy pool", () => {
  const entries = new Map([
    ["account-a", { busy: true, lastUsedAt: 100 }],
    ["account-b", { busy: true, lastUsedAt: 200 }],
  ]);

  assert.deepEqual(selectContextCapacityVictims(entries, 1, 4), []);
  assert.throws(
    () => selectContextCapacityVictims(entries, 2, 4),
    (error) => error?.code === "gemini_canvas_context_busy" && error?.status === 429,
  );
});

test("browser context capacity selects the oldest idle entry for eviction", () => {
  const entries = new Map([
    ["recent", { busy: false, lastUsedAt: 300 }],
    ["busy", { busy: true, lastUsedAt: 50 }],
    ["oldest-idle", { busy: false, lastUsedAt: 100 }],
  ]);

  assert.deepEqual(selectContextCapacityVictims(entries, 0, 3), ["oldest-idle"]);
});

test("browser context capacity evicts enough idle entries to recover an oversized pool", () => {
  const entries = new Map([
    ["newest", { busy: false, lastUsedAt: 400 }],
    ["oldest", { busy: false, lastUsedAt: 100 }],
    ["middle", { busy: false, lastUsedAt: 200 }],
    ["recent", { busy: false, lastUsedAt: 300 }],
  ]);

  assert.deepEqual(selectContextCapacityVictims(entries, 0, 3), ["oldest", "middle"]);
});

test("cloned launch profile cleanup stays within its managed root", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "gateway-browser-clones-"));
  const cloneRoot = path.join(tempRoot, "clones");
  const clonePath = path.join(cloneRoot, "account-clone");
  const outsidePath = path.join(tempRoot, "outside");
  await fs.mkdir(clonePath, { recursive: true });
  await fs.mkdir(outsidePath, { recursive: true });
  await fs.writeFile(path.join(clonePath, "SingletonLock"), "test", "utf8");

  try {
    assert.equal(await removeLaunchProfileClonePath(clonePath, cloneRoot), true);
    await assert.rejects(fs.access(clonePath));
    assert.equal(await removeLaunchProfileClonePath(cloneRoot, cloneRoot), false);
    assert.equal(await removeLaunchProfileClonePath(outsidePath, cloneRoot), false);
    await fs.access(outsidePath);
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});
