import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();

test("media capture URL admission retains provider matching and empty input handling", () => {
  for (const domain of ["gemini.google.com", "generativelanguage.googleapis.com", "storage.googleapis.com", "geminiweb-pa.clients6.google.com", "files.googleusercontent.com", "contribution.usercontent.google.com", "rr.googlevideo.com", "gvt1.com"]) {
    assert.equal(app.isInterestingNetworkUrl(`https://${domain}/fixture`), true, domain);
  }
  assert.equal(app.isInterestingNetworkUrl("HTTPS://GEMINI.GOOGLE.COM/fixture"), true);
  for (const url of [null, undefined, "", "https://fixture.invalid/media.png"]) assert.equal(app.isInterestingNetworkUrl(url), false);
});

test("media avatar and noise filtering retains existing URL markers", () => {
  for (const url of ["https://lh3.googleusercontent.com/a/fixture", "https://lh3.googleusercontent.com/u/0/ogw", "https://fixture.invalid/image=s64-c"]) {
    assert.equal(app.isLikelyAvatarUrl(url), true, url);
  }
  for (const marker of ["googleadservices.com", "/pagead/", "1p-conversion", "doubleclick.net", "google-analytics.com"]) {
    assert.equal(app.isLikelyNoiseMediaUrl(`https://fixture.invalid/${marker}`), true, marker);
  }
  const asset = "https://files.googleusercontent.com/generated.png";
  assert.equal(app.isLikelyAvatarUrl(asset), false);
  assert.equal(app.isLikelyNoiseMediaUrl(asset), false);
  assert.equal(app.isLikelyAvatarUrl(null), false);
  assert.equal(app.isLikelyNoiseMediaUrl(undefined), false);
});

test("media MIME inference preserves extension precedence and caller fallback", () => {
  for (const [suffix, mime] of [[".PNG", "image/png"], ["=w0", "image/png"], ["/gg-dl/file", "image/png"], [".jpg", "image/jpeg"], [".jpeg", "image/jpeg"], [".webp", "image/webp"], [".mp4", "video/mp4"], [".webm", "video/webm"], [".wav", "audio/wav"], [".mp3", "audio/mpeg"], [".ogg", "audio/ogg"]]) {
    assert.equal(app.inferMimeTypeFromUrl(`https://fixture.invalid/asset${suffix}`, "application/octet-stream"), mime, suffix);
  }
  assert.equal(app.inferMimeTypeFromUrl("https://fixture.invalid/audio.wav?preview=image.png", "unknown"), "image/png");
  assert.equal(app.inferMimeTypeFromUrl("https://fixture.invalid/opaque", "audio/custom"), "audio/custom");
  assert.equal(app.inferMimeTypeFromUrl(null, undefined), undefined);
});

test("media blob and audio MIME classification preserve whitespace and case handling", () => {
  assert.equal(app.isBlobLikeUrl(" BLOB:https://fixture.invalid/id "), true);
  assert.equal(app.isBlobLikeUrl("https://fixture.invalid/blob:id"), false);
  assert.equal(app.isBlobLikeUrl(null), false);
  assert.equal(app.isAudioLikeMimeType(" AUDIO/WAV; codecs=1 "), true);
  assert.equal(app.isAudioLikeMimeType("application/audio"), false);
  assert.equal(app.isAudioLikeMimeType(null), false);
});

test("media audio and video URL classification preserves distinct and shared candidates", () => {
  for (const [url, audio, video] of [
    ["https://fixture.invalid/clip.WAV?download=1", true, false],
    ["https://fixture.invalid/clip.mp4?download=1", false, true],
    ["https://fixture.invalid/clip.mov", false, true],
    ["https://fixture.invalid/get?filename=track.mp3&x=1", true, false],
    ["https://fixture.invalid/get?filename=clip.webm&x=1", false, true],
    ["https://rr.googlevideo.com/opaque", true, true],
    ["https://gvt1.com/opaque", true, true],
    ["https://fixture.invalid/clip.wav.txt", false, false], [null, false, false],
  ]) {
    assert.equal(app.isAudioLikeUrl(url), audio, String(url));
    assert.equal(app.isVideoLikeUrl(url), video, String(url));
  }
});

test("media asset normalization preserves signed and opaque values while trimming input", () => {
  for (const value of ["https://files.googleusercontent.com/a?sig=A%2Fb&x=1#part", "blob:https://fixture.invalid/id", "not a URL", "http://fixture.invalid/asset"]) {
    assert.equal(app.normalizeGeminiBrowserAssetUrl(`  ${value}  `), value);
  }
  for (const value of ["", "  ", null, undefined, 3, {}]) assert.equal(app.normalizeGeminiBrowserAssetUrl(value), null);
});

test("media dedupe preserves first result and candidate metadata without mutating inputs", () => {
  const store = [], metadata = { source: "fixture" };
  const candidate = { url: "  https://files.googleusercontent.com/asset.png  ", mimeType: "image/png", bodyBase64: "AQID", metadata };
  app.pushUniqueMediaUrl(store, candidate);
  app.pushUniqueMediaUrl(store, { url: candidate.url.trim(), mimeType: "image/jpeg", bodyBase64: "BAUG" });
  assert.equal(store.length, 1);
  assert.deepEqual(store[0], { ...candidate, url: candidate.url.trim() });
  assert.notEqual(store[0], candidate);
  assert.equal(store[0].metadata, metadata);
  assert.equal(candidate.url, "  https://files.googleusercontent.com/asset.png  ");
});

test("media dedupe rejects missing, avatar and tracking candidates without changing the store", () => {
  const existing = { url: "https://files.googleusercontent.com/kept.png" }, store = [existing];
  for (const candidate of [null, {}, { url: " " }, { url: "https://lh3.googleusercontent.com/a/user" }, { url: "https://fixture.invalid/pagead/pixel" }]) {
    app.pushUniqueMediaUrl(store, candidate);
  }
  assert.deepEqual(store, [existing]);
  assert.equal(store[0], existing);
});
