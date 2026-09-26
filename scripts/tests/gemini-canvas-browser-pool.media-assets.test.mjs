import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const audio = "https://fixture.invalid/music.wav";
const video = "https://fixture.invalid/video.mp4";
const image = "https://lh3.googleusercontent.com/generated.png";
const snapshot = (fields = {}) => ({ mediaNodes: [], anchorNodes: [], ...fields });
const capture = (fields = {}) => ({ audioUrls: [], imageUrls: [], videoUrls: [], rpcCaptures: [], ...fields });
const audioNode = { kind: "audio", src: "blob:fixture-audio", duration: 12 };
const videoNode = { kind: "video", src: video, width: 640, height: 480, duration: 8 };

test("asset audio chooses newest compatible capture before links RPC and DOM", () => {
  const state = capture({ rpcCaptures: [{ type: "response", label: "StreamGenerate", bodyText: '"https://contribution.usercontent.google.com/download?filename=rpc.wav"' }], audioUrls: [
    { url: audio }, { url: "https://fixture.invalid/latest", mimeType: "audio/ogg", bodyBase64: "fixture-bytes" },
    { url: "https://fixture.invalid/irrelevant", mimeType: "text/plain" },
  ] });
  const page = snapshot({ mediaNodes: [audioNode], anchorNodes: [{ href: "blob:anchor" }] });
  assert.deepEqual(app.selectAudioAsset(page, state), {
    url: "https://fixture.invalid/latest", mimeType: "audio/ogg", bodyBase64: "fixture-bytes", durationSeconds: 12,
  });
});

test("asset audio honors newest labelled or blob link before DOM", () => {
  const page = snapshot({ mediaNodes: [audioNode], anchorNodes: [
    { href: audio }, { href: "https://fixture.invalid/download", ariaLabel: "Download music" },
    { href: "https://fixture.invalid/help", text: "Help" },
  ] });
  assert.equal(app.selectAudioAsset(page, capture()).url, "https://fixture.invalid/download");
  page.anchorNodes.push({ href: "blob:last-link" });
  assert.equal(app.selectAudioAsset(page, capture()).url, "blob:last-link");
});

test("asset audio decodes latest StreamGenerate response download before DOM fallback", () => {
  const url = "https://contribution.usercontent.google.com/download?filename=music.mp3&tag=fixture";
  const encoded = url.replaceAll("=", "\\u003d").replaceAll("&", "\\u0026");
  const state = capture({ rpcCaptures: [
    { type: "response", label: "StreamGenerate", bodyText: '"https://contribution.usercontent.google.com/download?filename=old.wav"' },
    { type: "response", url: "https://fixture.invalid/StreamGenerate", bodyText: `"${encoded}"` },
    { type: "request", label: "StreamGenerate", bodyText: '"ignored"' },
    { type: "response", label: "OtherRpc", bodyText: '"ignored"' },
  ] });
  assert.deepEqual(app.selectAudioAsset(snapshot({ mediaNodes: [audioNode] }), state), {
    url, mimeType: "audio/mpeg", durationSeconds: 12,
  });
});

test("asset audio DOM fallback uses currentSrc and only finite numeric duration", () => {
  for (const [duration, expected] of [[7, 7], [Infinity, null], ["7", null]]) {
    const page = snapshot({ mediaNodes: [{ ...audioNode, currentSrc: audio, duration }] });
    assert.deepEqual(app.selectAudioAsset(page, capture()), { url: audio, mimeType: "audio/wav", durationSeconds: expected });
  }
});

test("asset audio rejects incompatible candidates and absent playable nodes", () => {
  const page = snapshot({ mediaNodes: [{ kind: "image", src: audio }], anchorNodes: [{ href: "https://fixture.invalid/help" }] });
  assert.equal(app.selectAudioAsset(page, capture({ audioUrls: [{ url: video }] })), null);
});

