import assert from "node:assert/strict";
import { access, mkdir, mkdtemp, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { cloneBrowserProfile, createFreshBrowserProfile } from "../chatgpt-web-session/profile.mjs";

test("profile names cannot redirect fresh or cloned profile directories", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-profile-safety-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  for (const name of ["", ".", "..", "../outside", "nested/profile", "nested\\profile", "C:\\outside", "name:stream", "CON", "profile.", "profile "]) {
    await assert.rejects(createFreshBrowserProfile(name), { code: "chatgpt_web_profile_invalid" });
    await assert.rejects(cloneBrowserProfile(root, name), { code: "chatgpt_web_profile_invalid" });
  }
});

test("linked profile subtree is rejected and failed clone allocation is removed", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-profile-links-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = path.join(root, "source");
  const outside = path.join(root, "outside");
  await mkdir(path.join(source, "Default"), { recursive: true });
  await mkdir(outside);
  await writeFile(path.join(outside, "private.txt"), "synthetic fixture");
  await symlink(outside, path.join(source, "Default", "Network"), process.platform === "win32" ? "junction" : "dir");
  // Isolate temp allocation from other concurrently running test files.
  const scratch = path.join(root, "scratch");
  await mkdir(scratch);
  const moduleUrl = new URL("../chatgpt-web-session/profile.mjs", import.meta.url).href;
  const child = spawnSync(process.execPath, ["--input-type=module", "-e", `
    import { cloneBrowserProfile } from ${JSON.stringify(moduleUrl)};
    try {
      await cloneBrowserProfile(${JSON.stringify(source)}, "Default");
      process.exitCode = 1;
    } catch (error) {
      process.stdout.write(JSON.stringify({ code: error.code }));
    }
  `], {
    env: { ...process.env, TMP: scratch, TEMP: scratch, TMPDIR: scratch },
    encoding: "utf8", timeout: 10_000,
  });
  assert.equal(child.status, 0, child.stderr);
  assert.deepEqual(JSON.parse(child.stdout), { code: "chatgpt_web_profile_link" });
  assert.deepEqual(await readdir(scratch), []);
  await access(path.join(outside, "private.txt"));
});

test("a linked selected profile is rejected before cloning", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-profile-selected-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const outside = path.join(root, "outside");
  await mkdir(outside);
  await symlink(outside, path.join(root, "Default"), process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(cloneBrowserProfile(root, "Default"), { code: "chatgpt_web_profile_link" });
});
