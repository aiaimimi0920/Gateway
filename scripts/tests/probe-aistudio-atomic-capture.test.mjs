import assert from "node:assert/strict";
import fs from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test, { mock } from "node:test";
import { writeAtomicCapture } from "../aistudio-live-probe/atomic-capture-file.mjs";

async function fixture(run) {
  const directory = await fs.mkdtemp(path.join(tmpdir(), "aistudio-atomic-"));
  try { await run(directory, path.join(directory, "capture.json")); } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

test("capture publication replaces complete UTF8 snapshots without leftover temporary files", async () => {
  await fixture(async (directory, destination) => {
    await writeAtomicCapture(destination, '{"revision":1}\n');
    await writeAtomicCapture(destination, '{"revision":2,"text":"\u4f60\u597d"}\n');
    const bytes = await fs.readFile(destination);
    assert.deepEqual(JSON.parse(bytes.toString("utf8")), { revision: 2, text: "\u4f60\u597d" });
    assert.notDeepEqual([...bytes.subarray(0, 3)], [239, 187, 191]);
    assert.deepEqual(await fs.readdir(directory), ["capture.json"]);
  });
});

test("partial temporary write failure preserves the published snapshot and closes its handle", async () => {
  await fixture(async (directory, destination) => {
    await fs.writeFile(destination, "old snapshot");
    const open = fs.open.bind(fs);
    let closed = false;
    const injected = mock.method(fs, "open", async (...args) => {
      const real = await open(...args);
      return {
        writeFile: async () => { await real.writeFile("partial"); throw new Error("synthetic write failure"); },
        sync: () => real.sync(),
        close: async () => { await real.close(); closed = true; },
      };
    });
    try {
      await assert.rejects(writeAtomicCapture(destination, "new snapshot"), /synthetic write failure/);
      assert.equal(closed, true);
      assert.equal(await fs.readFile(destination, "utf8"), "old snapshot");
      assert.deepEqual(await fs.readdir(directory), ["capture.json"]);
    } finally { injected.mock.restore(); }
  });
});

test("failed atomic replacement preserves an existing destination directory", async () => {
  await fixture(async (directory, destination) => {
    await fs.mkdir(destination);
    await fs.writeFile(path.join(destination, "sentinel"), "preserve");
    await assert.rejects(writeAtomicCapture(destination, "new snapshot"));
    assert.equal(await fs.readFile(path.join(destination, "sentinel"), "utf8"), "preserve");
    assert.deepEqual(await fs.readdir(directory), ["capture.json"]);
  });
});

test("atomic writer does not unlink a temporary path it failed to create", async () => {
  await fixture(async (directory, destination) => {
    const injected = mock.method(fs, "open", async (temporary) => {
      await fs.writeFile(temporary, "foreign file");
      throw Object.assign(new Error("collision"), { code: "EEXIST" });
    });
    try {
      await assert.rejects(writeAtomicCapture(destination, "new"), { code: "EEXIST" });
      const files = await fs.readdir(directory);
      assert.equal(files.length, 1);
      assert.equal(await fs.readFile(path.join(directory, files[0]), "utf8"), "foreign file");
    } finally { injected.mock.restore(); }
  });
});
