import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import path from "node:path";
import { runtimeExecutable } from "../aistudio-http/runtime-path.mjs";

const scratch = "C:/Users/Public/nas_home/AI/GameEditor/linshi";
test("bundled AI Studio proof resolves independently of the current directory", async () => {
  const root = await mkdtemp(path.join(scratch, "aistudio-runtime-test-"));
  try {
    await writeFile(path.join(root, "aistudio-proof.exe"), "fixture");
    assert.equal(runtimeExecutable("aistudio-proof.exe", undefined, "missing", { root }), path.join(root, "aistudio-proof.exe"));
    assert.throws(() => runtimeExecutable("aistudio-proof.exe", "relative.exe", "missing", { root }), /missing/);
    assert.throws(() => runtimeExecutable("aistudio-proof.exe", path.join(root, "absent.exe"), "missing", { root }), /missing/);
  } finally { await rm(root, { recursive: true }); }
});

test("ffmpeg discovery uses absolute trusted PATH entries, never cwd or directories", async () => {
  const root = await mkdtemp(path.join(scratch, "aistudio-encoder-test-"));
  try {
    const host = path.join(root, "host"); await mkdir(host);
    await writeFile(path.join(host, "ffmpeg.exe"), "fixture");
    const options = { root, searchPath: `relative${path.delimiter}${host}` };
    assert.equal(runtimeExecutable("ffmpeg.exe", undefined, "missing", options), path.join(host, "ffmpeg.exe"));
    assert.throws(() => runtimeExecutable("ffmpeg.exe", "", "missing", options), /missing/);
    assert.throws(() => runtimeExecutable("ffmpeg.exe", host, "missing", options), /missing/);
  } finally { await rm(root, { recursive: true }); }
});
