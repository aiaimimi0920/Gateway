import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { mediaPageUrl, mediaOperationHarness, imageAsset } from "./gemini-canvas-browser-pool.media-operation-fixtures.mjs";

const app = await importTestableScript();
const video = { kind: "video", url: "https://fixture.invalid/output.mp4", mimeType: "video/mp4" };
const audio = { kind: "audio", url: "https://fixture.invalid/output.mp3", mimeType: "audio/mpeg" };
const snapshot = (bodyText, assets = []) => ({ pageState: { url: mediaPageUrl, bodyText }, assets });
const count = (h, stage) => h.calls.filter(([name]) => name === stage).length;

for (const operation of ["image", "music", "video"]) {
  test(`${operation} never resubmits after an ambiguous template stall`, async (t) => {
    const h = mediaOperationHarness(t, app, {
      snapshots: [snapshot("ready", [operation === "image" ? imageAsset : operation === "music" ? audio : video])],
      responses: { "retry-check": [true] },
    });
    await h.run({ operation });
    assert.equal(count(h, "submit"), 1);
    assert.equal(h.calls.find(([name]) => name === "submit")[4]?.singleSubmit, true);
    assert.equal(count(h, "retry"), 0);
    assert.equal(count(h, "reset"), 1);
    assert.equal(h.stops, 1);
  });
}

test("video keeps an available result without resubmitting after a provider warning", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("Something went wrong (1155)", [video])], gates: {} });
  assert.deepEqual((await h.run({ operation: "video" })).media, [video]);
  assert.equal(count(h, "submit"), 1);
  assert.equal(count(h, "reset"), 1);
});

test("image with no final result times out without clicking send again", async (t) => {
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot("template still loading")], responses: { "retry-check": [true] },
  });
  await assert.rejects(h.run({ operation: "image" }), { code: "gemini_canvas_media_timeout" });
  assert.equal(count(h, "submit"), 1);
  assert.equal(count(h, "retry"), 0);
  assert.equal(h.stops, 1);
});

test("video failure without media returns its failure without resubmitting", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("Something went wrong (1155)")], gates: {} });
  await assert.rejects(h.run({ operation: "video" }), { code: "gemini_canvas_video_generation_failed", status: 502 });
  assert.equal(count(h, "submit"), 1);
  assert.equal(count(h, "reset"), 1);
  assert.equal(h.stops, 1);
});

test("resume never selects a template or clicks a create action", async (t) => {
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot("Choose a template"), snapshot("Create video"), snapshot("ready", [video])],
    responses: { template: [{ clicked: true, videoCreateClicked: true }], "retry-check": [true] },
  });
  await h.run({ operation: "video", resumeExistingMedia: true });
  for (const stage of ["submit", "retry", "template", "create", "mode"]) assert.equal(count(h, stage), 0, stage);
});

test("music downloads the audio track instead of accepting its video player as an audio result", async (t) => {
  const downloadedAudio = { ...audio, bodyBase64: "SUQzZml4dHVyZQ==" };
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot("Download music", [video])], downloadedAudio,
  });
  const result = await h.run({ operation: "music" });
  assert.equal(result.media[0].kind, "audio");
  assert.equal(result.media[0].mimeType, "audio/mpeg");
  assert.equal(result.media[0].bodyBase64, downloadedAudio.bodyBase64);
  assert.equal(count(h, "native-download"), 1);
});

test("music with only a download control can finish without a media URL", async (t) => {
  const downloadedAudio = { ...audio, bodyBase64: "SUQzZml4dHVyZQ==" };
  const ready = { ...snapshot("Music ready"), buttons: [{ ariaLabel: "下载音乐作品" }] };
  const h = mediaOperationHarness(t, app, { snapshots: [ready], downloadedAudio });
  const result = await h.run({ operation: "music" });
  assert.equal(result.media[0].kind, "audio");
  assert.equal(count(h, "native-download"), 1);
  for (const stage of ["retry", "play", "download"]) assert.equal(count(h, stage), 0, stage);
});

test("music without a downloadable audio track reaches a bounded failure without resubmission", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("Music ready", [video])] });
  await assert.rejects(h.run({ operation: "music" }), { code: "gemini_web_music_missing_audio" });
  assert.equal(count(h, "submit"), 1);
  assert.equal(count(h, "retry"), 0);
  assert.equal(h.stops, 1);
});