test("asset invoke audio applies explicit finite score and filters non-audio targets", () => {
  assert.deepEqual(app.selectAudioAssetFromInvokeContract({ target: audio, targetCandidates: [
    { url: video, score: 9999 }, { url: "https://fixture.invalid/winner", mimeType: "audio/ogg", score: 999 },
    { url: " ", mimeType: "audio/wav", score: 10000 },
  ] }), { url: "https://fixture.invalid/winner", mimeType: "audio/ogg", durationSeconds: null });
});

test("asset invoke audio uses shared scorer for unscored network candidates", () => {
  assert.equal(app.selectAudioAssetFromInvokeContract({ target: audio, targetCandidates: [
    { url: "https://fixture.invalid/network.wav", source: "network_download", score: Infinity },
  ] }).url, "https://fixture.invalid/network.wav");
});

test("asset invoke audio preserves direct target on equal score", () => {
  const score = app.scorePlayerReadyTargetCandidate("music", { url: audio, kind: "audio", source: "invoke_contract_target" });
  assert.equal(app.selectAudioAssetFromInvokeContract({ target: audio, targetCandidates: [
    { url: "https://fixture.invalid/tied.wav", score },
  ] }).url, audio);
});

test("asset invoke audio returns null for empty or incompatible contracts", () => {
  for (const contract of [null, {}, { target: video, targetCandidates: [{ url: " " }] }]) {
    assert.equal(app.selectAudioAssetFromInvokeContract(contract), null);
  }
});

test("asset image prefers newest generated capture and keeps DOM metadata and bytes", () => {
  const page = snapshot({ mediaNodes: [{ kind: "image", src: image, alt: "generated", width: 512, height: 256 }] });
  const state = capture({ imageUrls: [
    { url: image }, { url: "http://lh3.googleusercontent.com/latest.webp", mimeType: "image/png", bodyBase64: "image-bytes" },
    { url: "https://fixture.invalid/unrelated.png" },
  ] });
  assert.deepEqual(app.selectImageAssets(page, state), [{
    kind: "image", url: "https://lh3.googleusercontent.com/latest.webp", mimeType: "image/webp", bodyBase64: "image-bytes",
    alt: "generated", width: 512, height: 256, durationSeconds: null,
  }]);
});

test("asset image DOM fallback excludes avatar noise and undersized nodes", () => {
  const page = snapshot({ mediaNodes: [
    { kind: "image", src: "https://lh3.googleusercontent.com/a/avatar", width: 512 },
    { kind: "image", src: "https://googleusercontent.com/pagead/banner", width: 512 },
    { kind: "image", src: image, width: 255, height: 255 },
    { kind: "image", src: "http://lh3.googleusercontent.com/valid.png", width: 256 },
  ] });
  assert.deepEqual(app.selectImageAssets(page, capture()), [{
    kind: "image", url: "https://lh3.googleusercontent.com/valid.png", mimeType: "image/png", bodyBase64: null,
    alt: null, width: 256, height: null, durationSeconds: null,
  }]);
});

test("asset explicitly generated image can bypass ordinary size and host filtering", () => {
  const page = snapshot({ mediaNodes: [{ kind: "image", src: "https://fixture.invalid/small.jpg", alt: "AI 生成", width: 1 }] });
  assert.equal(app.selectImageAssets(page, capture())[0].url, "https://fixture.invalid/small.jpg");
});

test("asset video selects newest download-labelled anchor before DOM node", () => {
  const page = snapshot({ mediaNodes: [videoNode], anchorNodes: [
    { href: video }, { href: "https://fixture.invalid/download", title: "Download video" },
    { href: "https://fixture.invalid/help", text: "Help" },
  ] });
  assert.deepEqual(app.selectVideoAsset(page), [{
    kind: "video", url: "https://fixture.invalid/download", mimeType: "video/mp4", alt: null,
    width: null, height: null, durationSeconds: null,
  }]);
});

