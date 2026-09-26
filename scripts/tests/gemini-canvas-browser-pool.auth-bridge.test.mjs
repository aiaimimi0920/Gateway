import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { app, appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

const plain = (value) => JSON.parse(JSON.stringify(value));

function bridgeFixture(options = {}) {
  const listeners = [], domListeners = [], scripts = [], calls = [], sockets = [];
  class Socket {
    static OPEN = 1;
    constructor(...args) { sockets.push(args); }
  }
  const window = { WebSocket: Socket, addEventListener: (type, callback) => listeners.push({ type, callback }) };
  if (options.chrome) window.chrome = options.chrome;
  const document = { readyState: options.loading ? "loading" : "complete",
    addEventListener: (type, callback, config) => domListeners.push({ type, callback, config }) };
  const sandbox = vm.createContext({ window, document, URL, Error, Date: { now: () => 12345 } });
  const page = {
    addInitScript: async (callback, input) => { calls.push("init"); scripts.push({ callback, input }); if (options.initError) throw new Error("init failed"); },
    evaluate: async (callback, input) => {
      calls.push("evaluate"); if (options.evaluateError) throw new Error("evaluate failed");
      sandbox.input = input;
      return await vm.runInContext(`(${callback.toString()})(input)`, sandbox);
    },
  };
  return { page, window, document, listeners, domListeners, scripts, calls, sockets, Socket,
    emit: (event) => { for (const listener of listeners) if (listener.type === "message") listener.callback(event); } };
}

test("auth bridge normalizes account input and preserves chrome fields and install order", async () => {
  const f = bridgeFixture({ chrome: { retained: "value" } });
  const result = await app.installCanvasProxyPreviewAuthIndexBridge(f.page, " 03 ");
  assert.deepEqual(plain(result), { authIndex: "3", installedAt: 12345, eventCount: 1 });
  assert.deepEqual(f.calls, ["init", "evaluate"]);
  assert.deepEqual(plain(f.scripts[0].input), { authIndexValue: "03" });
  assert.equal(f.window.chrome.retained, "value");
  assert.equal(f.window.chrome._contextId, 3);
  assert.equal(f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.authIndex, "03");
  assert.equal(f.listeners.length, 1);
});

test("auth bridge repeated and DOM-ready installation retain the first bridge and one message listener", async () => {
  const f = bridgeFixture({ loading: true });
  await app.installCanvasProxyPreviewAuthIndexBridge(f.page, "2");
  const bridge = f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__, wrapped = f.window.WebSocket;
  assert.equal(f.domListeners[0].type, "DOMContentLoaded");
  assert.deepEqual(plain(f.domListeners[0].config), { once: true });
  f.document.readyState = "complete";
  f.domListeners[0].callback();
  const again = await app.installCanvasProxyPreviewAuthIndexBridge(f.page, "9");
  assert.equal(f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__, bridge);
  assert.equal(f.window.WebSocket, wrapped);
  assert.equal(f.window.chrome._contextId, 2);
  assert.equal(f.listeners.length, 1);
  assert.equal(again.authIndex, "2");
  assert.equal(bridge.events.length, 1);
});

test("auth bridge still evaluates after init failure and reports normalized fallback on evaluation failure", async () => {
  const init = bridgeFixture({ initError: true });
  const installed = await app.installCanvasProxyPreviewAuthIndexBridge(init.page, "bad");
  assert.equal(installed.authIndex, "0");
  assert.deepEqual(init.calls, ["init", "evaluate"]);
  const failed = bridgeFixture({ initError: true, evaluateError: true });
  const fallback = await app.installCanvasProxyPreviewAuthIndexBridge(failed.page, " 7 ");
  assert.deepEqual(fallback, { authIndex: "7", installedAt: null, eventCount: 0 });
  assert.deepEqual(failed.calls, ["init", "evaluate"]);
});

test("auth bridge rewrites only the loopback socket endpoint and preserves protocols and prototype", async () => {
  const f = bridgeFixture();
  await app.installCanvasProxyPreviewAuthIndexBridge(f.page);
  const Wrapped = f.window.WebSocket;
  const socket = new Wrapped("ws://127.0.0.1:9998/path?x=1", ["synthetic"]);
  new Wrapped("ws://127.0.0.1:99980/path");
  new Wrapped(new URL("wss://fixture.invalid/socket"));
  assert.equal(socket instanceof f.Socket, true);
  assert.equal(Wrapped.OPEN, 1);
  assert.deepEqual(plain(f.sockets), [["wss://127.0.0.1:9998/path?x=1", ["synthetic"]], ["ws://127.0.0.1:99980/path"], ["wss://fixture.invalid/socket"]]);
  const rewrites = f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.events.filter((event) => event.kind === "ws_rewrite");
  assert.equal(rewrites.length, 1);
  assert.equal(rewrites[0].from, "ws://127.0.0.1:9998/path?x=1");
});

test("auth bridge routes account replies to the message source and records reply failure", async () => {
  const f = bridgeFixture(), replies = [];
  await app.installCanvasProxyPreviewAuthIndexBridge(f.page, "5");
  f.emit({ origin: "https://fixture.invalid", data: { type: "requestAuthIndex", endpoint: "synthetic" }, source: { postMessage: (...args) => replies.push(args) } });
  assert.deepEqual(plain(replies), [[{ type: "authIndexResponse", authIndex: "5" }, "*"]]);
  f.emit({ data: { type: "requestAuthIndex" }, source: { postMessage: () => { throw new Error("detached sender"); } } });
  const events = f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.events;
  assert.deepEqual(plain(events.map((event) => event.kind)), ["install", "message", "reply", "message", "reply_error"]);
  assert.equal(events[1].origin, "https://fixture.invalid");
  assert.equal(events[1].endpoint, "synthetic");
  assert.equal(events.at(-1).errorMessage, "detached sender");
});

test("auth bridge bounds message previews and tolerates cyclic messages and frozen event storage", async () => {
  const f = bridgeFixture();
  await app.installCanvasProxyPreviewAuthIndexBridge(f.page);
  const cyclic = {}; cyclic.self = cyclic;
  f.emit({ data: "x".repeat(2000) });
  f.emit({ data: cyclic });
  const events = f.window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.events;
  assert.equal(events[1].messagePreview.length, 1200);
  assert.equal(events[2].messagePreview, "[[message preview unavailable]]");
  Object.freeze(events);
  assert.doesNotThrow(() => f.emit({ data: "after freeze" }));
});

test("auth bridge real offline page reload runs the registered init script and answers account requests", browserTest, async (t) => {
  const f = await appFixture(t);
  const result = await app.installCanvasProxyPreviewAuthIndexBridge(f.page, "6");
  assert.equal(result.authIndex, "6");
  await f.page.reload();
  const account = await f.page.evaluate(async () => {
    const reply = await new Promise((resolve, reject) => {
      const listener = (event) => {
        if (event.data?.type !== "authIndexResponse") return;
        clearTimeout(timer);
        window.removeEventListener("message", listener);
        resolve(event.data.authIndex);
      };
      const timer = setTimeout(() => {
        window.removeEventListener("message", listener);
        reject(new Error("synthetic bridge response timed out"));
      }, 5000);
      window.addEventListener("message", listener);
      window.postMessage({ type: "requestAuthIndex" }, "*");
    });
    return { reply, contextId: window.chrome._contextId, installs: window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.events.filter((event) => event.kind === "install").length };
  });
  assert.deepEqual(account, { reply: "6", contextId: 6, installs: 1 });
});
