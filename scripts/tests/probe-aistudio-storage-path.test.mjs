import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { resolveStorageObjectPath } from "../aistudio-live-probe/storage-path.mjs";
import { persistJsonObjectMirror, resolveRuntimeStateSource } from "../aistudio-live-probe/runtime-storage.mjs";

const invalid = { code: "aistudio_runtime_state_invalid_path", status: 400 };

test("storage paths reject traversal, absolute paths and Windows path aliases", () => {
  const root = path.resolve("synthetic-root");
  for (const key of ["../escape", "a/../../escape", "a\\..\\escape", "/absolute", "C:\\absolute",
    "\\\\server\\share", "a//b", "a/./b", "a/file:stream", "a/NUL.json", "COM1", "a/name.",
    "a/name ", "a/\0x", "", "x".repeat(4097), Array(129).fill("a").join("/")]) {
    assert.throws(() => resolveStorageObjectPath(root, key), invalid, key);
  }
  assert.equal(resolveStorageObjectPath(root, "credential-runtime/account/storage-state.json"),
    path.join(root, "credential-runtime", "account", "storage-state.json"));
  assert.equal(resolveStorageObjectPath(root, "credential-runtime\\account\\storage-state.json"),
    path.join(root, "credential-runtime", "account", "storage-state.json"));
});

test("storage path checks reject existing linked descendants", async () => {
  const directory = await fs.mkdtemp(path.join(tmpdir(), "aistudio-storage-path-"));
  try {
    const root = path.join(directory, "root");
    const outside = path.join(directory, "outside");
    await fs.mkdir(root);
    await fs.mkdir(outside);
    await fs.symlink(outside, path.join(root, "linked"), process.platform === "win32" ? "junction" : "dir");
    assert.throws(() => resolveStorageObjectPath(root, "linked/state.json"), invalid);
    assert.deepEqual(await fs.readdir(outside), []);
  } finally { await fs.rm(directory, { recursive: true, force: true }); }
});

test("runtime mirrors use contained atomic files and reject invalid read/write keys before I/O", async () => {
  const directory = await fs.mkdtemp(path.join(tmpdir(), "aistudio-storage-mirror-"));
  const oldRoot = process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR;
  const oldDriver = process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER;
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR = path.join(directory, "root");
  process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER = "local";
  try {
    const key = "credential-runtime/account/storage-state.json";
    const target = await persistJsonObjectMirror(key, { cookies: [], origins: [] });
    assert.deepEqual(await resolveRuntimeStateSource(key), { mode: "storage_state_file", absolutePath: target });
    assert.deepEqual(JSON.parse(await fs.readFile(target, "utf8")), { cookies: [], origins: [] });
    assert.equal(await persistJsonObjectMirror(key, { revision: 2 }), target);
    assert.deepEqual(JSON.parse(await fs.readFile(target, "utf8")), { revision: 2 });
    assert.deepEqual(await fs.readdir(path.dirname(target)), ["storage-state.json"]);
    await assert.rejects(persistJsonObjectMirror("../escape.json", {}), invalid);
    await assert.rejects(resolveRuntimeStateSource("../escape.json"), invalid);
    assert.deepEqual(await fs.readdir(directory), ["root"]);
  } finally {
    if (oldRoot === undefined) delete process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR;
    else process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR = oldRoot;
    if (oldDriver === undefined) delete process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER;
    else process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER = oldDriver;
    await fs.rm(directory, { recursive: true, force: true });
  }
});
