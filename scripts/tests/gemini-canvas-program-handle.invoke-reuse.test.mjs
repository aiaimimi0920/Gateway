import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const app = await importTestableProgramHandle();
const derive = app.deriveCanvasProxyWsInvokeCandidate;
const appPath = "/app/abcdef123456";

test("standalone invoke reuse: proxy endpoint discovery preserves pattern precedence and accepts each legacy endpoint form", () => {
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

test("standalone invoke reuse: proxy contract text collection filters in snapshot then capture order without mutation", () => {
  const snapshot = Object.freeze({ bodyText: "Browser API Proxy Client body", bodyBase64: "Server WS Endpoint raw field" });
  const captures = Object.freeze([
    Object.freeze({ bodyText: "ordinary body", url: "https://fixture.invalid/proxy_request" }),
    Object.freeze({ bodyText: "targetDomain=fixture", url: "https://fixture.invalid/ignored" }),
  ]);
  assert.deepEqual(app.collectCanvasProxyContractTexts(snapshot, Object.freeze({ rpcCaptures: captures })), [snapshot.bodyText, snapshot.bodyBase64, captures[0].url, captures[1].bodyText]);
});

test("standalone invoke reuse: proxy discovery uses the first eligible text without combining later endpoints or models", () => {
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

test("standalone invoke reuse: target-domain-only proxy candidates preserve the Google API style", () => {
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

test("standalone invoke reuse: proxy source path preserves explicit then URL fallback and allows absent app paths", () => {
  const bodyText = 'Browser API Proxy Client DEFAULT_ENDPOINT = "wss://fixture.invalid/ws"';
  for (const key of ["pageUrl", "canvasProgramUrl", "url"]) assert.equal(derive("video", { bodyText, [key]: "https://gemini.google.com" + appPath }, null).sourcePath, appPath);
  assert.equal(derive("video", { bodyText, appPath: " /app/987654abcdef ", pageUrl: "https://gemini.google.com" + appPath }, null).sourcePath, "/app/987654abcdef");
  assert.equal(derive("video", { bodyText }, null).sourcePath, null);
});

test("standalone invoke reuse: image invoke assembly retains proxy endpoint API model and source metadata", () => {
  const result = app.buildCanvasProgramInvokeContract("image", null, {}, { appPath, bodyText: 'Browser API Proxy Client DEFAULT_ENDPOINT = "wss://fixture.invalid/ws"; targetDomain = "fixture.generativelanguage.googleapis.com"; models/image-model' });
  assert.equal(result.target, "wss://fixture.invalid/ws");
  assert.equal(result.wsUrl, result.target);
  assert.equal(result.apiStyle, "google_generative_language");
  assert.equal(result.modelHint, "image-model");
  assert.equal(result.sourcePath, appPath);
});

for (const [label, operation, contract, bodyText, expected] of [
  ["absent contract", "music", null, "Track details", false],
  ["ready player", "video", { uiState: "video_player_ready" }, "", true],
  ["concrete target", "image", { target: "fixture-target" }, "", true],
  ["proxy-only target", "image", { target: "fixture-target", transportKind: "canvas_program_ws_candidate", requestEnvelopeKind: "canvas_proxy_request" }, "", false],
  ["nonproxy envelope", "image", { target: "fixture-target", transportKind: "canvas_program_ws_candidate", requestEnvelopeKind: "other" }, "", true],
  ["music action", "music", { actionName: "music_generation" }, "", true],
  ["music snapshot", "music", {}, "Track details", true],
  ["video busy", "video", {}, "Please try again later", true],
  ["operation isolation", "image", {}, "video_placeholder Track details", false],
  ["music busy is not accepted", "music", {}, "Please try again later", false],
  ["video placeholder", "video", {}, "video_placeholder", true],
  ["video rate limit", "video", {}, "GETTING A LOT OF REQUESTS RIGHT NOW", true],
  ["music straight cue", "music", {}, "I've put together a 30-second electronic cue", true],
  ["music curly cue", "music", {}, "I’ve put together a 30-second electronic cue", true],
  ["partial music action", "music", {}, "music_generation", false],
  ["nonproxy transport", "image", { target: "fixture-target", transportKind: "other", requestEnvelopeKind: "canvas_proxy_request" }, "", true],
]) {
  test("standalone invoke reuse: progress " + label, () => {
    const snapshot = Object.freeze({ bodyText }), original = structuredClone({ contract, snapshot });
    if (contract) Object.freeze(contract);
    assert.equal(app.invokeContractIndicatesConcreteProgress(operation, contract, snapshot), expected);
    assert.deepEqual({ contract, snapshot }, original);
  });
}

for (const operation of ["music", "video", "image"]) {
  test("standalone invoke reuse: complete " + operation + " result with RPC proxy media and lane evidence", () => {
    const proxy = "wss://fixture.invalid/proxy", rpc = "https://fixture.invalid/StreamGenerate";
    const media = "https://fixture.invalid/result.mp4";
    const actionInput = '{"prompt":"action prompt","duration_seconds":8,"aspect_ratio":"16:9"}';
    const action = Object.freeze({ canvasProgramAction: "generate", canvasProgramActionInput: actionInput });
    const transport = Object.freeze({
      musicWsUrls: Object.freeze(["wss://fixture.invalid/music"]),
      videoInvokePaths: Object.freeze(["/fixture:predict"]),
      invokeBaseUrls: Object.freeze(["https://fixture.invalid/base"]),
    });
    const snapshot = Object.freeze({
      appPath,
      bodyText: 'Browser API Proxy Client DEFAULT_ENDPOINT = "' + proxy + '"; models/proxy-model 0:00 / 0:12',
      buttons: Object.freeze([Object.freeze({ text: operation === "music" ? "下载音乐作品" : "播放视频" })]),
      mediaNodes: Object.freeze([]),
    });
    const captures = Object.freeze({
      cookieHeader: " fallback=synthetic ",
      rpcCaptures: Object.freeze([Object.freeze({
        type: "request", label: "StreamGenerate", rpcId: "StreamGenerate", sourcePath: appPath,
        url: rpc, bodyText: "models/rpc-model", cookieHeader: " request=synthetic ",
      })]),
      videoUrls: Object.freeze([Object.freeze({ url: media, mimeType: "video/mp4" })]),
    });
    const expected = {
      operation, transportKind: operation === "image" ? "canvas_program_ws_candidate" : "program_" + operation + "_streamgenerate_candidate",
      target: operation === "image" ? proxy : media,
      wsUrl: operation === "image" ? proxy : null, apiStyle: null, requestPath: null,
      requestEnvelopeKind: operation === "image" ? "canvas_proxy_request" : "page_stream_generate_form",
      requestUrl: operation === "image" ? null : rpc, requestBody: operation === "image" ? null : "models/rpc-model",
      requestRpcId: operation === "image" ? null : "StreamGenerate", responseRpcId: null,
      sourcePath: appPath, modelHint: operation === "image" ? "proxy-model" : "rpc-model",
      cookieHeader: operation === "image" ? "fallback=synthetic" : "request=synthetic",
      targetSource: operation === "image" ? null : "network_video_response",
      targetMimeType: operation === "image" ? null : "video/mp4",
      targetCandidates: operation === "image" ? [] : app.collectPlayerReadyTargetCandidates(operation, snapshot, captures),
      actionName: "generate", actionInput, prompt: "action prompt", durationSeconds: 8, aspectRatio: "16:9",
      uiState: operation === "image" ? null : operation + "_player_ready",
    };
    const original = structuredClone({ action, transport, snapshot, captures });
    assert.deepEqual(app.buildCanvasProgramInvokeContract(operation, action, transport, snapshot, "fallback prompt", captures), expected);
    assert.deepEqual({ action, transport, snapshot, captures }, original);
  });
}
