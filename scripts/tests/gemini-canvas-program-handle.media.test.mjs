import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const app = await importTestableProgramHandle();
const frozen = (items) => Object.freeze(items.map((item) => Object.freeze(item)));

for (const [url, expected] of [["audio.wav?preview=image.png", "image/png"], ["x=w0", "image/png"], ["gg-dl", "image/png"], ["x.jpeg", "image/jpeg"], ["x.webp", "image/webp"], ["x.mp4", "video/mp4"], ["x.webm", "video/webm"], ["x.wav", "audio/wav"], ["x.mp3", "audio/mpeg"], ["x.ogg", "audio/ogg"], ["x.mov", "fallback"]]) {
  test("program handle media MIME priority " + url, () => assert.equal(app.inferMimeTypeFromUrl(url, "fallback"), expected));
}

test("program handle media URL predicates preserve standalone contribution blob and mov rules", () => {
  assert.equal(app.isBlobLikeUrl(" BLOB:fixture "), true);
  assert.equal(app.isAudioLikeMimeType(" AUDIO/WAV "), true);
  assert.equal(app.isAudioLikeUrl("https://fixture.test/song.wav?x=1"), true);
  assert.equal(app.isAudioLikeUrl("https://fixture.test/song.wav#fragment"), false);
  assert.equal(app.isVideoLikeUrl("https://contribution.usercontent.google.com/download?x=1"), true);
  assert.equal(app.isVideoLikeUrl("blob:fixture"), true);
  assert.equal(app.isVideoLikeUrl("https://fixture.test/movie.mov"), false);
  assert.equal(app.isAudioLikeUrl("https://fixture.test/song.mp4"), false);
});

test("program handle media raw URL dedupe keeps first object and exact spelling", () => {
  const first = Object.freeze({ url: " x ", marker: 1 }), duplicate = Object.freeze({ url: " x ", marker: 2 }), store = [];
  app.pushUniqueMediaUrl(store, null); app.pushUniqueMediaUrl(store, first); app.pushUniqueMediaUrl(store, duplicate);
  app.pushUniqueMediaUrl(store, { url: "x" });
  assert.equal(store.length, 2); assert.equal(store[0], first);
});

test("program handle media score distinguishes URL scheme source priority and operation", () => {
  assert.equal(app.scorePlayerReadyTargetCandidate("music", { url: " " }), -1);
  assert.equal(app.scorePlayerReadyTargetCandidate("music", { url: "https://googlevideo.com/a.wav", source: "network_download", mimeType: "audio/wav" }), 419);
  assert.equal(app.scorePlayerReadyTargetCandidate("video", { url: "blob:fixture", source: "media_node", mimeType: "video/mp4" }), 240);
  assert.equal(app.scorePlayerReadyTargetCandidate("music", { url: "https://fixture.test/a.mp4", source: "network", mimeType: "video/mp4" }), 328);
});

test("program handle media summary exposes exact shape and scores original metadata", () => {
  const result = app.summarizePlayerReadyTargetCandidate("music", { url: " https://fixture.test/asset ", source: " anchor ", mimeType: " audio/wav ", download: "yes", extra: true });
  assert.deepEqual(result, { url: "https://fixture.test/asset", source: "anchor", mimeType: "audio/wav", kind: "audio", download: true, score: 260 });
  assert.equal(app.summarizePlayerReadyTargetCandidate("video", {}), null);
});

test("program handle media summarized URL dedupe replaces equal score but keeps stronger target", () => {
  const store = [];
  app.pushPlayerReadyTargetCandidate(store, "music", { url: "https://fixture.test/a.wav", source: "network", kind: "first" });
  app.pushPlayerReadyTargetCandidate(store, "music", { url: "https://fixture.test/a.wav", source: "anchor", kind: "weaker" });
  assert.equal(store[0].kind, "first");
  app.pushPlayerReadyTargetCandidate(store, "music", { url: "https://fixture.test/a.wav", source: "network", kind: "equal" });
  assert.equal(store[0].kind, "equal"); assert.equal(store.length, 1);
});

