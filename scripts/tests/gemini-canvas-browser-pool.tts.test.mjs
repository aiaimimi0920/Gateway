import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { ElementFixture } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";
import { ttsHarness } from "./gemini-canvas-browser-pool.tts-fixtures.mjs";

const app = await importTestableScript();
const stages = (h, name) => h.calls.filter(([stage]) => stage === name);

test("TTS real entry rejects blank prompts before fixture or page access", async () => {
  for (const prompt of [undefined, " ", 12]) {
    await assert.rejects(app.runTtsOperation(null, { prompt, baseUrl: "http://127.0.0.1:4200", requireAppPage: false }), (error) => {
      assert.deepEqual([error.status, error.code], [400, "gemini_canvas_invalid_tts_prompt"]);
      return true;
    });
  }
});

test("TTS real fixture bypass produces normalized synthetic Ogg without an entry", async () => {
  const result = await app.runTtsOperation(null, { prompt: "  hello  ", baseUrl: "http://127.0.0.1:4200///", requireAppPage: false });
  assert.deepEqual(Object.keys(result).sort(), ["bodyBase64", "bodyText", "media", "mimeType", "operation", "pageUrl"]);
  assert.equal(result.operation, "tts");
  assert.equal(result.pageUrl, "http://127.0.0.1:4200/gemini-canvas-fixture");
  assert.equal(result.mimeType, "audio/ogg");
  assert.deepEqual(result.media, []);
  const bytes = Buffer.from(result.bodyBase64, "base64");
  assert.equal(bytes.length, 128);
  assert.ok(bytes.toString("utf8").startsWith("OggSgemini canvas fixture tts::hello"));
});

test("TTS diagnostics cap collections and retain summaries without audio bytes", () => {
  const list = (length) => Array.from({ length }, (_, index) => index);
  const result = app.buildTtsDiagnostics({
    captureState: { audioUrls: list(20), events: list(150), streamGenerateRequestAt: 10, streamGenerateResponseAt: 20 },
    lastSnapshot: { pageState: { url: "fixture", title: "title", bodyText: "body" }, mediaNodes: list(100), anchorNodes: list(60) },
    lastButtonSnapshot: list(140), clicked: true, pollCount: 7,
    asset: { url: "audio", mimeType: "audio/ogg", durationSeconds: 3, bodyBase64: "unlogged-bytes" },
    audio: { mimeType: "audio/ogg", bodyBase64: "unlogged-bytes" },
  });
  assert.deepEqual([result.buttons.length, result.mediaNodes.length, result.anchorNodes.length, result.audioUrls[0], result.events[0]], [120, 80, 40, 8, 30]);
  assert.deepEqual(result.selectedAsset, { url: "audio", mimeType: "audio/ogg", durationSeconds: 3 });
  assert.deepEqual(result.audioResult, { mimeType: "audio/ogg", bodyBase64Length: 14 });
  assert.equal(JSON.stringify(result).includes("unlogged-bytes"), false);
  assert.deepEqual([result.clicked, result.pollCount, result.streamGenerateRequestAt, result.streamGenerateResponseAt], [true, 7, 10, 20]);
});

test("TTS diagnostics tolerate absent snapshots and malformed collections", () => {
  const result = app.buildTtsDiagnostics({ captureState: { audioUrls: {}, events: null }, lastButtonSnapshot: "bad", audio: { bodyBase64: 1 } });
  assert.deepEqual([result.pageUrl, result.title, result.bodyText, result.selectedAsset, result.pollCount], [null, null, null, null, null]);
  assert.deepEqual([result.buttons, result.mediaNodes, result.anchorNodes, result.audioUrls, result.events], [[], [], [], [], []]);
  assert.equal(result.audioResult.bodyBase64Length, 0);
});

test("TTS Listen candidates preserve role named and text priority", () => {
  const h = ttsHarness(app);
  assert.deepEqual(app.listenControlCandidates(h.page), h.candidates);
});

test("TTS Listen click preserves fixed visibility wait click cap and scroll tolerance", async () => {
  const h = ttsHarness(app, { scrollFailure: true });
  assert.equal(await app.clickListenControlWithFallback(h.page, h.candidates[0], 90000), true);
  assert.deepEqual(stages(h, "listen-wait"), [["listen-wait", "role", { state: "visible", timeout: 1200 }]]);
  assert.deepEqual(stages(h, "listen-click"), [["listen-click", "role", { timeout: 10000 }]]);
});

test("TTS Listen click serializes intercepted-pointer fallback with original event order", async () => {
  const node = new ElementFixture({ clickable: true });
  const h = ttsHarness(app, { clickFailure: true, handleNode: node });
  assert.equal(await app.clickListenControlWithFallback(h.page, h.candidates[0], 25), true);
  assert.deepEqual(node.events, ["pointerdown", "mousedown", "mouseup", "click", "click"]);
  assert.equal(node.nativeClicks, 1);
});

