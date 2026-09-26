import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const build = app.buildCanvasProgramInvokeContract;
const appPath = "/app/abcdef123456";
const mediaUrl = "https://fixture.invalid/result.mp4";
const rpcUrl = "https://fixture.invalid/StreamGenerate";
const capture = (cookieHeader = null) => ({ type: "request", label: "StreamGenerate", sourcePath: appPath, url: rpcUrl, bodyText: "models/synthetic", cookieHeader });

test("invoke assembly omits unsupported empty evidence while retaining the image default lane", () => {
  for (const operation of ["music", "video", "unknown"]) assert.equal(build(operation, null, null, null), null);
  const image = build("image", null, null, null);
  assert.equal(image.transportKind, "app_image_http_candidate");
  assert.equal(image.target, null);
  assert.deepEqual(image.targetCandidates, []);
});

test("invoke assembly uses action parameters before snapshot and fallback values", () => {
  const input = '{"prompt":"action prompt","duration_seconds":8,"aspect_ratio":"16:9"}';
  const action = Object.freeze({ canvasProgramAction: "generate", canvasProgramActionInput: input });
  const snapshot = Object.freeze({ bodyText: '"prompt":"snapshot prompt"\n0:00 / 0:30', buttons: Object.freeze([]) });
  const result = build("video", action, null, snapshot, "fallback prompt");
  assert.equal(result.actionName, "generate");
  assert.equal(result.actionInput, input);
  assert.equal(result.prompt, "action prompt");
  assert.equal(result.durationSeconds, 8);
  assert.equal(result.aspectRatio, "16:9");
});

test("invoke assembly falls back from snapshot prompt and player time to normalized caller prompt", () => {
  const snapshot = { bodyText: '"prompt":"snapshot prompt"\n0:00 / 0:12', buttons: [] };
  const result = build("music", null, null, snapshot, "fallback");
  assert.equal(result.prompt, "snapshot prompt");
  assert.equal(result.durationSeconds, 12);
  assert.equal(build("music", null, null, null, "  caller prompt  ").prompt, "caller prompt");
});

test("invoke assembly retains action evidence but emits null for zero or unparsed negative duration", () => {
  for (const duration of [0, -3]) {
    const result = build("video", { canvasProgramAction: "generate", canvasProgramActionInput: JSON.stringify({ duration_seconds: duration }) }, null, null);
    assert.equal(result.actionName, "generate");
    assert.equal(result.durationSeconds, null);
  }
});

test("music invoke assembly picks the newest lane without mutating transport arrays", () => {
  const urls = Object.freeze(["wss://fixture.invalid/old", "wss://fixture.invalid/new"]);
  const result = build("music", null, Object.freeze({ musicWsUrls: urls }), null);
  assert.equal(result.transportKind, "app_music_ws");
  assert.equal(result.target, urls[1]);
  assert.deepEqual(urls, ["wss://fixture.invalid/old", "wss://fixture.invalid/new"]);
});

test("video invoke paths take precedence over newest base URLs with independent fallback", () => {
  const paths = Object.freeze(["/old:predict", "/new:predict"]), bases = Object.freeze(["https://fixture.invalid/old", "https://fixture.invalid/new"]);
  assert.equal(build("video", null, Object.freeze({ videoInvokePaths: paths, invokeBaseUrls: bases }), null).target, paths[1]);
  assert.equal(build("video", null, Object.freeze({ invokeBaseUrls: bases }), null).target, bases[1]);
});

for (const operation of ["music", "video"]) {
  test(`${operation} ready media replaces the RPC target while keeping RPC request metadata`, () => {
    const result = build(operation, null, {}, { appPath, bodyText: operation === "music" ? "0:00 / 0:12" : "Your video is ready!", buttons: operation === "music" ? [{ ariaLabel: "下载音乐作品" }] : [], mediaNodes: [] }, null,
      { rpcCaptures: [capture()], videoUrls: [{ url: mediaUrl }] });
    assert.equal(result.target, mediaUrl);
    assert.equal(result.requestUrl, rpcUrl);
    assert.equal(result.requestBody, "models/synthetic");
    assert.equal(result.transportKind, `program_${operation}_streamgenerate_candidate`);
    assert.equal(result.targetSource, "network_video_response");
    assert.equal(result.targetMimeType, "video/mp4");
  });
}

test("generating invoke assembly retains the RPC target even when candidate media was captured", () => {
  const result = build("video", null, {}, { appPath, bodyText: "Generating your video", buttons: [], mediaNodes: [] }, null,
    { rpcCaptures: [capture()], videoUrls: [{ url: mediaUrl }] });
  assert.equal(result.target, rpcUrl);
  assert.equal(result.targetCandidates[0].url, mediaUrl);
  assert.equal(result.uiState, "video_generating");
});

test("invoke cookie fallback prefers the selected request then normalized capture state", () => {
  const snapshot = { appPath, bodyText: "", buttons: [] };
  const state = { rpcCaptures: [capture("request=synthetic")], cookieHeader: " fallback=synthetic " };
  assert.equal(build("video", null, {}, snapshot, null, state).cookieHeader, "request=synthetic");
  assert.equal(build("video", null, {}, snapshot, null, { ...state, rpcCaptures: [capture(" ")] }).cookieHeader, "fallback=synthetic");
});
