import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { imageAsset, mediaPageUrl, mediaOperationHarness } from "./gemini-canvas-browser-pool.media-operation-fixtures.mjs";

const app = await importTestableScript();
const video = { kind: "video", url: "https://fixture.invalid/video.mp4", mimeType: "video/mp4" };
const audio = { kind: "audio", url: "https://fixture.invalid/music.wav", mimeType: "audio/wav" };
const gate = { status: 429, code: "fixture_provider_quota", message: "Fixture provider quota" };
const snapshot = (bodyText = "ready", assets = []) => ({ pageState: { url: mediaPageUrl, bodyText }, assets });
const calls = (h, stage) => h.calls.filter(([name]) => name === stage);
const waits = (h) => calls(h, "wait").map(([, ms]) => ms);
const deferred = (code) => Object.assign(new Error("Fixture deferred bytes"), { code });
const runVideo = (h) => h.run({ operation: "video", timeoutMs: 60000 });
const runMusic = (h, args = {}) => h.run({ operation: "music", ...args });

function releasedAfterPolling(h) {
  assert.equal(h.stops, 1);
  assert.equal(calls(h, "close").length, 1);
  const stopIndex = h.calls.findIndex(([stage]) => stage === "stop");
  assert.ok(stopIndex > h.calls.findLastIndex(([stage]) => ["snapshot", "select", "audio", "extract", "handle"].includes(stage)));
  for (const event of ["request", "response", "websocket"]) assert.deepEqual(h.page.listeners(event), [h.inherited]);
}

test("Media polling awaits later snapshots before capture cleanup and preserves snapshot merge order", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("waiting"), snapshot("ready", [imageAsset])] });
  assert.equal((await h.run()).media[0].url, imageAsset.url);
  assert.deepEqual(waits(h), [2000]);
  const start = h.calls.findIndex(([stage]) => stage === "snapshot");
  assert.deepEqual(h.calls.slice(start, start + 5).map(([stage]) => stage), ["snapshot", "action", "build", "invoke", "select"]);
  assert.equal(calls(h, "build")[0][4].bodyText, "waiting");
  assert.equal(calls(h, "build")[0][6], h.state);
  releasedAfterPolling(h);
});

test("Media result finalization rebuilds and stores the latest invoke contract", async (t) => {
  const initialTarget = { url: "https://fixture.invalid/initial.png", kind: "image", score: 1 };
  const finalTarget = { url: "https://fixture.invalid/final.png", kind: "image", score: 2 };
  const h = mediaOperationHarness(t, app, {
    responses: {
      build: [
        { operation: "image", targetCandidates: [initialTarget] },
        { operation: "image", targetCandidates: [finalTarget] },
      ],
    },
  });

  const result = await h.run();
  const selectedTargets = result.canvasProgramInvokeContract.targetCandidates.map(({ url }) => url);
  const storedTargets = h.state.invokeContract.targetCandidates.map(({ url }) => url);
  assert.deepEqual(selectedTargets, [initialTarget.url, finalTarget.url]);
  assert.deepEqual(storedTargets, selectedTargets);
  assert.equal(calls(h, "build").length, 2);
  assert.ok(h.calls.findLastIndex(([stage]) => stage === "invoke") < h.calls.findLastIndex(([stage]) => stage === "select"));
  releasedAfterPolling(h);
});

test("Media template-stall prompt retry stops after two attempts before accepting image", async (t) => {
  const h = mediaOperationHarness(t, app, { responses: { "retry-check": [true] } });
  await h.run();
  assert.equal(calls(h, "retry").length, 2);
  assert.equal(calls(h, "snapshot").length, 3);
  assert.deepEqual(waits(h), [1200, 1200]);
  releasedAfterPolling(h);
});

test("Media video composer resets are capped and restart the prompt retry budget", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("ready", [video])], responses: { "retry-check": [true] } });
  await runVideo(h);
  assert.equal(calls(h, "retry").length, 8);
  for (const stage of ["reset", "mode", "submit"]) assert.equal(calls(h, stage).length, 4);
  assert.equal(calls(h, "snapshot").length, 12);
  assert.deepEqual(waits(h), Array(8).fill(1200));
  releasedAfterPolling(h);
});

