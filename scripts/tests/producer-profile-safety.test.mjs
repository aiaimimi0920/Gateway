import assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test, { mock } from "node:test";
import { cloneBrowserProfile, resolveBrowserProfileSource } from "../producer-browser/profile.mjs";

async function withFixture(run) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "producer-profile-safety-"));
  const source = path.join(root, "source");
  await fs.mkdir(path.join(source, "Default"), { recursive: true });
  const temp = mock.method(os, "tmpdir", () => root);
  try { await run({ root, source }); } finally {
    temp.mock.restore();
    await fs.rm(root, { recursive: true, force: true });
  }
}

test("Producer profile selection rejects traversal and Windows special names", async () => {
  await withFixture(async ({ source }) => {
    for (const name of ["..", ".", "../private", "Default/child", "Default\\child", "C:\\outside", "Default:stream", "NUL", "COM1.txt", "trailing."]) {
      assert.throws(() => resolveBrowserProfileSource({ browserUserDataDir: source, browserProfileDirectory: name }), { code: "producer_browser_profile_invalid" }, name);
    }
  });
});

test("Producer clone rejects linked profile descendants rather than copying outside data", async () => {
  await withFixture(async ({ root, source }) => {
    const privateDir = path.join(root, "private");
    await fs.mkdir(privateDir);
    await fs.writeFile(path.join(privateDir, "private.txt"), "synthetic private fixture");
    await fs.symlink(privateDir, path.join(source, "Default/Network"), process.platform === "win32" ? "junction" : "dir");
    await assert.rejects(cloneBrowserProfile(source, "Default"), { code: "producer_browser_profile_link" });
    assert.equal((await fs.readdir(root)).some((name) => name.startsWith("producer-browser-profile-")), false);
  });
});

test("Producer clone failure removes its owned temporary root", async () => {
  await withFixture(async ({ root, source }) => {
    await fs.mkdir(path.join(source, "Default/Preferences"));
    await assert.rejects(cloneBrowserProfile(source, "Default"));
    assert.equal((await fs.readdir(root)).some((name) => name.startsWith("producer-browser-profile-")), false);
  });
});

test("Producer clone validates its direct caller before creating an output root", async () => {
  await withFixture(async ({ root, source }) => {
    for (const name of ["..", "../outside", "Default:stream", "NUL", "Default "]) {
      await assert.rejects(cloneBrowserProfile(source, name), { code: "producer_browser_profile_invalid" });
    }
    assert.equal((await fs.readdir(root)).some((name) => name.startsWith("producer-browser-profile-")), false);
  });
});

test("Producer permits the explicitly selected root to be a NAS-style directory link", async () => {
  await withFixture(async ({ root, source }) => {
    await fs.writeFile(path.join(source, "Default/Preferences"), "fixture preferences");
    const alias = path.join(root, "operator-root");
    await fs.symlink(source, alias, process.platform === "win32" ? "junction" : "dir");
    const selected = resolveBrowserProfileSource({ browserUserDataDir: alias });
    assert.equal(selected.userDataDir, await fs.realpath(source));
    const clone = await cloneBrowserProfile(alias, "Default");
    assert.equal(await fs.readFile(path.join(clone, "Default/Preferences"), "utf8"), "fixture preferences");
  });
});

for (const relative of ["LinkedProfile", "Default/Network/nested"]) {
  test(`Producer rejects a profile link at ${relative}`, async () => {
    await withFixture(async ({ root, source }) => {
      const privateDir = path.join(root, "private");
      await fs.mkdir(privateDir);
      await fs.writeFile(path.join(privateDir, "private.txt"), "synthetic fixture");
      const link = path.join(source, relative);
      await fs.mkdir(path.dirname(link), { recursive: true });
      await fs.symlink(privateDir, link, process.platform === "win32" ? "junction" : "dir");
      const profile = relative === "LinkedProfile" ? relative : "Default";
      if (profile === relative) {
        assert.throws(() => resolveBrowserProfileSource({ browserUserDataDir: source, browserProfileDirectory: profile }), { code: "producer_browser_profile_link" });
      }
      await assert.rejects(cloneBrowserProfile(source, profile), { code: "producer_browser_profile_link" });
      assert.equal((await fs.readdir(root)).some((name) => name.startsWith("producer-browser-profile-")), false);
    });
  });
}
