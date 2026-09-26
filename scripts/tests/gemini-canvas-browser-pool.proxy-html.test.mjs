import assert from "node:assert/strict";
import vm from "node:vm";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const document = '<!DOCTYPE html><html><head></head><body>Browser API Proxy Client</body></html>';
const clientHtml = '<html><head></head><body>Browser API Proxy Client<script>' + [
  'var resolveAuthIndex;',
  'window.__authIndexReady = new Promise(function(resolve) { resolveAuthIndex = resolve; });',
  'window.endpoint = "ws://127.0.0.1:9998";',
  'class FixtureClient {',
  '  _sanitizeHeaders(headers) { return headers; }',
  '  config(requestSpec, signal) {',
  '    const config = { headers: this._sanitizeHeaders(requestSpec.headers), method: requestSpec.method, signal, };',
  '    return config;',
  '  }',
  '  async send(requestUrl, requestConfig) {',
  '    const response = await fetch(requestUrl, requestConfig);',
  '    return response;',
  '  }',
  '  async reader(responsePromise) {',
  '    const response = await responsePromise;',
  '    let reader;',
  '    reader = response.body.getReader();',
  '    return reader;',
  '  }',
  '}',
  'window.client = new FixtureClient();',
].join('\n') + '</script></body></html>';

function executePatched(fetch, authIndex = "2", html = clientHtml) {
  const timers = new Map(), cleared = [], logs = [];
  let nextId = 0;
  const sandbox = {
    AbortController, DOMException, fetch,
    Logger: { output: (message) => logs.push(message) },
    setTimeout: (callback, milliseconds) => { const id = ++nextId; timers.set(id, { callback, milliseconds }); return id; },
    clearTimeout: (id) => { cleared.push(id); timers.delete(id); },
  };
  sandbox.window = sandbox;
  const context = vm.createContext(sandbox);
  const patched = app.injectForcedCanvasProxyAuthIndex(html, authIndex);
  for (const match of patched.matchAll(/<script>([\s\S]*?)<\/script>/g)) vm.runInContext(match[1], context, { timeout: 1000 });
  return { sandbox, timers, cleared, logs, patched };
}

test("proxy HTML decoding extracts a complete document and rejects incomplete inputs", () => {
  assert.equal(app.decodeEscapedCanvasProxyHtml("prefix" + document + "suffix"), document);
  for (const value of [null, "", "ordinary", "<html>missing close"]) assert.equal(app.decodeEscapedCanvasProxyHtml(value), null);
});

test("proxy HTML decoding handles captured unicode delimiters without changing document content", () => {
  const encoded = document.replaceAll("<", "\\\\u003c").replaceAll(">", "\\\\u003e");
  assert.equal(app.decodeEscapedCanvasProxyHtml(encoded), document);
});

test("proxy client extraction skips unrelated HTML and returns the first matching client", () => {
  assert.equal(app.extractCanvasProxyClientHtmlFromTexts(["<html>unrelated</html>", document, document.replace("Client", "Client later")]), document);
  assert.equal(app.extractCanvasProxyClientHtmlFromTexts(null), null);
  assert.equal(app.extractCanvasProxyClientHtmlFromTexts(["<html>Browser Proxy Client</html>"]), "<html>Browser Proxy Client</html>");
});

test("injected auth bootstrap initializes globals resolves auth readiness and upgrades loopback WS", async () => {
  for (const [value, expected] of [[" 3 ", 3], ["invalid", 0], ["-1", 0], [null, 0]]) {
    const { sandbox } = executePatched(async () => ({}), value);
    assert.equal(sandbox.chrome._contextId, expected);
    assert.equal(sandbox.__NEURO_FORCED_AUTH_INDEX__, expected);
    assert.equal(await sandbox.__authIndexReady, expected);
    assert.equal(sandbox.endpoint, "wss://127.0.0.1:9998");
  }
});

test("injected fetch includes credentials and clears its timer on success", async () => {
  const response = { status: 200, url: "https://fixture.invalid/result", body: {} };
  let received;
  const { sandbox, timers, cleared } = executePatched(async (_url, config) => { received = config; return response; });
  const headers = { "x-fixture": "synthetic" }, controller = new AbortController();
  const config = sandbox.client.config({ headers, method: "POST" }, controller.signal);
  assert.equal(await sandbox.client.send(response.url, config), response);
  assert.equal(received.credentials, "include");
  assert.equal(received.headers, headers);
  assert.equal(received.method, "POST");
  assert.notEqual(received.signal, controller.signal);
  assert.equal(timers.size, 0);
  assert.equal(cleared.length, 1);
});

test("injected fetch clears its timer and preserves rejection identity", async () => {
  const failure = new Error("synthetic fetch failure");
  const { sandbox, timers, cleared } = executePatched(async () => { throw failure; });
  await assert.rejects(sandbox.client.send("https://fixture.invalid/fail", {}), (error) => error === failure);
  assert.equal(timers.size, 0);
  assert.equal(cleared.length, 1);
});

test("injected fetch forwards both pre-existing and in-flight caller cancellation", async () => {
  for (const alreadyAborted of [false, true]) {
    const controller = new AbortController();
    if (alreadyAborted) controller.abort();
    const { sandbox, timers } = executePatched((_url, config) => new Promise((_resolve, reject) => {
      if (config.signal.aborted) reject(config.signal.reason);
      else config.signal.addEventListener("abort", () => reject(config.signal.reason), { once: true });
    }));
    const pending = sandbox.client.send("https://fixture.invalid/abort", { signal: controller.signal });
    if (!alreadyAborted) controller.abort();
    await assert.rejects(pending, (error) => error.name === "AbortError");
    assert.equal(timers.size, 0);
  }
});

test("injected fetch timeout aborts the request and clears the scheduled timer", async () => {
  const { sandbox, timers, cleared } = executePatched((_url, config) => new Promise((_resolve, reject) => config.signal.addEventListener("abort", () => reject(config.signal.reason), { once: true })));
  const pending = sandbox.client.send("https://fixture.invalid/timeout", {});
  const [{ callback, milliseconds }] = [...timers.values()];
  assert.equal(milliseconds, 30000);
  callback();
  await assert.rejects(pending, (error) => error.name === "AbortError" && /timeout/.test(error.message));
  assert.equal(timers.size, 0);
  assert.equal(cleared.length, 1);
});

test("injected response reader rejects absent bodies and keeps the actual reader", async () => {
  const { sandbox } = executePatched(async () => ({}));
  await assert.rejects(sandbox.client.reader(Promise.resolve({ body: null })), /Response body missing before reader acquisition/);
  const reader = {};
  assert.equal(await sandbox.client.reader(Promise.resolve({ body: { getReader: () => reader } })), reader);
});

test("auth bootstrap works without a head element and ignores empty HTML", () => {
  const { sandbox, patched } = executePatched(async () => ({}), "4", "<html><body>fixture</body></html>");
  assert.equal(sandbox.chrome._contextId, 4);
  assert.ok(patched.startsWith("<script>"));
  assert.equal(app.injectForcedCanvasProxyAuthIndex(""), null);
});