test("asset video DOM fallback prefers currentSrc and sanitizes numeric metadata", () => {
  assert.deepEqual(app.selectVideoAsset(snapshot({ mediaNodes: [{
    ...videoNode, currentSrc: "https://fixture.invalid/current.webm", width: Infinity, height: "480", duration: NaN,
  }] })), [{
    kind: "video", url: "https://fixture.invalid/current.webm", mimeType: "video/webm", alt: null,
    width: null, height: null, durationSeconds: null,
  }]);
});

test("asset video ignores unrelated media and unsupported node URLs", () => {
  assert.deepEqual(app.selectVideoAsset(snapshot({ mediaNodes: [
    { kind: "audio", src: video }, { kind: "video", src: "https://fixture.invalid/page" },
  ] })), []);
});

test("asset music prioritizes captured audio over contract and video and keeps result schema", () => {
  const state = capture({ audioUrls: [{ url: audio, bodyBase64: "source-bytes" }], invokeContract: { target: "https://fixture.invalid/contract.wav" }, videoUrls: [{ url: video }] });
  assert.deepEqual(app.selectMediaAssetsForOperation("music", snapshot({ mediaNodes: [audioNode, videoNode] }), state), [{
    kind: "audio", url: audio, mimeType: "audio/wav", alt: null, width: null, height: null, durationSeconds: 12,
  }]);
});

test("asset music prioritizes contract audio over both network and DOM video", () => {
  const state = capture({ invokeContract: { target: audio }, videoUrls: [{ url: "https://fixture.invalid/network.mp4" }] });
  assert.equal(app.selectMediaAssetsForOperation("music", snapshot({ mediaNodes: [videoNode] }), state)[0].url, audio);
});

test("asset music prioritizes network video over DOM video when no audio exists", () => {
  const state = capture({ videoUrls: [{ url: "https://fixture.invalid/network.mp4" }] });
  assert.equal(app.selectMediaAssetsForOperation("music", snapshot({ mediaNodes: [videoNode] }), state)[0].url, "https://fixture.invalid/network.mp4");
});

test("asset video operation prioritizes DOM video over network video", () => {
  const state = capture({ videoUrls: [{ url: "https://fixture.invalid/network.mp4" }] });
  const [asset] = app.selectMediaAssetsForOperation("video", snapshot({ mediaNodes: [videoNode] }), state);
  assert.equal(asset.url, video);
  assert.deepEqual([asset.width, asset.height, asset.durationSeconds], [640, 480, 8]);
});

test("asset video network fallback chooses newest nonempty URL with supplied MIME", () => {
  const state = capture({ videoUrls: [{ url: video }, { url: "https://fixture.invalid/latest", mimeType: "video/webm" }, { url: "" }] });
  const [asset] = app.selectMediaAssetsForOperation("video", snapshot(), state);
  assert.equal(asset.url, "https://fixture.invalid/latest");
  assert.equal(asset.mimeType, "video/webm");
});

test("asset operation dispatch returns empty results when all candidate sources are empty", () => {
  for (const operation of ["image", "music", "video"]) {
    assert.deepEqual(app.selectMediaAssetsForOperation(operation, snapshot(), capture()), []);
  }
});

test("asset selection does not mutate shared snapshot capture or contract candidates", () => {
  const freeze = (value) => {
    for (const child of Object.values(value)) if (child && typeof child === "object") freeze(child);
    return Object.freeze(value);
  };
  const page = freeze(snapshot({ mediaNodes: [audioNode, videoNode, { kind: "image", src: image, width: 256 }] }));
  const state = freeze(capture({
    audioUrls: [{ url: audio }], imageUrls: [{ url: image }], videoUrls: [{ url: video }],
    invokeContract: { targetCandidates: [{ url: audio, score: 1 }, { url: "https://fixture.invalid/best.wav", score: 2 }] },
  }));
  const before = structuredClone({ page, state });
  for (const operation of ["image", "music", "video"]) app.selectMediaAssetsForOperation(operation, page, state);
  app.selectAudioAssetFromInvokeContract(state.invokeContract);
  assert.deepEqual({ page, state }, before);
});
