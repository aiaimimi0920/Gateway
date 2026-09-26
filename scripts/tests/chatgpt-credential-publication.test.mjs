import assert from "node:assert/strict";
import { link, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import { buildCredentialPayload, maybeWriteCredentialFile } from "../chatgpt-web-session/credentials.mjs";

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-credential-publication-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  return root;
}

test("credential replacement does not truncate or overwrite a linked previous inode", async (t) => {
  const root = await fixture(t);
  const target = path.join(root, "account.json");
  const previous = path.join(root, "previous.json");
  await writeFile(previous, "previous synthetic credential\n");
  await link(previous, target);
  const result = { ok: true, authToken: "new-synthetic-token" };
  await maybeWriteCredentialFile({ credentialFilePath: target }, result);
  assert.equal(await readFile(previous, "utf8"), "previous synthetic credential\n");
  assert.deepEqual(JSON.parse(await readFile(target, "utf8")), buildCredentialPayload(result));
  assert.deepEqual((await readdir(root)).sort(), ["account.json", "previous.json"]);
});

test("failed credential rename leaves the target directory and removes owned staging", async (t) => {
  const root = await fixture(t);
  const target = path.join(root, "account.json");
  await mkdir(target);
  await writeFile(path.join(target, "sentinel"), "preserve");
  await assert.rejects(maybeWriteCredentialFile({ credentialFilePath: target }, { ok: true, authToken: "synthetic" }));
  assert.equal(await readFile(path.join(target, "sentinel"), "utf8"), "preserve");
  assert.deepEqual(await readdir(root), ["account.json"]);
});

test("new credential files are owner-only on POSIX", { skip: process.platform === "win32" }, async (t) => {
  const root = await fixture(t);
  const target = path.join(root, "account.json");
  await maybeWriteCredentialFile({ credentialFilePath: target }, { ok: true, authToken: "synthetic" });
  assert.equal((await stat(target)).mode & 0o777, 0o600);
});

test("concurrent credential replacements publish one complete payload without staging residue", async (t) => {
  const root = await fixture(t);
  const target = path.join(root, "account.json");
  const results = Array.from({ length: 12 }, (_, index) => ({
    ok: true, authToken: `synthetic-${index}-${"x".repeat(64 * 1024)}`,
  }));
  const outcomes = await Promise.allSettled(results.map((result) => maybeWriteCredentialFile({ credentialFilePath: target }, result)));
  for (const outcome of outcomes) assert.equal(outcome.status, "fulfilled", outcome.reason?.code);
  const payload = JSON.parse(await readFile(target, "utf8"));
  assert.ok(results.some((result) => JSON.stringify(buildCredentialPayload(result)) === JSON.stringify(payload)));
  assert.deepEqual(await readdir(root), ["account.json"]);
});

test("serialization failure leaves the previous file unchanged and creates no staging", async (t) => {
  const root = await fixture(t);
  const target = path.join(root, "account.json");
  await writeFile(target, "previous synthetic credential\n");
  const circular = {};
  circular.self = circular;
  await assert.rejects(maybeWriteCredentialFile({ credentialFilePath: target }, { ok: true, authProbe: circular }), /circular/i);
  assert.equal(await readFile(target, "utf8"), "previous synthetic credential\n");
  assert.deepEqual(await readdir(root), ["account.json"]);
});

test("publication fault injection preserves primary errors and attempts handle/staging cleanup", async () => {
  const source = await readFile(new URL("../chatgpt-web-session/credentials.mjs", import.meta.url), "utf8");
  const body = source.slice(source.indexOf("export async function maybeWriteCredentialFile"), source.indexOf("export function buildCredentialPayload"));
  for (const failingStage of ["write", "sync", "close", "rename"]) {
    for (const failCleanup of [false, true]) {
      const calls = [];
      const primary = new Error(`fixture ${failingStage}`);
      const cleanup = new Error("fixture cleanup");
      const stage = (name) => async () => {
        calls.push(name);
        if (name === failingStage) throw primary;
        if (name === "unlink" && failCleanup) throw cleanup;
      };
      const publish = vm.runInNewContext(`(${body.replace("export ", "")})`, {
        path, process: { env: {} }, AggregateError,
        resolveCredentialFilePath: () => "fixture.json",
        buildCredentialPayload: () => ({ fixture: true }),
        randomUUID: () => "fixture", mkdir: stage("mkdir"),
        open: async () => ({ writeFile: stage("write"), sync: stage("sync"), close: stage("close") }),
        rename: stage("rename"), unlink: stage("unlink"),
      });
      await assert.rejects(publish({ writeCredentialFile: true }, { ok: true }), (error) => {
        if (failCleanup) assert.deepEqual(error.errors, [primary, cleanup]);
        else assert.equal(error, primary);
        return true;
      });
      assert.ok(calls.includes("close"));
      assert.equal(calls.at(-1), "unlink");
      if (failingStage !== "rename") assert.equal(calls.includes("rename"), false);
    }
  }
});

test("Windows replacement retries are bounded and never delete the destination", async () => {
  const source = await readFile(new URL("../chatgpt-web-session/credentials.mjs", import.meta.url), "utf8");
  const body = source.slice(source.indexOf("export async function maybeWriteCredentialFile"), source.indexOf("export function buildCredentialPayload"));
  for (const failures of [2, Infinity]) {
    let attempts = 0;
    const waits = [];
    const removed = [];
    const busy = Object.assign(new Error("fixture busy"), { code: "EPERM" });
    const publish = vm.runInNewContext(`(${body.replace("export ", "")})`, {
      path, process: { env: {}, platform: "win32" }, AggregateError,
      resolveCredentialFilePath: () => "fixture.json", buildCredentialPayload: () => ({}),
      randomUUID: () => "fixture", mkdir: async () => {},
      open: async () => ({ writeFile: async () => {}, sync: async () => {}, close: async () => {} }),
      rename: async () => { if (++attempts <= failures) throw busy; },
      unlink: async (name) => { removed.push(name); }, delay: async (ms) => { waits.push(ms); },
    });
    if (failures === Infinity) await assert.rejects(publish({ writeCredentialFile: true }, { ok: true }), (error) => error === busy);
    else assert.equal(await publish({ writeCredentialFile: true }, { ok: true }), "fixture.json");
    assert.equal(attempts, failures === Infinity ? 8 : 3);
    assert.ok(waits.reduce((sum, ms) => sum + ms, 0) <= 710);
    assert.deepEqual(removed, [".chatgpt-credential-fixture.tmp"]);
  }
});
