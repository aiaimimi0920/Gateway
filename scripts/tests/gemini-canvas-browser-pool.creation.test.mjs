import assert from "node:assert/strict";
import fs from "node:fs";
import fsPromises from "node:fs/promises";
import { syncBuiltinESMExports } from "node:module";
import path from "node:path";
import test from "node:test";
import { chromium } from "playwright-core";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

const { contexts, createContextEntry, ensureContext, initializingContexts } = await importTestableScript();

function stateArgs(t) {
  const root = storageFixture(t);
  const runtimeStateObjectKey = path.join(root, "state.json");
  fs.writeFileSync(runtimeStateObjectKey, '{"cookies":[],"origins":[]}');
  return { runtimeStateObjectKey, browserExecutablePath: process.execPath };
}

for (const attached of [false, true]) {
  test(`failed ${attached ? "CDP" : "owned"} newContext closes its browser connection and clears pending creation`, async (t) => {
    const args = stateArgs(t), failure = new Error("fixture newContext failed"), calls = [];
    if (attached) args.browserCdpUrl = "http://127.0.0.1:1";
    const browser = {
      contexts: () => [],
      newContext: async () => { calls.push("newContext"); throw failure; },
      close: async () => { calls.push("close"); throw new Error("fixture close failed"); },
    };
    t.mock.method(chromium, attached ? "connectOverCDP" : "launch", async () => browser);
    await assert.rejects(ensureContext(args), (error) => error === failure);
    assert.equal(initializingContexts.has(args.runtimeStateObjectKey), false);
    assert.equal(contexts.has(args.runtimeStateObjectKey), false);
    assert.deepEqual(calls, ["newContext", "close"]);
    await assert.rejects(ensureContext(args), (error) => error === failure);
    assert.deepEqual(calls, ["newContext", "close", "newContext", "close"]);
  });
}

test("owned page creation failure closes context and browser while preserving the original failure", async (t) => {
  const args = stateArgs(t), failure = new Error("fixture page failed"), calls = [];
  const context = {
    pages: () => [],
    newPage: async () => { throw failure; },
    close: async () => { calls.push("context"); throw new Error("fixture context close failed"); },
  };
  t.mock.method(chromium, "launch", async () => ({
    newContext: async () => context,
    close: async () => { calls.push("browser"); },
  }));
  await assert.rejects(createContextEntry(args), (error) => error === failure);
  assert.deepEqual(calls, ["context", "browser"]);
  assert.equal(contexts.has(args.runtimeStateObjectKey), false);
});

for (const failAt of ["launch", "page"]) {
  test(`profile clone is removed exactly once after retry ${failAt} failure without changing its source`, async (t) => {
    const root = storageFixture(t), source = path.join(root, "source"), launches = [], removals = [];
    fs.mkdirSync(source);
    fs.writeFileSync(path.join(source, "Local State"), "fixture source");
    const failure = new Error(`fixture ${failAt} failed`);
    let contextCloses = 0;
    const remove = fsPromises.rm;
    t.mock.method(fsPromises, "rm", async (...args) => { removals.push(args[0]); return remove(...args); });
    syncBuiltinESMExports();
    t.after(() => { t.mock.restoreAll(); syncBuiltinESMExports(); });
    t.mock.method(console, "log", () => {});
    t.mock.method(chromium, "launchPersistentContext", async (profile) => {
      launches.push(profile);
      if (launches.length === 1) throw new Error("Process singleton");
      if (failAt === "launch") throw failure;
      return {
        pages: () => [], newPage: async () => { throw failure; },
        close: async () => { contextCloses += 1; },
      };
    });
    await assert.rejects(createContextEntry({ runtimeStateObjectKey: source, browserExecutablePath: process.execPath }), (error) => error === failure);
    assert.equal(launches.length, 2);
    assert.equal(launches[0], source);
    assert.notEqual(launches[1], source);
    assert.equal(fs.existsSync(launches[1]), false);
    assert.equal(fs.readFileSync(path.join(source, "Local State"), "utf8"), "fixture source");
    assert.equal(contextCloses, failAt === "page" ? 1 : 0);
    assert.deepEqual(removals, [launches[1]]);
    assert.equal(contexts.has(source), false);
  });
}
