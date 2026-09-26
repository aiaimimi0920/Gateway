import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const app = await importTestableProgramHandle();
const derive = app.deriveProgramRpcInvokeCandidate;
const appPath = "/app/abcdef123456", otherPath = "/app/987654abcdef";
const capture = (type, overrides = {}) => Object.freeze({ type, label: "StreamGenerate", sourcePath: appPath, url: "https://fixture.invalid/StreamGenerate", ...overrides });
const state = (...captures) => Object.freeze({ rpcCaptures: Object.freeze(captures) });

test("standalone reuse: program RPC discovery requires an app path, supported operation and matching captures", () => {
  assert.equal(derive("video", null, state(capture("request"))), null);
  assert.equal(derive("image", { appPath }, state(capture("request"))), null);
  assert.equal(derive("music", { appPath }, null), null);
  assert.equal(derive("video", { appPath }, state(capture("event"), capture("request", { label: "other" }))), null);
});

test("standalone reuse: RPC model hints retain legacy delimiters and the first matching model", () => {
  assert.equal(app.extractModelHintFromRpcText(null), null);
  assert.equal(app.extractModelHintFromRpcText('prefix models/veo-test"; models/later'), "veo-test");
  assert.equal(app.extractModelHintFromRpcText("MODELS/music-test, extra"), "music-test");
  assert.equal(app.extractModelHintFromRpcText("models/model-test]"), "model-test");
  assert.equal(app.extractModelHintFromRpcText("model/unsupported"), null);
});

test("standalone reuse: RPC app path resolution preserves explicit and URL field precedence", () => {
  const captures = state(capture("request"));
  for (const key of ["pageUrl", "canvasProgramUrl", "url"]) {
    const result = derive("video", { [key]: "https://gemini.google.com" + appPath }, captures);
    assert.equal(result.sourcePath, appPath);
  }
  const scopedOther = capture("request", { sourcePath: otherPath, url: "https://fixture.invalid/other" });
  assert.equal(derive("video", { appPath: " " + otherPath + " ", pageUrl: "https://gemini.google.com" + appPath }, state(scopedOther, capture("request"))).requestUrl, scopedOther.url);
});

