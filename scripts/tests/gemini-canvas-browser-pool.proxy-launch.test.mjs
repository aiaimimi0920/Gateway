import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { after, before, test } from "node:test";
import { app, appFixture, baseUrl, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

const launch = app.tryLaunchCanvasProxyClientFromCapturedHtml;
const eventNames = ["console", "pageerror", "request", "response", "requestfailed", "requestfinished"];
const html = '<!DOCTYPE html><html><head></head><body>Browser API Proxy Client Connected System initializing<script>class ProxySystem { initialize() {} } new ProxySystem().initialize(); var requestAuthIndex; console.log("connected synthetic");</script></body></html>';
const sourcePage = { url: () => `${baseUrl}/u/3/app` };
const originalCwd = process.cwd();
let testRoot;

before(async () => {
  testRoot = await fs.mkdtemp(path.join(os.tmpdir(), "gateway-browser-direct-launch-"));
  process.chdir(testRoot);
});
after(async () => {
  process.chdir(originalCwd);
  if (!testRoot) return;
  assert.equal(path.dirname(testRoot), path.resolve(os.tmpdir()));
  assert.ok(path.basename(testRoot).startsWith("gateway-browser-direct-launch-"));
  await fs.rm(testRoot, { recursive: true, force: true });
});

function fakePage(t, options = {}) {
  t.mock.method(console, "log", () => {});
  const page = new EventEmitter(), state = { closed: 0, waits: [], ...options };
  const external = new Map(eventNames.map((name) => [name, () => {}]));
  for (const [name, listener] of external) page.on(name, listener);
  Object.assign(page, {
    url: () => "about:blank",
    bringToFront: async () => { if (state.frontError) throw new Error("synthetic front failure"); },
    addInitScript: async (fn, args) => {
      state.init = { fn, args };
      if (state.failAt === "init") throw new Error("synthetic init failure");
    },
    setContent: async (content, settings) => {
      state.content = content; state.settings = settings;
      await state.onContent?.(page);
      if (state.failAt === "content") throw new Error("synthetic content failure");
    },
    waitForTimeout: async (ms) => { state.waits.push(ms); },
    evaluate: async (fn) => {
      if (state.diagnosticsError) throw new Error("synthetic evaluate failure");
      return fn.toString().includes("innerText") ? (state.body ?? "Connected") : { url: "about:blank", chromeContextId: 3 };
    },
    close: async () => { state.closed += 1; if (state.closeError) throw new Error("synthetic close failure"); },
  });
  return { page, state, external };
}

function assertDetached(page, external) {
  for (const name of eventNames) assert.deepEqual(page.listeners(name), [external.get(name)]);
}

test("direct proxy launch with missing HTML does not create a page", async () => {
  const result = await launch({ newPage: async () => assert.fail("must not create page") }, sourcePage, {}, null, 1000);
  assert.deepEqual(result, { launched: false, reason: "canvas_proxy_client_html_missing" });
});

test("direct proxy launch propagates newPage rejection before listener acquisition", async () => {
  const failure = new Error("synthetic newPage failure");
  await assert.rejects(launch({ newPage: async () => { throw failure; } }, sourcePage, { bodyText: html }, null, 1000), (error) => error === failure);
});

test("direct proxy launch retains the created page and writes the exact injected artifact", async (t) => {
  const { page, state, external } = fakePage(t, { frontError: true, body: "Connected" + ".".repeat(2000) });
  const result = await launch({ newPage: async () => page }, sourcePage, { bodyText: html }, null, 30000);
  assert.equal(result.launched, true);
  assert.equal(result.reason, "about_blank_set_content");
  assert.equal(result.page, page);
  assert.equal(result.authIndex, "3");
  assert.equal(state.init.args.authIndexValue, 3);
  assert.equal(typeof state.init.fn, "function");
  assert.deepEqual(state.settings, { waitUntil: "domcontentloaded", timeout: 20000 });
  assert.deepEqual(state.waits, [800]);
  assert.equal(result.bodyPreview.length, 800);
  assert.equal(state.closed, 0);
  assertDetached(page, external);
  const relative = path.relative(testRoot, result.artifactPath);
  assert.ok(!relative.startsWith("..") && !path.isAbsolute(relative));
  assert.equal(await fs.readFile(result.artifactPath, "utf8"), state.content);
  assert.equal(result.htmlBytes, state.content.length);
  for (const key of ["htmlContainsRequestAuthIndex", "htmlContainsProxySystemClass", "htmlContainsProxySystemInitCall", "htmlContainsSystemInitializingText"]) assert.equal(result[key], true);
});

for (const failAt of ["init", "content"]) {
  test(`direct proxy ${failAt} failure closes only the created page and detaches listeners`, async (t) => {
    const { page, state, external } = fakePage(t, { failAt, closeError: true });
    let sourceClosed = false;
    const result = await launch({ newPage: async () => page }, { ...sourcePage, close: () => { sourceClosed = true; } }, { bodyText: html }, null, 1000);
    assert.equal(result.launched, false);
    assert.equal(result.reason, "captured_html_launch_error");
    assert.equal(result.errorMessage, `synthetic ${failAt} failure`);
    assert.equal(Object.hasOwn(result, "page"), false);
    assert.equal(state.closed, 1);
    assert.equal(sourceClosed, false);
    assertDetached(page, external);
  });
}

test("direct proxy diagnostics filter unrelated events and bound retained payload fields", async (t) => {
  const url = "https://generativelanguage.googleapis.com/v1/fixture";
  const request = { url: () => url, method: () => "POST", headers: () => ({ "Content-Type": "application/json", cookie: "fixture=synthetic", authorization: "synthetic-excluded" }), postData: () => "x".repeat(2100), failure: () => ({ errorText: "synthetic timeout" }) };
  const { page, external } = fakePage(t, { onContent: async (target) => {
    target.emit("request", { ...request, url: () => "https://fixture.invalid/ignored" });
    target.emit("request", request);
    for (const listener of target.listeners("response")) await listener({ url: () => url, status: () => 200,
      headers: () => ({ "content-type": "application/json", "content-length": "2100", "access-control-allow-origin": "*", "set-cookie": "synthetic-excluded" }),
      text: async () => "y".repeat(2100) });
    target.emit("requestfailed", request);
    target.emit("requestfinished", request);
    target.emit("pageerror", new Error("synthetic page error"));
  } });
  const result = await launch({ newPage: async () => page }, sourcePage, { bodyText: html }, null, 1000);
  assert.deepEqual(result.networkEvents.map((event) => event.type), ["request", "response", "requestfailed", "requestfinished"]);
  assert.deepEqual(result.networkEvents[0].headers, { "Content-Type": "application/json", cookie: "fixture=synthetic" });
  assert.equal(result.networkEvents[0].postData.length, 2000);
  assert.deepEqual(result.networkEvents[1].headers, { "content-type": "application/json", "access-control-allow-origin": "*" });
  assert.equal(result.networkEvents[1].bodyPreview.length, 2000);
  assert.equal(result.networkEvents[2].failureText, "synthetic timeout");
  assert.equal(result.consoleEvents[0].type, "pageerror");
  assertDetached(page, external);
});

test("direct proxy cleanup releases response transport after an earlier listener detach fails", async (t) => {
  const { page, external } = fakePage(t);
  const off = page.off.bind(page), failure = new Error("console detach failed");
  page.off = (event, listener) => {
    off(event, listener);
    if (event === "console") throw failure;
    return page;
  };
  await assert.rejects(launch({ newPage: async () => page }, sourcePage, { bodyText: html }, null, 1000),
    (error) => error === failure);
  assertDetached(page, external);
});

test("direct proxy partial listener registration closes its page and rolls back ownership", async (t) => {
  const { page, state, external } = fakePage(t);
  const on = page.on.bind(page);
  page.on = (event, listener) => {
    on(event, listener);
    if (event === "requestfinished") throw new Error("registration failed");
    return page;
  };
  const result = await launch({ newPage: async () => page }, sourcePage, { bodyText: html }, null, 1000);
  assert.equal(result.launched, false);
  assert.equal(result.errorMessage, "registration failed");
  assert.equal(state.closed, 1);
  assertDetached(page, external);
});

test("direct proxy result retains only the latest console and network diagnostic windows", async (t) => {
  const { page } = fakePage(t, { onContent: async (target) => {
    for (let index = 0; index < 100; index += 1) {
      target.emit("console", { type: () => "log", text: () => `message-${index}` });
      target.emit("requestfinished", { url: () => `https://generativelanguage.googleapis.com/${index}`, method: () => "GET" });
    }
  } });
  const result = await launch({ newPage: async () => page }, sourcePage, { bodyText: html }, null, 1000);
  assert.equal(result.consoleEvents.length, 20);
  assert.equal(result.consoleEvents[0].text, "message-80");
  assert.equal(result.networkEvents.length, 40);
  assert.ok(result.networkEvents[0].url.endsWith("/60"));
});

test("direct proxy diagnostic evaluation failure does not discard a successfully created page", async (t) => {
  const { page, state, external } = fakePage(t, { diagnosticsError: true });
  const result = await launch({ newPage: async () => page }, sourcePage, { bodyText: html }, null, 0);
  assert.equal(result.launched, true);
  assert.equal(result.page, page);
  assert.equal(result.runtimeDiagnostics.errorMessage, "synthetic evaluate failure");
  assert.equal(result.bodyPreview, "");
  assert.equal(state.closed, 0);
  assertDetached(page, external);
});

test("real offline direct proxy page keeps auth globals and source-page state until context cleanup", browserTest, async (t) => {
  const { context, page: source } = await appFixture(t, { initialUrl: `${baseUrl}/u/3/app` });
  await source.locator("#draft").fill("retained source draft");
  const result = await launch(context, source, { bodyText: html }, null, 2000);
  assert.equal(result.launched, true);
  assert.notEqual(result.page, source);
  assert.equal(result.page.isClosed(), false);
  assert.equal(source.isClosed(), false);
  assert.equal(await source.locator("#draft").inputValue(), "retained source draft");
  assert.equal(result.runtimeDiagnostics.chromeContextId, 3);
  assert.equal(result.runtimeDiagnostics.forcedAuthIndex, 3);
  assert.equal(result.runtimeDiagnostics.proxySystemType, "function");
  assert.equal(await result.page.evaluate(() => window.__NEURO_FORCED_AUTH_INDEX__), 3);
  for (const name of eventNames) assert.equal(result.page.listenerCount(name), 0);
  assert.equal(context.pages().length, 2);
});
