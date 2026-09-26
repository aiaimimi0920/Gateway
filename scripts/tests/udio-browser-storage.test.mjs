import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, readFile, writeFile, rm, symlink, readdir, link, open } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { maybeReadRuntimeState, maybePersistRuntimeState } from "../udio-browser/storage.mjs";
import { readLocalState, writeLocalState } from "../udio-browser/local-state.mjs";

const state = { cookies: [{ name: "sb-ssr-production-auth-token", value: "synthetic", domain: ".udio.com" }], origins: [] };
async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), "udio-storage-boundary-"));
  const storage = path.join(root, "objects");
  await mkdir(storage);
  const env = {
    AI_GATEWAY_OBJECT_STORAGE_DRIVER: "local",
    AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: storage,
  };
  const previous = Object.fromEntries(Object.keys(env).map((key) => [key, process.env[key]]));
  Object.assign(process.env, env);
  t.after(async () => {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    await rm(root, { recursive: true, force: true });
  });
  return { root, storage };
}

test("runtime read refuses parent traversal without reading the outside state", async (t) => {
  const { root } = await fixture(t);
  await writeFile(path.join(root, "outside.json"), JSON.stringify(state));
  assert.equal(await maybeReadRuntimeState("../outside.json"), null);
});

test("runtime write refuses traversal and leaves existing outside data intact", async (t) => {
  const { root } = await fixture(t);
  const outside = path.join(root, "outside.json");
  await writeFile(outside, "sentinel");
  await assert.rejects(maybePersistRuntimeState({ storageState: async () => state }, "../outside.json"),
    (error) => error.code === "udio_invalid_local_object_key");
  assert.equal(await readFile(outside, "utf8"), "sentinel");
});

test("local object keys reject ambiguous path segments before touching files", async (t) => {
  await fixture(t);
  for (const key of ["/absolute", "a/../b", "./state", "a//b", "a\\b", "C:/state",
    "a/stream:secret", "a/trailing.", "a/ spaced /state", "a/CON.json", "a/\u0000state"]) {
    await assert.rejects(maybePersistRuntimeState({ storageState: async () => state }, key),
      (error) => error.code === "udio_invalid_local_object_key");
  }
});

test("nested junction cannot redirect runtime state reads or writes outside root", async (t) => {
  const { root, storage } = await fixture(t);
  const outside = path.join(root, "outside");
  await mkdir(outside);
  await writeFile(path.join(outside, "state.json"), JSON.stringify(state));
  await symlink(outside, path.join(storage, "link"), process.platform === "win32" ? "junction" : "dir");
  assert.equal(await maybeReadRuntimeState("link/state.json"), null);
  await assert.rejects(maybePersistRuntimeState({ storageState: async () => state }, "link/new.json"),
    (error) => error.code === "udio_local_object_link");
  assert.deepEqual(await readdir(outside), ["state.json"]);
});

test("valid nested state round trips and leaves no temporary files", async (t) => {
  const { storage } = await fixture(t);
  const key = "credential-runtime/udio/account/storage-state.json";
  await maybePersistRuntimeState({ storageState: async () => state }, key);
  assert.deepEqual(await maybeReadRuntimeState(key), state);
  const next = { ...state, origins: [{ origin: "https://www.udio.com", localStorage: [] }] };
  await maybePersistRuntimeState({ storageState: async () => next }, key, state);
  assert.deepEqual(await maybeReadRuntimeState(key), next);
  assert.deepEqual(await readdir(path.dirname(path.join(storage, key))), ["storage-state.json"]);
});

test("atomic replacement does not overwrite a hard-linked outside file", async (t) => {
  const { root, storage } = await fixture(t);
  const outside = path.join(root, "outside.json");
  await writeFile(outside, "sentinel");
  await link(outside, path.join(storage, "state.json"));
  await maybePersistRuntimeState({ storageState: async () => state }, "state.json");
  assert.equal(await readFile(outside, "utf8"), "sentinel");
  assert.deepEqual(await maybeReadRuntimeState("state.json"), state);
  assert.deepEqual(await readdir(storage), ["state.json"]);
});

test("directory targets are refused without leaving temporary files", async (t) => {
  const { storage } = await fixture(t);
  await mkdir(path.join(storage, "state.json"));
  await assert.rejects(maybePersistRuntimeState({ storageState: async () => state }, "state.json"),
    (error) => error.code === "udio_local_object_type");
  assert.deepEqual(await readdir(storage), ["state.json"]);
});

test("oversized local state is rejected before whole-file buffering", async (t) => {
  const { storage } = await fixture(t);
  const file = await open(path.join(storage, "large.json"), "w");
  try {
    await file.truncate(16 * 1024 * 1024 + 1);
  } finally {
    await file.close();
  }
  await assert.rejects(readLocalState(storage, "large.json"),
    (error) => error.code === "udio_runtime_state_too_large");
  assert.equal(await maybeReadRuntimeState("large.json"), null);
});

test("oversized replacement preserves old local state without temporary artifacts", async (t) => {
  const { storage } = await fixture(t);
  await writeFile(path.join(storage, "state.json"), "sentinel");
  await assert.rejects(writeLocalState(storage, "state.json", Buffer.alloc(16 * 1024 * 1024 + 1)),
    (error) => error.code === "udio_runtime_state_too_large");
  assert.equal(await readFile(path.join(storage, "state.json"), "utf8"), "sentinel");
  assert.deepEqual(await readdir(storage), ["state.json"]);
});

test("shared context persistence excludes unrelated site credentials", async (t) => {
  await fixture(t);
  const shared = {
    cookies: [...state.cookies,
      { name: "other-session", value: "unrelated-secret", domain: "unrelated.test" },
      { name: "lookalike", value: "unrelated-secret", domain: "eviludio.com" }],
    origins: [
      { origin: "https://www.udio.com", localStorage: [{ name: "auth", value: "synthetic" }] },
      { origin: "https://unrelated.test", localStorage: [{ name: "auth", value: "unrelated-secret" }] },
    ],
  };
  await maybePersistRuntimeState({ storageState: async () => shared }, "scoped.json", null, "https://www.udio.com");
  const persisted = await maybeReadRuntimeState("scoped.json");
  assert.deepEqual(persisted, { cookies: state.cookies, origins: [shared.origins[0]] });
  assert.equal(shared.cookies.length, 3);
  assert.equal(shared.origins.length, 2);
});

test("unrelated auth cannot replace a previously authenticated provider state", async (t) => {
  await fixture(t);
  await maybePersistRuntimeState({ storageState: async () => state }, "provider.json");
  const foreign = { cookies: [{ ...state.cookies[0], domain: "unrelated.test" }], origins: [] };
  await maybePersistRuntimeState({ storageState: async () => foreign }, "provider.json", state);
  assert.deepEqual(await maybeReadRuntimeState("provider.json"), state);
});
