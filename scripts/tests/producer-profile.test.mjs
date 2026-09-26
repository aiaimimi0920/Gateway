import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import * as fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import * as api from "../producer-browser/profile.mjs";

async function withFixture(run) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "producer-profile-test-"));
  try { await run(root); } finally { await fs.rm(root, { recursive: true, force: true }); }
}

test("Producer profile resolution honors explicit directories and trims profile names", async () => {
  await withFixture(async (root) => {
    await fs.mkdir(path.join(root, "Profile 1"));
    const selected = api.resolveBrowserProfileSource({ browserUserDataDir: root, browserProfileDirectory: " Profile 1 " });
    assert.equal(selected.userDataDir, root);
    assert.equal(selected.profileDirectory, "Profile 1");
    assert.equal(api.resolveBrowserProfileSource({ browserUserDataDir: root, browserProfileDirectory: "missing" }), null);
    assert.equal(api.resolveBrowserProfileSource({ browserUserDataDir: path.join(root, "missing") }), null);
  });
});

test("Producer executable resolution retains explicit existing-file precedence", async () => {
  await withFixture(async (root) => {
    const executable = path.join(root, "fixture-browser.exe");
    await fs.writeFile(executable, "synthetic fixture, never executed");
    assert.equal(api.resolveExecutablePath(` ${executable} `), executable);
  });
});

test("Producer clone copies selected profile state and leaves source/cache untouched", async () => {
  await withFixture(async (root) => {
    const profile = path.join(root, "Default");
    await fs.mkdir(path.join(profile, "Network"), { recursive: true });
    await fs.mkdir(path.join(profile, "Cache"));
    for (const [relative, value] of [["Local State", "local"], ["Default/Preferences", "preferences"], ["Default/Network/Cookies", "fixture cookies"], ["Default/Cache/ignored", "cache"]]) {
      await fs.writeFile(path.join(root, relative), value);
    }
    const clone = await api.cloneBrowserProfile(root, "Default");
    assert.equal(path.dirname(clone), path.resolve(os.tmpdir()));
    assert.ok(path.basename(clone).startsWith("producer-browser-profile-"));
    try {
      assert.equal(await fs.readFile(path.join(clone, "Local State"), "utf8"), "local");
      assert.equal(await fs.readFile(path.join(clone, "Default/Preferences"), "utf8"), "preferences");
      assert.equal(await fs.readFile(path.join(clone, "Default/Network/Cookies"), "utf8"), "fixture cookies");
      assert.equal(existsSync(path.join(clone, "Default/Cache")), false);
      await fs.writeFile(path.join(clone, "Default/Preferences"), "clone changed");
      assert.equal(await fs.readFile(path.join(profile, "Preferences"), "utf8"), "preferences");
    } finally {
      await fs.rm(clone, { recursive: true, force: true });
    }
  });
});
