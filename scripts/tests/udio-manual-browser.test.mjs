import assert from "node:assert/strict";
import test from "node:test";
import * as fs from "node:fs";
import * as io from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import * as settings from "../udio-manual-browser/settings.mjs";
import * as profile from "../udio-manual-browser/profile.mjs";
import * as persistence from "../udio-manual-browser/persistence.mjs";
import * as snapshot from "../udio-manual-browser/snapshot.mjs";

// Import only side-effect-free modules, never the visible-browser entry point.
const api = { ...settings, ...profile, ...persistence, ...snapshot };

async function temporary(t) {
  const root = await io.mkdtemp(path.join(os.tmpdir(), "gateway-udio-contract-"));
  t.after(() => io.rm(root, { recursive: true, force: true }));
  return root;
}

test("normalization preserves timeout clamps and boolean defaults", () => {
  assert.equal(api.normalizeString("  value "), "value");
  assert.equal(api.normalizeString(42), null);
  assert.equal(api.normalizeTimeoutMs("bad", 10, 1, 20), 10);
  assert.equal(api.normalizeTimeoutMs(undefined, 10, 1, 20), 10);
  assert.equal(api.normalizeTimeoutMs("", 10, 1, 20), 1);
  assert.equal(api.normalizeTimeoutMs("99", 10, 1, 20), 20);
  assert.equal(api.normalizeTimeoutMs("2.9", 10, 1, 20), 2);
  for (const value of ["true", " YES ", "on", "1", true]) assert.equal(api.parseBoolean(value, false), true);
  for (const value of ["false", " NO ", "off", "0", false]) assert.equal(api.parseBoolean(value, true), false);
  assert.equal(api.parseBoolean("unknown", true), true);
});

test("profile clone copies only the approved files and Network tree", async (t) => {
  const root = await temporary(t);
  await io.mkdir(path.join(root, "Default", "Network", "nested"), { recursive: true });
  await io.mkdir(path.join(root, "Default", "Local Storage"));
  const files = ["Local State", "Default/Preferences", "Default/Secure Preferences",
    "Default/Network/Cookies", "Default/Network/nested/data", "Default/History",
    "Default/Local Storage/private"];
  for (const name of files) await io.writeFile(path.join(root, name), name);
  const cloned = await api.cloneBrowserProfile(root, "Default");
  t.after(() => io.rm(cloned, { recursive: true, force: true }));
  for (const name of files.slice(0, 5)) assert.equal(await io.readFile(path.join(cloned, name), "utf8"), name);
  assert.equal(fs.existsSync(path.join(cloned, "Default/History")), false);
  assert.equal(fs.existsSync(path.join(cloned, "Default/Local Storage")), false);
  await assert.rejects(api.cloneBrowserProfile(root, "Missing"), /Browser profile not found/);
});

test("profile lock tolerance is limited to EBUSY and EPERM", () => {
  for (const code of ["EBUSY", " eperm "]) assert.equal(api.shouldIgnoreLockedProfileFile({ code }), true);
  for (const code of ["EACCES", "ENOENT", undefined]) assert.equal(api.shouldIgnoreLockedProfileFile({ code }), false);
});

test("auth cookie detection and header filtering preserve exact rules", () => {
  assert.equal(api.hasUdioAuthCookies([{ name: "sb-ssr-production-auth-token.0" }]), true);
  assert.equal(api.hasUdioAuthCookies([{ name: "sb-ssr-production-auth-token" }]), true);
  assert.equal(api.hasUdioAuthCookies([{ name: "sb-ssr-production-auth-token-extra" }]), false);
  assert.equal(api.buildCookieHeader([{ name: " a ", value: " synthetic " }, { name: "b", value: "" }]), "a=synthetic");
});

test("account snapshot falls back when browser evaluation fails", async () => {
  const result = await api.readAccountSnapshot({
    title: async () => { throw new Error("closed"); },
    url: () => "https://www.udio.com/create",
    evaluate: async () => { throw new Error("navigation"); },
  });
  assert.equal(result.title, null);
  assert.equal(result.currentUrl, "https://www.udio.com/create");
  assert.equal(result.accountEmail, null);
  assert.equal(result.localStorageKeys.length, 0);
});

