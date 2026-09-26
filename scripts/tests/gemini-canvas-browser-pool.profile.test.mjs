import assert from "node:assert/strict";
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import path from "node:path";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

const { cleanupClonedLaunchProfile, cloneProfileDirectory, removeManagedLaunchProfileClone } = await importTestableScript();

function sourceProfile(root) {
  const source = path.join(root, "source");
  fs.mkdirSync(path.join(source, "Default"), { recursive: true });
  fs.writeFileSync(path.join(source, "Local State"), "fixture local state");
  fs.writeFileSync(path.join(source, "Default", "Preferences"), "fixture preferences");
  return source;
}

test("profile clone copies nested files without modifying source or following junctions", (t) => {
  const root = storageFixture(t), source = sourceProfile(root);
  const external = path.join(root, "outside");
  fs.mkdirSync(external);
  fs.writeFileSync(path.join(external, "retained.txt"), "outside fixture");
  fs.symlinkSync(external, path.join(source, "linked"), "junction");
  const clone = cloneProfileDirectory(source);
  assert.equal(path.dirname(clone), path.join(root, "credential-runtime/gemini-canvas-browser-launch-clones"));
  assert.match(path.basename(clone), /^[a-f0-9]{12}-\d+$/);
  assert.equal(fs.readFileSync(path.join(clone, "Default", "Preferences"), "utf8"), "fixture preferences");
  assert.equal(fs.readFileSync(path.join(clone, "Local State"), "utf8"), "fixture local state");
  assert.equal(fs.existsSync(path.join(clone, "linked")), false);
  fs.writeFileSync(path.join(clone, "Default", "Preferences"), "changed clone");
  assert.equal(fs.readFileSync(path.join(source, "Default", "Preferences"), "utf8"), "fixture preferences");
  assert.equal(fs.readFileSync(path.join(external, "retained.txt"), "utf8"), "outside fixture");
});

test("profile clone skips a locked file and still copies the remaining entries", (t) => {
  const root = storageFixture(t), source = sourceProfile(root), messages = [];
  fs.writeFileSync(path.join(source, "locked.json"), "fixture locked entry");
  const copy = fs.copyFileSync;
  t.mock.method(fs, "copyFileSync", (from, to) => {
    if (path.basename(from) === "locked.json") throw Object.assign(new Error("fixture lock"), { code: "EBUSY" });
    return copy(from, to);
  });
  t.mock.method(console, "log", (...parts) => messages.push(parts));
  syncBuiltinESMExports();
  t.after(() => { t.mock.restoreAll(); syncBuiltinESMExports(); });
  const clone = cloneProfileDirectory(source);
  assert.equal(fs.existsSync(path.join(clone, "locked.json")), false);
  assert.equal(fs.readFileSync(path.join(clone, "Default", "Preferences"), "utf8"), "fixture preferences");
  assert.equal(fs.readFileSync(path.join(source, "locked.json"), "utf8"), "fixture locked entry");
  assert.equal(messages.filter((parts) => parts[1] === "skipped locked profile entry during clone").length, 1);
});

test("profile cleanup clears ownership before awaiting removal and remains idempotent", async (t) => {
  const root = storageFixture(t), source = sourceProfile(root);
  const clone = cloneProfileDirectory(source);
  const entry = { launchClonedProfile: true, launchRuntimePath: clone };
  const pending = cleanupClonedLaunchProfile(entry);
  assert.equal(entry.launchClonedProfile, false);
  await pending;
  assert.equal(fs.existsSync(clone), false);
  assert.equal(fs.existsSync(source), true);
  await cleanupClonedLaunchProfile(entry);
  await cleanupClonedLaunchProfile(null);
  assert.equal(fs.readFileSync(path.join(source, "Local State"), "utf8"), "fixture local state");
});

test("profile cleanup refuses its root and outside paths while preserving the ownership flag transition", async (t) => {
  const root = storageFixture(t), source = sourceProfile(root);
  const clone = cloneProfileDirectory(source), cloneRoot = path.dirname(clone);
  assert.equal(await removeManagedLaunchProfileClone(cloneRoot), false);
  assert.equal(await removeManagedLaunchProfileClone(source), false);
  assert.equal(await removeManagedLaunchProfileClone(""), false);
  const entry = { launchClonedProfile: true, launchRuntimePath: source };
  await cleanupClonedLaunchProfile(entry);
  assert.equal(entry.launchClonedProfile, false);
  assert.equal(fs.existsSync(source), true);
  assert.equal(fs.existsSync(cloneRoot), true);
  assert.equal(await removeManagedLaunchProfileClone(clone), true);
  assert.equal(fs.existsSync(clone), false);
});

test("profile clone propagates missing-source errors without inventing profile contents", (t) => {
  const root = storageFixture(t), missing = path.join(root, "missing");
  assert.throws(() => cloneProfileDirectory(missing), (error) => error.code === "ENOENT");
  assert.equal(fs.existsSync(missing), false);
  const cloneRoot = path.join(root, "credential-runtime/gemini-canvas-browser-launch-clones");
  assert.deepEqual(fs.readdirSync(cloneRoot), []);
});
