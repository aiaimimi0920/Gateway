import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const video = "https://fixture.invalid/video.mp4", audio = "https://fixture.invalid/music.wav";
const collect = app.collectPlayerReadyTargetCandidates;

test("media target collection ignores empty inputs and rejects absent candidate URLs", () => {
  for (const operation of ["music", "video", "image"]) assert.deepEqual(collect(operation, null, null), []);
  assert.equal(app.scorePlayerReadyTargetCandidate("video", null), -1);
  assert.deepEqual(collect("video", { anchors: [{ href: " " }], mediaNodes: [{ src: "" }] }, { videoUrls: [null] }), []);
});

test("media target ranking prefers a captured HTTP asset over page links and blob players", () => {
  const candidates = collect("video", {
    anchors: [{ href: "https://fixture.invalid/linked.mp4" }],
    mediaNodes: [{ kind: "video", currentSrc: "blob:https://fixture.invalid/player" }],
  }, { videoUrls: [{ url: video }] });
  assert.deepEqual(candidates.map((candidate) => candidate.source), ["network_video_response", "anchor_media_href", "media_node_current_src"]);
  assert.ok(candidates[0].score > candidates[1].score && candidates[1].score > candidates[2].score);
});

test("music may use captured audio or video while video excludes captured audio", () => {
  const state = { audioUrls: [{ url: audio }], videoUrls: [{ url: video }] };
  assert.deepEqual(collect("music", {}, state).map((candidate) => candidate.url), [audio, video]);
  assert.deepEqual(collect("video", {}, state).map((candidate) => candidate.url), [video]);
});

test("duplicate media URLs retain the higher-ranked downloadable candidate metadata", () => {
  const candidates = collect("video", {
    anchorNodes: [{ href: video, download: "saved.mp4" }], mediaNodes: [{ kind: "video", src: video }],
  }, { videoUrls: [{ url: video }] });
  assert.equal(candidates.length, 1);
  assert.deepEqual(Object.keys(candidates[0]).sort(), ["download", "kind", "mimeType", "score", "source", "url"]);
  assert.equal(candidates[0].source, "anchor_download_href");
  assert.equal(candidates[0].mimeType, "video/mp4");
  assert.equal(candidates[0].kind, "video");
  assert.equal(candidates[0].download, true);
});

test("equal-ranked distinct network targets preserve newest-first order without mutating captures", () => {
  const older = Object.freeze({ url: "https://fixture.invalid/older.mp4" });
  const newer = Object.freeze({ url: "https://fixture.invalid/newer.mp4" });
  const entries = Object.freeze([older, newer]), state = Object.freeze({ videoUrls: entries });
  assert.deepEqual(collect("video", {}, state).map((candidate) => candidate.url), [newer.url, older.url]);
  assert.deepEqual(entries, [older, newer]);
});

test("media-node kind filtering uses currentSrc first and skips unrelated links", () => {
  const snapshot = Object.freeze({
    anchors: Object.freeze([Object.freeze({ href: "https://fixture.invalid/help", text: "Help" })]),
    mediaNodes: Object.freeze([
      Object.freeze({ kind: "audio", currentSrc: " blob:https://fixture.invalid/audio ", src: video }),
      Object.freeze({ kind: "image", src: video }), Object.freeze({ kind: "video", src: "https://fixture.invalid/not-media" }),
    ]),
  });
  const [candidate] = collect("music", snapshot, null);
  assert.equal(candidate.url, "blob:https://fixture.invalid/audio");
  assert.equal(candidate.kind, "audio");
  assert.equal(candidate.mimeType, "audio/wav");
  assert.deepEqual(collect("video", snapshot, null), []);
});

const rpcAudio = "https://contribution.usercontent.google.com/download?filename=music.wav&id=synthetic";
const rpcVideo = "https://contribution.usercontent.google.com/download?filename=video.mp4&id=synthetic";
function rpcBody(url) {
  return '"' + url.replaceAll("=", "\\u003d").replaceAll("&", "\\u0026") + '"';
}

for (const operation of ["music", "video"]) {
  test(`${operation} RPC targets decode escaped download URLs and exclude unrelated captures`, () => {
    const state = { rpcCaptures: [
      { type: "request", label: "StreamGenerate", bodyText: rpcBody("https://contribution.usercontent.google.com/download?filename=ignored.mp4") },
      { type: "response", label: "unrelated", bodyText: rpcBody("https://contribution.usercontent.google.com/download?filename=ignored.wav") },
      { type: "response", label: "StreamGenerate", bodyText: rpcBody(rpcAudio) },
      { type: "response", url: "https://gemini.google.com/StreamGenerate", bodyText: rpcBody(rpcVideo) },
      { type: "response", label: "StreamGenerate", bodyText: '"https://contribution.usercontent.google.com/download?filename=unknown.bin"' },
    ] };
    const candidates = collect(operation, {}, state);
    assert.deepEqual(candidates.map((candidate) => candidate.url), operation === "music" ? [rpcAudio, rpcVideo] : [rpcVideo]);
    assert.ok(candidates.every((candidate) => candidate.download && candidate.source === "stream_generate_response_body"));
    assert.equal(candidates[0].mimeType, operation === "music" ? "audio/wav" : "video/mp4");
  });
}

test("equal-ranked RPC download targets retain reverse capture order", () => {
  const newer = rpcVideo.replace("synthetic", "newer");
  const entries = Object.freeze([
    Object.freeze({ type: "response", label: "StreamGenerate", bodyText: rpcBody(rpcVideo) }),
    Object.freeze({ type: "response", label: "StreamGenerate", bodyText: rpcBody(newer) }),
  ]);
  assert.deepEqual(collect("video", {}, Object.freeze({ rpcCaptures: entries })).map((candidate) => candidate.url), [newer, rpcVideo]);
});

test("ready invoke contract selects the ranked media target while retaining transport metadata", () => {
  const state = { videoUrls: [{ url: video }] };
  const contract = app.buildCanvasProgramInvokeContract("video", null,
    { videoInvokePaths: ["/v1/models/fixture:predictLongRunning"] },
    { bodyText: "Your video is ready!", buttons: [], mediaNodes: [{ kind: "video", src: "blob:https://fixture.invalid/player" }] }, null, state);
  assert.equal(contract.target, video);
  assert.equal(contract.targetSource, "network_video_response");
  assert.equal(contract.targetMimeType, "video/mp4");
  assert.equal(contract.targetCandidates.length, 2);
  assert.equal(contract.uiState, "video_player_ready");
  assert.equal(contract.transportKind, "app_video_http");
});
