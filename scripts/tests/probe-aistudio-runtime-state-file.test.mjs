import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { readRuntimeStateFile } from "../aistudio-live-probe/runtime-state-file.mjs";

async function fixture(run) {
  const directory = await fs.mkdtemp(path.join(tmpdir(), "aistudio-state-file-"));
  try { await run(directory, path.join(directory, "state.json")); } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

test("runtime state file preserves BOM/UTF8 JSON and rejects missing or malformed files safely", async () => {
  await fixture(async (_, file) => {
    await assert.rejects(readRuntimeStateFile(file), {
      code: "aistudio_runtime_state_read_failed", message: "AI Studio runtime state file could not be read.",
    });
    const value = { cookies: [], origins: [], text: "\u4f60\u597d" };
    await fs.writeFile(file, `\ufeff${JSON.stringify(value)}`);
    assert.deepEqual(await readRuntimeStateFile(file), value);
    await fs.writeFile(file, "S3CR3T42");
    await assert.rejects(readRuntimeStateFile(file), {
      code: "aistudio_runtime_state_invalid_json", message: "AI Studio runtime state file must contain valid JSON.",
    });
  });
});

test("runtime state file enforces actual streamed bytes before parsing oversized JSON", async () => {
  await fixture(async (_, file) => {
    await fs.writeFile(file, Buffer.alloc(16 * 1024 * 1024 + 1, 32));
    await assert.rejects(readRuntimeStateFile(file), { code: "aistudio_runtime_state_too_large", status: 413 });
  });
});

test("probe CLI rejects malformed runtime state before launching its configured executable", async () => {
  await fixture(async (directory, file) => {
    await fs.writeFile(file, "S3CR3T42");
    const probe = fileURLToPath(new URL("../probe-aistudio-live-request.mjs", import.meta.url));
    const result = spawnSync(process.execPath, [probe], {
      input: JSON.stringify({ runtimeStateObjectKey: "state.json", captureDir: path.join(directory, "capture"),
        browserExecutablePath: process.execPath, browserProxyUrl: "direct", localWsPort: -1 }),
      encoding: "utf8", timeout: 10000,
      env: { ...process.env, AISTUDIO_PROBE_SUPPRESS_MAIN: "0",
        AI_GATEWAY_OBJECT_STORAGE_DRIVER: "local", AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: directory },
    });
    assert.equal(result.status, 1, result.stderr);
    assert.equal(JSON.parse(result.stdout).error.code, "aistudio_runtime_state_invalid_json");
    assert.equal(result.stdout.includes("S3CR3T42"), false);
    assert.equal(result.stderr.includes("S3CR3T42"), false);
  });
});
