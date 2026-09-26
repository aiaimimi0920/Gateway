import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile, access } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  normalizeAuthSeed,
  normalizeString,
  normalizeTimeoutMs,
  parseBoolean,
} from "../chatgpt-web-session/configuration.mjs";
import { createWorkerError, serializeError } from "../chatgpt-web-session/errors.mjs";
import {
  cloneBrowserProfile,
  createFreshBrowserProfile,
  resolveExecutablePath,
  resolveProfileSource,
} from "../chatgpt-web-session/profile.mjs";

test("configuration preserves normalization and timeout boundaries", () => {
  assert.equal(normalizeString("  seed  "), "seed");
  assert.equal(normalizeString(42), null);
  assert.equal(normalizeTimeoutMs(0), 120_000);
  assert.equal(normalizeTimeoutMs(Infinity), 120_000);
  assert.equal(normalizeTimeoutMs(900_000), 600_000);
  assert.equal(normalizeTimeoutMs(13), 13);
  assert.equal(parseBoolean(" YES ", false), true);
  assert.equal(parseBoolean("off", true), false);
  assert.equal(parseBoolean("unknown", true), true);
});

test("auth seed aliases and structured errors retain their wire shapes", () => {
  assert.deepEqual(normalizeAuthSeed({ login_email: " a@example.test ", password_hash: "hash" }), {
    email: "a@example.test", password: null, passwordSha256: "hash",
  });
  assert.equal(normalizeAuthSeed({ email: "a@example.test" }), null);
  assert.deepEqual(serializeError(createWorkerError(409, "profile_busy", "Busy")), {
    status: 409, code: "profile_busy", message: "Busy",
  });
  assert.equal(serializeError(null).status, 500);
});

test("configured executable and profile take precedence over discovery", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-chatgpt-contract-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const executable = path.join(root, "browser.exe");
  await writeFile(executable, "fixture");
  assert.equal(resolveExecutablePath(executable), executable);
  assert.deepEqual(resolveProfileSource({ userDataDir: root, profileDirectory: "Profile 2" }, null), {
    mode: "clone", browser: "chromium", userDataDir: root, profileDirectory: "Profile 2",
  });
});

test("fresh profile creates the requested directory without browser launch", async (t) => {
  const root = await createFreshBrowserProfile("Profile 3");
  t.after(() => rm(root, { recursive: true, force: true }));
  await access(path.join(root, "Profile 3"));
});

test("clone copies the session allowlist and leaves the source untouched", async (t) => {
  const source = await mkdtemp(path.join(os.tmpdir(), "gateway-chatgpt-source-"));
  t.after(() => rm(source, { recursive: true, force: true }));
  await mkdir(path.join(source, "Default", "Network"), { recursive: true });
  await writeFile(path.join(source, "Local State"), "root fixture");
  await writeFile(path.join(source, "Default", "Preferences"), "preferences fixture");
  await writeFile(path.join(source, "Default", "Network", "Cookies"), "cookie fixture");
  await writeFile(path.join(source, "Default", "History"), "not in allowlist");
  const clone = await cloneBrowserProfile(source, "Default");
  t.after(() => rm(clone, { recursive: true, force: true }));
  assert.equal(await readFile(path.join(clone, "Local State"), "utf8"), "root fixture");
  assert.equal(await readFile(path.join(clone, "Default", "Network", "Cookies"), "utf8"), "cookie fixture");
  await assert.rejects(access(path.join(clone, "Default", "History")), { code: "ENOENT" });
  assert.equal(await readFile(path.join(source, "Default", "History"), "utf8"), "not in allowlist");
});

test("missing profile preserves its structured error before allocating a clone", async (t) => {
  const source = await mkdtemp(path.join(os.tmpdir(), "gateway-chatgpt-missing-"));
  t.after(() => rm(source, { recursive: true, force: true }));
  await assert.rejects(cloneBrowserProfile(source, "Missing"), {
    status: 500, code: "chatgpt_web_profile_missing",
  });
});