test("Media failed video composer reactivation keeps its 409 error and cleanup", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot()], responses: { "retry-check": [true], mode: [true, false] } });
  await assert.rejects(runVideo(h), { status: 409, code: "gemini_canvas_video_mode_unavailable" });
  assert.equal(calls(h, "submit").length, 1);
  releasedAfterPolling(h);
});

test("Media video template selection attempts remain capped at three", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("Choose a template")] });
  await assert.rejects(h.run({ operation: "video", timeoutMs: 9000 }), { code: "gemini_canvas_media_timeout" });
  assert.deepEqual(calls(h, "template").map((call) => call[3]), [0, 1, 2]);
  assert.equal(calls(h, "snapshot").length, 5);
  releasedAfterPolling(h);
});

test("Media video create probes retain the three-second spacing and three-attempt cap", async (t) => {
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot("Choose a template"), ...Array.from({ length: 4 }, () => snapshot("Create video")), snapshot("ready", [video])],
    responses: { template: [{ clicked: true }], create: [true, false] },
  });
  await runVideo(h);
  assert.equal(calls(h, "create").length, 3);
  assert.deepEqual(waits(h), [1500, 2000, 2000, 2000]);
  releasedAfterPolling(h);
});

test("Media video transient failures retry three times then preserve an available result", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("Something went wrong (1155)", [video])], gates: {} });
  assert.deepEqual((await runVideo(h)).media, [video]);
  for (const stage of ["reset", "mode", "submit"]) assert.equal(calls(h, stage).length, 4);
  assert.deepEqual(waits(h), [2000, 2000, 2000]);
  releasedAfterPolling(h);
});

test("Media video transient retry resets template and create attempt state", async (t) => {
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot("Choose a template"), snapshot("Something went wrong (1155)"), snapshot("Choose a template"), snapshot("ready", [video])],
    responses: { template: [{ clicked: true, videoCreateClicked: true }] }, gates: {},
  });
  await runVideo(h);
  assert.deepEqual(calls(h, "template").map((call) => call[3]), [0, 0]);
  assert.deepEqual(waits(h), [2000]);
  releasedAfterPolling(h);
});

for (const source of ["visible quota", "snapshot", "recent traffic"]) {
  test(`Media provider gate uses ${source} and preserves status body and capture identity`, async (t) => {
    const quotaText = "已达到视频生成数量上限";
    const h = mediaOperationHarness(t, app, {
      snapshots: [snapshot(source === "snapshot" ? "gated" : "body")],
      quotaText: source === "visible quota" ? quotaText : null,
      recentGateText: source === "recent traffic" ? "gated" : "",
      gates: { gated: gate, [quotaText]: gate },
    });
    await assert.rejects(runVideo(h), (error) => {
      assert.equal(error.status, gate.status);
      assert.equal(error.code, gate.code);
      assert.equal(error.message, gate.message);
      assert.equal(error.bodyText, h.snapshots[0].pageState.bodyText);
      assert.equal(error.captureState, h.state);
      return true;
    });
    releasedAfterPolling(h);
  });
}

test("Media video provider gate defers for twenty seconds after successful template selection", async (t) => {
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot("Choose a template"), snapshot("gated")], responses: { template: [{ clicked: true }] }, gates: { gated: gate },
  });
  await assert.rejects(runVideo(h), { code: gate.code });
  assert.deepEqual(waits(h), Array(10).fill(2000));
  assert.equal(h.now, 21000);
  releasedAfterPolling(h);
});

test("Media video ready assets are not hidden by a provider gate", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("gated", [video])], gates: { gated: gate } });
  assert.deepEqual((await runVideo(h)).media, [video]);
  releasedAfterPolling(h);
});

test("Media music retains its provider gate even with a provisional audio asset", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("gated", [audio])], gates: { gated: gate } });
  await assert.rejects(runMusic(h), { code: gate.code });
  assert.equal(calls(h, "audio").length, 0);
  releasedAfterPolling(h);
});

for (const operation of ["music", "video"]) {
  test(`Media ${operation} player probes run once each and await the next snapshot`, async (t) => {
    const asset = operation === "music" ? audio : video;
    const h = mediaOperationHarness(t, app, {
      snapshots: [snapshot(), snapshot(), snapshot("ready", [asset])],
      invokeContract: { uiState: `${operation}_player_ready` }, responses: { play: [true], download: [true] },
    });
    await h.run({ operation });
    assert.equal(calls(h, "play").length, 1);
    assert.equal(calls(h, "download").length, 1);
    assert.deepEqual(waits(h), [1800, 2200]);
    releasedAfterPolling(h);
  });
}

