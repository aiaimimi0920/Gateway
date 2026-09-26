import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

const { getStorageRoot, resolveRuntimeStateSource } = await importTestableScript();

test("runtime storage root preserves environment precedence and relative resolution", (t) => {
  const root = storageFixture(t);
  process.env.CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR = path.join(root, "credential");
  process.env.OBJECT_STORAGE_LOCAL_DIR = path.join(root, "generic");
  assert.equal(getStorageRoot(), root);
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR = " ";
  assert.equal(getStorageRoot(), path.join(root, "credential"));
  delete process.env.CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR;
  assert.equal(getStorageRoot(), path.join(root, "generic"));
  process.env.OBJECT_STORAGE_LOCAL_DIR = path.relative(process.cwd(), root);
  assert.equal(getStorageRoot(), root);
  delete process.env.OBJECT_STORAGE_LOCAL_DIR;
  assert.equal(getStorageRoot(), path.resolve(process.cwd(), ".runtime/ai-gateway-objects"));
});

test("local runtime state creates missing relative profiles and accepts JSON without parsing", async (t) => {
  const root = storageFixture(t);
  const profile = await resolveRuntimeStateSource("credential-runtime/missing");
  assert.deepEqual(profile, { mode: "profile_dir", absolutePath: path.join(root, "credential-runtime/missing") });
  assert.equal(fs.statSync(profile.absolutePath).isDirectory(), true);
  const jsonPath = path.join(root, "state.JSON");
  fs.writeFileSync(jsonPath, "deliberately unparsed fixture");
  assert.deepEqual(await resolveRuntimeStateSource(`  ${jsonPath}  `), { mode: "storage_state_file", absolutePath: jsonPath });
  assert.equal(fs.readFileSync(jsonPath, "utf8"), "deliberately unparsed fixture");
});

test("runtime state rejects existing non-JSON files without replacing them", async (t) => {
  const root = storageFixture(t), file = path.join(root, "state.txt");
  fs.writeFileSync(file, "fixture retained");
  await assert.rejects(resolveRuntimeStateSource(file), (error) => error.status === 400 && error.code === "gemini_canvas_runtime_state_invalid_path");
  assert.equal(fs.readFileSync(file, "utf8"), "fixture retained");
});

test("remote configuration errors preserve wrapped error and explicit fixture fallback", async (t) => {
  const root = storageFixture(t);
  process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER = "s3";
  const absent = "remote/absent";
  await assert.rejects(resolveRuntimeStateSource(absent), (error) => error.status === 500
    && error.code === "gemini_canvas_remote_profile_not_supported"
    && error.message.includes("Remote object storage is not fully configured."));
  assert.equal(fs.existsSync(path.join(root, absent)), false);
  assert.deepEqual(await resolveRuntimeStateSource(absent, { allowFixtureEmptyProfile: true }), {
    mode: "profile_dir", absolutePath: path.join(root, absent),
  });
  assert.equal(fs.statSync(path.join(root, absent)).isDirectory(), true);
});
