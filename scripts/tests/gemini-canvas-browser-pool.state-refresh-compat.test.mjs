import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import test from "node:test";
import { chromium } from "playwright-core";
import { S3Client } from "@aws-sdk/client-s3";
import { refreshFixture, state } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";

test("CDP requests bypass storage reads and changed CDP binding cannot reuse the old connection", async t => {
  const f = refreshFixture(t), endpoints = [], closed = [];
  t.mock.method(S3Client.prototype, "send", async () => { throw new Error("CDP must not read storage"); });
  t.mock.method(chromium, "connectOverCDP", async url => {
    endpoints.push(url);
    const browser = new EventEmitter(), context = new EventEmitter(), page = { isClosed: () => false };
    browser.contexts = () => [context]; browser.isConnected = () => true;
    browser.close = async () => { closed.push(url); browser.emit("disconnected"); };
    context.pages = () => [page]; context.browser = () => browser;
    context.close = async () => { throw new Error("Attached context must not be closed"); };
    return browser;
  });
  const args = { ...f.args(), browserCdpUrl: "http://127.0.0.1:1" };
  const first = await f.owner.ensureContext(args);
  assert.equal(await f.owner.ensureContext(args), first);
  const second = await f.owner.ensureContext({ ...args, browserCdpUrl: "http://127.0.0.1:2" });
  assert.notEqual(second, first);
  assert.deepEqual(endpoints, ["http://127.0.0.1:1", "http://127.0.0.1:2"]);
  assert.deepEqual(closed, ["http://127.0.0.1:1"]);
});

test("persistent profile lifetime remains browser-owned rather than hashing its live databases", async t => {
  const f = refreshFixture(t), directory = path.join(f.root, "profile");
  fs.mkdirSync(path.join(directory, "Default"), { recursive: true });
  fs.writeFileSync(path.join(directory, "Local State"), "{}", "utf8");
  const context = new EventEmitter(), page = { isClosed: () => false };
  context.pages = () => [page]; context.browser = () => null;
  context.close = async () => context.emit("close");
  const launch = t.mock.method(chromium, "launchPersistentContext", async (location, options) => {
    assert.equal(location, directory);
    assert.equal(options.args.includes("--profile-directory=Default"), true);
    return context;
  });
  const original = await f.owner.ensureContext(f.args("profile"));
  fs.writeFileSync(path.join(directory, "Default", "Cookies"), "synthetic browser mutation", "utf8");
  assert.equal(await f.owner.ensureContext(f.args("profile")), original);
  assert.equal(launch.mock.callCount(), 1);
  assert.equal(f.launch.mock.callCount(), 0);
});

test("source refresh excludes its entry from capacity and idle eviction until its I/O finishes", { timeout: 10000 }, async t => {
  const f = refreshFixture(t), oldLimit = process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS;
  process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS = "1";
  t.after(() => {
    if (oldLimit === undefined) delete process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS;
    else process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS = oldLimit;
  });
  Object.assign(process.env, { AI_GATEWAY_OBJECT_STORAGE_DRIVER: "s3", AI_GATEWAY_OBJECT_STORAGE_BUCKET: "fixture",
    AI_GATEWAY_OBJECT_STORAGE_ENDPOINT: "http://127.0.0.1:1", AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID: "FIXTURE",
    AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY: "fixture-only" });
  let release, entered, calls = 0;
  const gate = new Promise(resolve => { release = resolve; });
  const waiting = new Promise(resolve => { entered = resolve; });
  t.mock.method(S3Client.prototype, "send", async () => {
    if (++calls === 2) { entered(); await gate; }
    return { Body: Readable.from([JSON.stringify(state(calls === 1 ? "old" : "new"))]) };
  });
  const original = await f.owner.ensureContext(f.args());
  original.lastUsedAt = 0;
  const refresh = f.owner.ensureContext(f.args());
  try {
    await waiting;
    assert.equal(original.busy, true);
    await f.owner.evictIdleContexts();
    assert.deepEqual(f.sessions[0].closes, []);
    await assert.rejects(f.owner.ensureContext(f.args("other.json")), error => error.status === 429);
    assert.equal(calls, 2);
  } finally { release(); }
  const replacement = await refresh;
  assert.notEqual(replacement, original);
  assert.equal(original.busy, false);
  assert.equal(replacement.busy, false);
  assert.equal(f.owner.initializingContexts.size, 0);
});