test("TTS Listen failure returns false for invisible control or absent DOM handle", async () => {
  for (const options of [{ visibleIndex: -1 }, { clickFailure: true }]) {
    const h = ttsHarness(app, options);
    assert.equal(await app.clickListenControlWithFallback(h.page, h.candidates[0], 25), false);
  }
});

test("TTS execution normalizes prompt resets attached page and returns audio handle schema", async () => {
  const h = ttsHarness(app, { visibleIndex: 1 });
  const result = await h.run({ includeDiagnostics: "true" });
  assert.deepEqual(result.media, [{ kind: "audio", url: "https://fixture.invalid/audio.wav", mimeType: "audio/ogg", durationSeconds: 5 }]);
  assert.deepEqual([result.operation, result.pageUrl, result.mimeType, result.bodyBase64, result.appPath, result.conversationId], ["tts", "https://fixture.invalid/current", "audio/ogg", "fixture-audio", "/app/fixture", "fixture-conversation"]);
  assert.deepEqual(structuredClone(stages(h, "reset")[0].slice(2)), ["https://gemini.google.com", 3000, "https://fixture.invalid/preferred", { skipInitialNavigationWhenAppSurfaceReady: true }]);
  assert.deepEqual(stages(h, "submit")[0].slice(2), ["fixture prompt", 3000]);
  assert.deepEqual(stages(h, "listen-wait").map(([, name]) => name), ["role", "named"]);
  assert.equal(result.diagnostics.clicked, true);
  assert.equal(result.diagnostics.pollCount, 1);
  assert.equal(stages(h, "stop").length, 1);
  assert.deepEqual(h.calls.at(-1), ["stop"]);
});

test("TTS execution omits diagnostics by default and falls back to current page URL", async () => {
  const h = ttsHarness(app, { snapshot: { pageState: {}, mediaNodes: [], anchorNodes: [] } });
  const result = await h.run();
  assert.equal(result.pageUrl, "https://fixture.invalid/page-fallback");
  assert.equal(result.bodyText, null);
  assert.equal(Object.hasOwn(result, "diagnostics"), false);
  assert.equal(stages(h, "stop").length, 1);
});

test("TTS operation DOM fallback clicks the last matching Listen target", async () => {
  const first = new ElementFixture(), last = new ElementFixture();
  const h = ttsHarness(app, { visibleIndex: -1, domControls: [first, last] });
  assert.equal((await h.run()).operation, "tts");
  assert.equal(first.nativeClicks, 0);
  assert.equal(last.nativeClicks, 1);
  assert.equal(stages(h, "stop").length, 1);
});

test("TTS missing Listen deadline returns bounded diagnostics and stops capture", async () => {
  const h = ttsHarness(app, { visibleIndex: -1, buttonFailure: true });
  await assert.rejects(h.run(), (error) => {
    assert.deepEqual([error.status, error.code], [500, "gemini_canvas_tts_control_missing"]);
    const diagnostics = JSON.parse(error.bodyText);
    assert.equal(diagnostics.clicked, false);
    assert.equal(diagnostics.pollCount, 2);
    assert.deepEqual(diagnostics.buttons, []);
    return true;
  });
  assert.equal(h.now, 3000);
  assert.equal(stages(h, "stop").length, 1);
});

test("TTS audio deadline returns timeout diagnostics after Listen success and stops capture", async () => {
  const h = ttsHarness(app, { assets: [] });
  await assert.rejects(h.run(), (error) => {
    assert.deepEqual([error.status, error.code], [504, "gemini_canvas_tts_timeout"]);
    assert.equal(JSON.parse(error.bodyText).clicked, true);
    return true;
  });
  assert.deepEqual(stages(h, "wait"), [["wait", 2000], ["wait", 2000]]);
  assert.equal(stages(h, "stop").length, 1);
});

test("TTS non-audio payload retries while other extraction failures remain fatal", async () => {
  const nonAudio = Object.assign(new Error("not audio"), { code: "gemini_canvas_tts_non_audio_asset", mimeType: "text/html" });
  const h = ttsHarness(app, { extractions: [nonAudio, { mimeType: "audio/wav", bodyBase64: "retry-audio" }] });
  assert.equal((await h.run()).bodyBase64, "retry-audio");
  assert.deepEqual(stages(h, "wait"), [["wait", 1200]]);
  assert.equal(stages(h, "extract").length, 2);
  assert.equal(stages(h, "stop").length, 1);
});

for (const [failureAt, expectedStops] of [["reset", 0], ["capture", 0], ["submit", 1], ["snapshot", 1], ["extract", 1], ["handle", 1]]) {
  test(`TTS ${failureAt} dependency failure preserves primary error and capture ownership`, async () => {
    const failure = new Error(`fixture ${failureAt} failed`), h = ttsHarness(app, { failureAt, failure });
    await assert.rejects(h.run(), (error) => error === failure);
    assert.equal(stages(h, "stop").length, expectedStops);
  });
}