test("snapshot writes require auth and changed hashes; remote sync can be disabled", async (t) => {
  const root = await temporary(t);
  globalThis.__UDIO_MANUAL_HELPER_GENERATE_CAPTURE_PATH = path.join(root, "capture.json");
  t.after(() => { delete globalThis.__UDIO_MANUAL_HELPER_GENERATE_CAPTURE_PATH; });
  let cookies = [];
  const state = { cookies: [], origins: [] };
  const options = {
    context: {
      cookies: async (urls) => {
        assert.equal(JSON.stringify(urls), JSON.stringify(["https://www.udio.com", "https://udio.com"]));
        return cookies;
      },
      storageState: async () => state,
    },
    page: { title: async () => "Udio", url: () => "https://www.udio.com/create",
      evaluate: async () => ({ title: "Udio", currentUrl: "https://www.udio.com/create" }) },
    profileSource: { browser: "synthetic" },
    storageStatePath: path.join(root, "state.json"), statusPath: path.join(root, "status.json"),
    objectKey: null, lastSnapshotHash: { value: null }, lastRemoteHash: { value: null },
  };
  assert.equal((await api.exportAuthSnapshot(options)).authenticated, false);
  assert.equal(fs.existsSync(options.storageStatePath), false);
  cookies = [{ name: "sb-ssr-production-auth-token", value: "synthetic" }];
  assert.equal((await api.exportAuthSnapshot(options)).authenticated, true);
  assert.equal(await io.readFile(options.storageStatePath, "utf8"), JSON.stringify(state, null, 2));
  await io.writeFile(options.storageStatePath, "sentinel");
  await api.exportAuthSnapshot(options);
  assert.equal(await io.readFile(options.storageStatePath, "utf8"), "sentinel");
  state.origins.push({ origin: "https://www.udio.com", localStorage: [] });
  await api.exportAuthSnapshot(options);
  assert.equal(await io.readFile(options.storageStatePath, "utf8"), JSON.stringify(state, null, 2));
  assert.equal(options.lastRemoteHash.value, null);
});

test("optional JSON reads distinguish absent files from malformed content", async (t) => {
  const root = await temporary(t);
  assert.equal(await api.maybeReadJsonFile(" "), null);
  assert.equal(await api.maybeReadJsonFile(path.join(root, "missing.json")), null);
  const file = path.join(root, "invalid.json");
  await io.writeFile(file, "{");
  await assert.rejects(api.maybeReadJsonFile(file), { name: "SyntaxError" });
});

test("local object persistence keeps key paths and exact payload bytes", async (t) => {
  const root = await temporary(t);
  const keys = ["AI_GATEWAY_OBJECT_STORAGE_DRIVER", "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR"];
  const previous = keys.map((key) => process.env[key]);
  t.after(() => keys.forEach((key, i) => {
    if (previous[i] === undefined) delete process.env[key];
    else process.env[key] = previous[i];
  }));
  process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER = "local";
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR = root;
  const body = Buffer.from('{"synthetic":true}');
  const destination = await api.putObject("credential-runtime/udio/state.json", body);
  assert.equal(destination, path.join(root, "credential-runtime/udio/state.json"));
  assert.deepEqual(await io.readFile(destination), body);
});

test("live helper lock reuses the process before touching profiles or browsers", async (t) => {
  const root = await temporary(t);
  const lockPath = path.join(root, "lock.json");
  const statusPath = path.join(root, "status.json");
  await io.writeFile(lockPath, JSON.stringify({ pid: process.pid }));
  const entry = fileURLToPath(new URL("../udio-manual-browser-helper.mjs", import.meta.url));
  const result = spawnSync(process.execPath, [entry], {
    cwd: root, encoding: "utf8", timeout: 10000,
    env: {
      PATH: process.env.PATH,
      SystemRoot: process.env.SystemRoot,
      UDIO_MANUAL_HELPER_BROWSER_EXECUTABLE_PATH: process.execPath,
      UDIO_MANUAL_HELPER_LOCK_PATH: lockPath,
      UDIO_MANUAL_HELPER_STATUS_PATH: statusPath,
      UDIO_MANUAL_HELPER_BROWSER_USER_DATA_DIR: path.join(root, "nonexistent-profile"),
      UDIO_MANUAL_HELPER_DISABLE_OBJECT_SYNC: "true",
    },
  });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    ok: true, reused: true, pid: process.pid, statusPath, lockPath,
  });
  assert.equal(fs.existsSync(statusPath), false);
  assert.deepEqual(JSON.parse(await io.readFile(lockPath, "utf8")), { pid: process.pid });
});