test("Media rejected play probe falls through to download without aborting capture", async (t) => {
  const h = mediaOperationHarness(t, app, {
    snapshots: [snapshot(), snapshot("ready", [video])], invokeContract: { uiState: "video_player_ready" },
    failureAt: "play", responses: { download: [true] },
  });
  await runVideo(h);
  assert.deepEqual(waits(h), [2200]);
  releasedAfterPolling(h);
});

test("Media music settlement waits stop before crossing the deadline", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [snapshot("done", [audio])], extractionError: deferred("gemini_canvas_tts_non_audio_asset") });
  assert.deepEqual((await runMusic(h)).media, [audio]);
  assert.deepEqual(waits(h), [1600, 1600, 1600]);
  assert.equal(calls(h, "snapshot").length, 4);
  releasedAfterPolling(h);
});

test("Media music settlement does not wait when the next wait equals the deadline", async (t) => {
  const h = mediaOperationHarness(t, app, { resultTimeoutMs: 1, snapshots: [snapshot("done", [audio])], extractionError: deferred("gemini_canvas_tts_non_audio_asset") });
  await runMusic(h, { timeoutMs: 1600 });
  assert.deepEqual(waits(h), []);
  releasedAfterPolling(h);
});

for (const [label, invokeContract, pending] of [
  ["pending body", null, true],
  ["audio kind", { targetCandidates: [{ kind: "audio", url: "https://fixture.invalid/kind-target" }] }, false],
  ["audio URL", { targetCandidates: [{ url: audio.url }] }, false],
  ["audio MIME", { targetCandidates: [{ mimeType: "audio/wav", url: "https://fixture.invalid/mime-target" }] }, false],
]) {
  test(`Media music ${label} bypasses settlement waiting without inline bytes`, async (t) => {
    const h = mediaOperationHarness(t, app, {
      snapshots: [snapshot("done", [audio])], extractionError: deferred("gemini_canvas_tts_audio_fetch_failed"),
      invokeContract, responses: { "music-pending": [pending] },
    });
    await runMusic(h);
    assert.deepEqual(waits(h), []);
    releasedAfterPolling(h);
  });
}

for (const [operation, asset, code] of [
  ["image", imageAsset, "gemini_canvas_non_image_asset"],
  ["image", imageAsset, "gemini_canvas_image_fetch_failed"],
  ["music", audio, "gemini_canvas_tts_non_audio_asset"],
  ["music", audio, "gemini_canvas_tts_audio_fetch_failed"],
]) {
  test(`Media ${code} defers original asset bytes without discarding its metadata`, async (t) => {
    const h = mediaOperationHarness(t, app, { snapshots: [snapshot("done", [asset])], extractionError: deferred(code), responses: { "music-pending": [true] } });
    assert.deepEqual((await h.run({ operation })).media, [asset]);
    releasedAfterPolling(h);
  });
}

test("Media refreshed asset selection takes precedence over the initial candidate", async (t) => {
  const refreshed = { ...imageAsset, url: "https://lh3.googleusercontent.com/refreshed.png" };
  const h = mediaOperationHarness(t, app, { responses: { select: [[imageAsset], [refreshed]] } });
  assert.equal((await h.run()).media[0].url, refreshed.url);
  assert.equal(calls(h, "extract")[0][2], refreshed);
  releasedAfterPolling(h);
});

test("Media empty refresh preserves initial assets and skips mismatched inline extraction", async (t) => {
  const h = mediaOperationHarness(t, app, { responses: { select: [[video], []] } });
  assert.deepEqual((await h.run()).media, [video]);
  assert.equal(calls(h, "extract").length, 0);
  releasedAfterPolling(h);
});

test("Media missing page state retains URL fallback and null body", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [{ assets: [imageAsset] }] });
  const result = await h.run();
  assert.equal(result.pageUrl, mediaPageUrl);
  assert.equal(result.bodyText, null);
  releasedAfterPolling(h);
});
