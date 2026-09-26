import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const derive = app.deriveProgramRpcInvokeCandidate;
const appPath = "/app/abcdef123456", otherPath = "/app/987654abcdef";
const capture = (type, overrides = {}) => Object.freeze({ type, label: "StreamGenerate", sourcePath: appPath, url: "https://fixture.invalid/StreamGenerate", ...overrides });
const state = (...captures) => Object.freeze({ rpcCaptures: Object.freeze(captures) });

test("program RPC discovery requires an app path, supported operation and matching captures", () => {
  assert.equal(derive("video", null, state(capture("request"))), null);
  assert.equal(derive("image", { appPath }, state(capture("request"))), null);
  assert.equal(derive("music", { appPath }, null), null);
  assert.equal(derive("video", { appPath }, state(capture("event"), capture("request", { label: "other" }))), null);
});

test("RPC model hints retain legacy delimiters and the first matching model", () => {
  assert.equal(app.extractModelHintFromRpcText(null), null);
  assert.equal(app.extractModelHintFromRpcText('prefix models/veo-test"; models/later'), "veo-test");
  assert.equal(app.extractModelHintFromRpcText("MODELS/music-test, extra"), "music-test");
  assert.equal(app.extractModelHintFromRpcText("models/model-test]"), "model-test");
  assert.equal(app.extractModelHintFromRpcText("model/unsupported"), null);
});

test("RPC app path resolution preserves explicit and URL field precedence", () => {
  const captures = state(capture("request"));
  for (const key of ["pageUrl", "canvasProgramUrl", "url"]) {
    const result = derive("video", { [key]: "https://gemini.google.com" + appPath }, captures);
    assert.equal(result.sourcePath, appPath);
  }
  const scopedOther = capture("request", { sourcePath: otherPath, url: "https://fixture.invalid/other" });
  assert.equal(derive("video", { appPath: " " + otherPath + " ", pageUrl: "https://gemini.google.com" + appPath }, state(scopedOther, capture("request"))).requestUrl, scopedOther.url);
});

for (const operation of ["music", "video"]) {
  test(`${operation} RPC discovery prefers newest same-app captures over newer unrelated captures and batch RPCs`, () => {
    const request = capture("request", { label: null, rpcId: " StreamGenerate ", sourcePath: " " + appPath + " ", bodyText: " models/request-model ", cookieHeader: " fixture=synthetic " });
    const response = capture("response", { rpcId: "response-id", bodyText: 'models/response-model"' });
    const captures = state(capture("request", { bodyText: "older" }), request, response,
      capture("request", { sourcePath: otherPath, bodyText: "other app" }), capture("response", { sourcePath: otherPath, bodyText: "models/other-model" }),
      capture("request", { label: "hNvQHb", rpcId: "hNvQHb", bodyText: "batch" }));
    const result = derive(operation, Object.freeze({ appPath }), captures);
    assert.equal(result.transportKind, `program_${operation}_streamgenerate_candidate`);
    assert.equal(result.requestEnvelopeKind, "page_stream_generate_form");
    assert.equal(result.requestBody, "models/request-model");
    assert.equal(result.requestRpcId, "StreamGenerate");
    assert.equal(result.responseRpcId, "response-id");
    assert.equal(result.modelHint, "response-model");
    assert.equal(result.cookieHeader, "fixture=synthetic");
    assert.equal(result.sourcePath, appPath);
    assert.equal(captures.rpcCaptures[1], request);
  });

  test(`${operation} StreamGenerate retains newest cross-path fallback independently for requests and responses`, () => {
    const response = capture("response", { sourcePath: otherPath, bodyText: "models/fallback-model", rpcId: "fallback-response" });
    const request = capture("request", { sourcePath: otherPath, url: "https://fixture.invalid/recent", bodyText: "fallback-body" });
    const result = derive(operation, { appPath }, state(capture("request", { sourcePath: otherPath, bodyText: "older" }), response, request));
    assert.equal(result.requestUrl, request.url);
    assert.equal(result.requestBody, "fallback-body");
    assert.equal(result.responseRpcId, "fallback-response");
    assert.equal(result.sourcePath, otherPath);
    assert.equal(result.modelHint, "fallback-model");
  });

  test(`${operation} response-only StreamGenerate candidates preserve absent request metadata`, () => {
    const response = capture("response", { sourcePath: null, bodyText: "models/response-only", rpcId: "response-only" });
    const result = derive(operation, { appPath }, state(response));
    assert.equal(result.requestUrl, response.url);
    assert.equal(result.requestBody, null);
    assert.equal(result.requestRpcId, null);
    assert.equal(result.responseRpcId, "response-only");
    assert.equal(result.cookieHeader, null);
    assert.equal(result.sourcePath, appPath);
    assert.equal(result.modelHint, "response-only");
  });

  test(`${operation} batch RPC discovery matches id or label and falls back to request model hints`, () => {
    const request = capture("request", { label: "unrelated", rpcId: "kwDCne", url: "https://fixture.invalid/batch", bodyText: "models/request-only" });
    const response = capture("response", { label: " MUAZcd ", rpcId: null, bodyText: "no model" });
    const result = derive(operation, { appPath }, state(request, response));
    assert.equal(result.transportKind, `program_${operation}_batchexecute_candidate`);
    assert.equal(result.requestEnvelopeKind, "page_rpc_form");
    assert.equal(result.requestUrl, request.url);
    assert.equal(result.requestBody, "models/request-only");
    assert.equal(result.requestRpcId, "kwDCne");
    assert.equal(result.responseRpcId, null);
    assert.equal(result.modelHint, "request-only");
  });

  test(`${operation} batch RPCs do not use other-app captures or response-only ids as requests`, () => {
    assert.equal(derive(operation, { appPath }, state(capture("request", { label: "MUAZcd" }))), null);
    assert.equal(derive(operation, { appPath }, state(capture("request", { label: "hNvQHb", sourcePath: otherPath }), capture("response", { label: "MUAZcd", sourcePath: otherPath }))), null);
    const response = capture("response", { label: "hNvQHb", url: "https://fixture.invalid/batch-response" });
    const result = derive(operation, { appPath }, state(response));
    assert.equal(result.requestUrl, response.url);
    assert.equal(result.requestBody, null);
    assert.equal(result.requestRpcId, null);
  });

  test(`${operation} invoke assembly prefers RPC metadata over a discovered proxy WebSocket`, () => {
    const snapshot = { appPath, bodyText: 'Browser API Proxy Client DEFAULT_ENDPOINT = "ws://127.0.0.1:9998"; models/proxy-model', buttons: [], mediaNodes: [] };
    const captures = state(capture("request", { bodyText: "models/rpc-model", cookieHeader: "fixture=synthetic" }));
    const result = app.buildCanvasProgramInvokeContract(operation, null, {}, snapshot, null, captures);
    assert.equal(result.transportKind, `program_${operation}_streamgenerate_candidate`);
    assert.equal(result.requestEnvelopeKind, "page_stream_generate_form");
    assert.equal(result.target, "https://fixture.invalid/StreamGenerate");
    assert.equal(result.modelHint, "rpc-model");
    assert.equal(result.cookieHeader, "fixture=synthetic");
    assert.equal(result.wsUrl, null);
    const proxyOnly = app.buildCanvasProgramInvokeContract(operation, null, {}, snapshot, null, null);
    assert.equal(proxyOnly.modelHint, "proxy-model");
    assert.equal(proxyOnly.wsUrl, "ws://127.0.0.1:9998");
  });
}