for (const operation of ["music", "video"]) {
  test(`standalone reuse: ${operation} RPC discovery prefers newest same-app captures over newer unrelated captures and batch RPCs`, () => {
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

  test(`standalone reuse: ${operation} StreamGenerate retains newest cross-path fallback independently for requests and responses`, () => {
    const response = capture("response", { sourcePath: otherPath, bodyText: "models/fallback-model", rpcId: "fallback-response" });
    const request = capture("request", { sourcePath: otherPath, url: "https://fixture.invalid/recent", bodyText: "fallback-body" });
    const result = derive(operation, { appPath }, state(capture("request", { sourcePath: otherPath, bodyText: "older" }), response, request));
    assert.equal(result.requestUrl, request.url);
    assert.equal(result.requestBody, "fallback-body");
    assert.equal(result.responseRpcId, "fallback-response");
    assert.equal(result.sourcePath, otherPath);
    assert.equal(result.modelHint, "fallback-model");
  });

  test(`standalone reuse: ${operation} response-only StreamGenerate candidates preserve absent request metadata`, () => {
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

  test(`standalone reuse: ${operation} batch RPC discovery matches id or label and falls back to request model hints`, () => {
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

  test(`standalone reuse: ${operation} batch RPCs do not use other-app captures or response-only ids as requests`, () => {
    assert.equal(derive(operation, { appPath }, state(capture("request", { label: "MUAZcd" }))), null);
    assert.equal(derive(operation, { appPath }, state(capture("request", { label: "hNvQHb", sourcePath: otherPath }), capture("response", { label: "MUAZcd", sourcePath: otherPath }))), null);
    const response = capture("response", { label: "hNvQHb", url: "https://fixture.invalid/batch-response" });
    const result = derive(operation, { appPath }, state(response));
    assert.equal(result.requestUrl, response.url);
    assert.equal(result.requestBody, null);
    assert.equal(result.requestRpcId, null);
  });

  test(`standalone reuse: ${operation} invoke assembly prefers RPC metadata over a discovered proxy WebSocket`, () => {
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

const input = '{"prompt":"synthetic action prompt","duration_seconds":8,"aspect_ratio":"16:9"}';
const actionText = `"action": "generate_video",\n"action_input": ${input},\n`;

test("standalone reuse: action extraction returns no contract for absent text", () => {
  for (const value of [null, undefined, " ", "ordinary page text"]) {
    assert.deepEqual(app.extractCanvasProgramActionContractFromText(value), { canvasProgramAction: null, canvasProgramActionInput: null });
  }
});

test("standalone reuse: action merging fills missing fields without replacing an earlier usable value", () => {
  const target = { canvasProgramAction: "generate_video", canvasProgramActionInput: null, untouched: true };
  app.mergeActionContract(target, Object.freeze({ canvasProgramAction: "ignored-later-action", canvasProgramActionInput: input }));
  app.mergeActionContract(target, { canvasProgramActionInput: "ignored-later-input" });
  app.mergeActionContract(target, null);
  app.mergeActionContract(null, { canvasProgramAction: "ignored" });
  assert.deepEqual(target, { canvasProgramAction: "generate_video", canvasProgramActionInput: input, untouched: true });
});

test("standalone reuse: action extraction retains a line-delimited object payload without its trailing comma", () => {
  assert.deepEqual(app.extractCanvasProgramActionContractFromText(actionText), {
    canvasProgramAction: "generate_video", canvasProgramActionInput: input,
  });
});

test("standalone reuse: quoted action input remains usable by escaped scalar extraction", () => {
  const contract = app.extractCanvasProgramActionContractFromText(`"action": "generate_video",\n"action_input": ${JSON.stringify(input)},\n`);
  assert.equal(contract.canvasProgramAction, "generate_video");
  assert.equal(app.extractQuotedScalar(contract.canvasProgramActionInput, ["prompt"]), "synthetic action prompt");
  assert.equal(app.extractQuotedScalar(contract.canvasProgramActionInput, ["aspect_ratio"]), "16:9");
});

test("standalone reuse: quoted scalar extraction respects alias precedence and either quote style", () => {
  assert.equal(app.extractQuotedScalar("'aspect': ' 4:3 ', \"aspect_ratio\": \"16:9\"", ["aspect_ratio", "aspect"]), "16:9");
  assert.equal(app.extractQuotedScalar("'prompt': ' synthetic text '", ["prompt"]), "synthetic text");
  assert.equal(app.extractQuotedScalar("'prompt': ''", ["prompt"]), null);
  assert.equal(app.extractQuotedScalar(null, ["prompt"]), null);
});

test("standalone reuse: number scalar extraction retains decimals and resolves aliases in caller order", () => {
  assert.equal(app.extractNumberScalar('"duration": 3, "duration_seconds": 8.5', ["duration_seconds", "duration"]), 8.5);
  assert.equal(app.extractNumberScalar("'duration': 0", ["duration"]), 0);
  for (const value of [null, '"duration": "unknown"', '"duration": -5', "unrelated"]) {
    assert.equal(app.extractNumberScalar(value, ["duration"]), null);
  }
});

test("standalone reuse: duration fallback uses total player time including whole minutes", () => {
  assert.equal(app.extractDurationSecondsFromBodyText("Player 0:03 / 1:05"), 65);
  assert.equal(app.extractDurationSecondsFromBodyText("0:00 / 12:34"), 754);
  for (const value of [null, "", "Generating your video", "0:03"]) assert.equal(app.extractDurationSecondsFromBodyText(value), null);
});

test("standalone reuse: video ready body and English controls remain insufficient", () => {
  for (const bodyText of ["Your video is ready", "视频已准备好", "视频已生成"]) {
    const snapshot = { bodyText, buttons: [{ text: "Play video", title: "Download video" }] };
    assert.equal(app.inferInvokeUiState("video", snapshot), null);
    assert.equal(app.buildCanvasProgramInvokeContract("video", null, {}, snapshot), null);
  }
});

test("standalone reuse: localized controls retain ready and generating precedence", () => {
  const buttons = [{ ariaLabel: "播放视频", title: "不使用应用，再试一次" }];
  assert.equal(app.inferInvokeUiState("video", { bodyText: "", buttons }), "video_player_ready");
  assert.equal(app.inferInvokeUiState("video", { bodyText: "Generating your video", buttons }), "video_generating");
  assert.equal(app.inferInvokeUiState("music", { bodyText: "", buttons }), "retry_without_app_visible");
});

test("standalone reuse: action parsing composes with the original invoke assembly", () => {
  const action = Object.freeze(app.extractCanvasProgramActionContractFromText(actionText));
  const snapshot = Object.freeze({ bodyText: '0:00 / 1:05 "prompt": "snapshot prompt"', buttons: Object.freeze([]) });
  const result = app.buildCanvasProgramInvokeContract("video", action, {}, snapshot, "fallback");
  assert.equal(result.actionInput, input);
  assert.equal(result.actionName, "generate_video");
  assert.equal(result.prompt, "synthetic action prompt");
  assert.equal(result.durationSeconds, 8);
  assert.equal(result.aspectRatio, "16:9");
  assert.equal(result.uiState, null);
  assert.equal(result.transportKind, null);
});

test("standalone reuse: zero duration suppresses player fallback while missing duration uses it", () => {
  const snapshot = { bodyText: '0:00 / 1:05 "prompt": "snapshot prompt"' };
  const zero = app.buildCanvasProgramInvokeContract("video", { canvasProgramActionInput: '{"duration":0}' }, {}, snapshot, "fallback");
  const missing = app.buildCanvasProgramInvokeContract("video", null, {}, snapshot, "fallback");
  assert.equal(zero.durationSeconds, null);
  assert.equal(missing.durationSeconds, 65);
  assert.equal(zero.prompt, "snapshot prompt");
  assert.equal(missing.prompt, "snapshot prompt");
});
