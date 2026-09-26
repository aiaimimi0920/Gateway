import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const derive = app.deriveCanvasProxyWsInvokeCandidate;
const appPath = "/app/abcdef123456";

test("proxy endpoint discovery preserves pattern precedence and accepts each legacy endpoint form", () => {
  const cases = [
    ['DEFAULT_ENDPOINT = "wss://fixture.invalid/default"; constructor(endpoint = "wss://fixture.invalid/ctor")', "wss://fixture.invalid/default"],
    ['constructor(endpoint = "wss://fixture.invalid/ctor")', "wss://fixture.invalid/ctor"],
    ['placeholder="ws://fixture.invalid/input"', "ws://fixture.invalid/input"],
    ["connect ws://127.0.0.1:9000/ws", "ws://127.0.0.1:9000/ws"],
  ];
  for (const [text, endpoint] of cases) {
    assert.equal(derive("video", { bodyText: "Browser API Proxy Client " + text }, null).wsUrl, endpoint);
  }
  assert.equal(derive("video", null, null), null);
  assert.equal(derive("video", { bodyText: "Browser API Proxy Client without endpoint" }, null), null);
});

test("proxy contract text collection filters in snapshot then capture order without mutation", () => {
  const snapshot = Object.freeze({ bodyText: "Browser API Proxy Client body", bodyBase64: "Server WS Endpoint raw field" });
  const captures = Object.freeze([
    Object.freeze({ bodyText: "ordinary body", url: "https://fixture.invalid/proxy_request" }),
    Object.freeze({ bodyText: "targetDomain=fixture", url: "https://fixture.invalid/ignored" }),
  ]);
  assert.deepEqual(app.collectCanvasProxyContractTexts(snapshot, Object.freeze({ rpcCaptures: captures })), [snapshot.bodyText, snapshot.bodyBase64, captures[0].url, captures[1].bodyText]);
});

test("proxy discovery uses the first eligible text without combining later endpoints or models", () => {
  const snapshot = { bodyText: "Browser API Proxy Client no endpoint" };
  const captures = { rpcCaptures: [
    { bodyText: 'targetDomain = "first.generativelanguage.googleapis.com" models/first' },
    { bodyText: 'Browser API Proxy Client DEFAULT_ENDPOINT = "wss://fixture.invalid/later" models/later' },
  ] };
  const result = derive("music", snapshot, captures);
  assert.equal(result.wsUrl, null);
  assert.equal(result.modelHint, "first");
  assert.equal(result.apiStyle, "google_generative_language");
});

test("target-domain-only proxy candidates preserve the Google API style", () => {
  for (const text of ['targetDomain = "generativelanguage.googleapis.com"', "Browser API Proxy Client https://regional-generativelanguage.googleapis.com/path"]) {
    const result = derive("image", { bodyText: text }, null);
    assert.equal(result.transportKind, "canvas_program_ws_candidate");
    assert.equal(result.requestEnvelopeKind, "canvas_proxy_request");
    assert.equal(result.apiStyle, "google_generative_language");
    assert.equal(result.wsUrl, null);
    assert.equal(result.requestPath, null);
    assert.equal(result.operation, "image");
  }
});

test("proxy source path preserves explicit then URL fallback and allows absent app paths", () => {
  const bodyText = 'Browser API Proxy Client DEFAULT_ENDPOINT = "wss://fixture.invalid/ws"';
  for (const key of ["pageUrl", "canvasProgramUrl", "url"]) assert.equal(derive("video", { bodyText, [key]: "https://gemini.google.com" + appPath }, null).sourcePath, appPath);
  assert.equal(derive("video", { bodyText, appPath: " /app/987654abcdef ", pageUrl: "https://gemini.google.com" + appPath }, null).sourcePath, "/app/987654abcdef");
  assert.equal(derive("video", { bodyText }, null).sourcePath, null);
});

test("image invoke assembly retains proxy endpoint API model and source metadata", () => {
  const result = app.buildCanvasProgramInvokeContract("image", null, {}, { appPath, bodyText: 'Browser API Proxy Client DEFAULT_ENDPOINT = "wss://fixture.invalid/ws"; targetDomain = "fixture.generativelanguage.googleapis.com"; models/image-model' });
  assert.equal(result.target, "wss://fixture.invalid/ws");
  assert.equal(result.wsUrl, result.target);
  assert.equal(result.apiStyle, "google_generative_language");
  assert.equal(result.modelHint, "image-model");
  assert.equal(result.sourcePath, appPath);
});
