import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

export const pool = await importTestableScript();

export function contextFixture(t) {
  const names = ["GEMINI_CANVAS_BROWSER_MAX_CONTEXTS", "GEMINI_CANVAS_BROWSER_IDLE_TIMEOUT_MS"];
  const previous = names.map((name) => process.env[name]);
  process.env[names[0]] = "4";
  process.env[names[1]] = "10000";
  t.mock.method(console, "log", () => {});
  t.after(async () => {
    try {
      await Promise.all([...pool.contexts.keys()].map((key) => pool.closeContext(key)));
      assert.equal(pool.contexts.size, 0);
      assert.equal(pool.initializingContexts.size, 0);
    } finally {
      names.forEach((name, i) => {
        if (previous[i] === undefined) delete process.env[name];
        else process.env[name] = previous[i];
      });
    }
  });
}

export function browserSession() {
  const browser = new EventEmitter(), context = new EventEmitter();
  const page = { closed: false, isClosed() { return this.closed; } };
  const pages = [page], closes = [];
  browser.connected = true;
  browser.isConnected = () => browser.connected;
  browser.contexts = () => [context];
  browser.newContext = async () => context;
  browser.close = async () => {
    closes.push("browser");
    browser.connected = false;
    browser.emit("disconnected");
  };
  context.pages = () => pages;
  context.browser = () => browser;
  context.close = async () => { closes.push("context"); context.emit("close"); };
  return { browser, context, page, pages, closes };
}

export const contextArgs = (key) => ({ runtimeStateObjectKey: key, browserCdpUrl: "http://127.0.0.1:1" });

export function deferred() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}