test("program handle media RPC collection decodes escaped URLs and filters request/non-stream records", () => {
  const audio = "https://contribution.usercontent.google.com/download?filename=a.wav&x=1";
  const video = "https://contribution.usercontent.google.com/download?filename=v.mp4";
  const escaped = audio.replaceAll("=", String.raw`\u003d`).replaceAll("&", String.raw`\u0026`);
  const rpcCaptures = frozen([
    { type: "response", label: "StreamGenerate", bodyText: '"' + escaped + '"' },
    { type: "response", url: "https://fixture.test/StreamGenerate", bodyText: '"' + video + '"' },
    { type: "request", label: "StreamGenerate", bodyText: '"' + audio + '"' },
    { type: "response", label: "ignored", bodyText: '"' + video + '"' },
  ]);
  const music = app.collectRpcBodyDownloadCandidates("music", { rpcCaptures });
  assert.deepEqual(music.map((x) => x.url), [video, audio]);
  assert.equal(music[1].mimeType, "audio/wav"); assert.equal(music[1].kind, "audio");
  assert.equal(music[1].source, "stream_generate_response_body"); assert.equal(music[1].download, true);
  assert.deepEqual(app.collectRpcBodyDownloadCandidates("video", { rpcCaptures }).map((x) => x.url), [video]);
});

test("program handle media equal-score network targets stay newest-first without mutation", () => {
  const audioUrls = frozen([{ url: "https://fixture.test/old.wav" }, { url: "https://fixture.test/new.wav" }]);
  const result = app.collectPlayerReadyTargetCandidates("music", {}, { audioUrls });
  assert.deepEqual(result.map((x) => x.url), [audioUrls[1].url, audioUrls[0].url]);
  assert.equal(audioUrls[0].url, "https://fixture.test/old.wav");
});

test("program handle media duplicate reversed network URLs retain later-visited metadata on ties", () => {
  const result = app.collectPlayerReadyTargetCandidates("music", {}, { audioUrls: frozen([{ url: "https://fixture.test/a.wav", mimeType: "audio/old" }, { url: "https://fixture.test/a.wav", mimeType: "audio/new" }]) });
  assert.equal(result.length, 1); assert.equal(result[0].mimeType, "audio/old");
});

test("program handle media uses anchors only and recognizes labels and download metadata", () => {
  const snapshot = { anchorNodes: frozen([{ href: "https://fixture.test/ignored.wav" }]), anchors: frozen([{ href: "https://fixture.test/a.wav" }, { href: "https://fixture.test/opaque", text: "Play" }, { href: "https://fixture.test/download", download: "asset" }]) };
  const result = app.collectPlayerReadyTargetCandidates("music", snapshot, {});
  assert.equal(result.some((x) => x.url.includes("ignored")), false);
  assert.equal(result.length, 3);
  assert.equal(result.find((x) => x.url.endsWith("download")).source, "anchor_download_href");
  assert.equal(app.collectPlayerReadyTargetCandidates("video", { anchors: [{ href: "https://fixture.test/opaque", text: "Play video" }] }, {}).length, 1);
  assert.deepEqual(app.collectPlayerReadyTargetCandidates("text", snapshot, {}), []);
});

test("program handle media currentSrc wins and kind filters preserve blob fallbacks", () => {
  const snapshot = { mediaNodes: frozen([{ kind: "audio", currentSrc: "blob:current", src: "https://fixture.test/old.wav" }, { kind: "video", src: "blob:video" }, { kind: "image", src: "https://fixture.test/ignored.mp4" }]) };
  const music = app.collectPlayerReadyTargetCandidates("music", snapshot, {});
  assert.equal(music.find((x) => x.url === "blob:current").mimeType, "audio/wav");
  assert.equal(music.length, 2);
  assert.deepEqual(app.collectPlayerReadyTargetCandidates("video", snapshot, {}).map((x) => x.url), ["blob:video"]);
});

test("program handle media full collection ranks network anchor and blob sources", () => {
  const result = app.collectPlayerReadyTargetCandidates("music", { anchors: [{ href: "https://fixture.test/anchor.wav" }], mediaNodes: [{ kind: "audio", src: "blob:audio" }] }, { audioUrls: [{ url: "https://fixture.test/network.wav" }], videoUrls: [{ url: "https://fixture.test/video.mp4" }] });
  assert.deepEqual(result.map((x) => x.source), ["network_audio_response", "anchor_media_href", "network_video_response", "media_node_current_src"]);
  assert.deepEqual(app.collectPlayerReadyTargetCandidates("video", null, { audioUrls: [{ url: "https://fixture.test/a.wav" }] }), []);
  assert.deepEqual(app.collectPlayerReadyTargetCandidates("music", null, null), []);
});
