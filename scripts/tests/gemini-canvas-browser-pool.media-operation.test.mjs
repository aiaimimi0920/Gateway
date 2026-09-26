import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { imageAsset, mediaPageUrl, mediaOperationHarness } from "./gemini-canvas-browser-pool.media-operation-fixtures.mjs";

const app = await importTestableScript();
const stages = (h, name) => h.calls.filter(([stage]) => stage === name);

function assertReleased(h, stops = 1, closes = 1) {
  assert.equal(h.stops, stops, "stop only the acquired capture, exactly once");
  assert.equal(stages(h, "close").length, closes, "close only an owned page lease, exactly once");
  assert.equal(h.closed, closes === 1);
  for (const name of ["request", "response", "websocket"]) assert.deepEqual(h.page.listeners(name), [h.inherited]);
  const state = structuredClone(h.state);
  h.page.emit("websocket", { url: () => "wss://fixture.invalid/late" });
  assert.deepEqual(h.state, state, "late traffic cannot mutate completed capture");
  if (stops && closes) assert.ok(h.calls.findIndex(([s]) => s === "stop") < h.calls.findIndex(([s]) => s === "close"));
}

test("Media native input validation rejects missing operation or prompt without page access", async () => {
  for (const args of [{ prompt: "fixture" }, { operation: "image", prompt: " " }]) {
    await assert.rejects(app.runMediaOperation(null, args), { status: 400, code: "gemini_canvas_invalid_request" });
  }
});

test("Media native unsupported operation rejects before acquiring a page", async () => {
  await assert.rejects(app.runMediaOperation(null, { operation: "unsupported", prompt: "fixture" }), {
    status: 400, code: "gemini_canvas_invalid_operation",
  });
});

for (const [operation, kind, file, mimeType, width, height, durationSeconds] of [
  ["image", "image", "image.png", "image/png", 1024, 1024, null],
  ["music", "audio", "music.wav", "audio/wav", null, null, 3.2],
  ["video", "video", "video.mp4", "video/mp4", 1280, 720, 5.4],
]) {
  test(`Media native ${operation} fixture preserves complete response without acquiring a page`, async () => {
    assert.deepEqual(await app.runMediaOperation(null, { operation, prompt: "fixture", baseUrl: "https://fixture.invalid///", requireAppPage: false }), {
      operation, pageUrl: "https://fixture.invalid/gemini-canvas-fixture", bodyText: `gemini canvas fixture ${operation} ok`,
      media: [{ kind, url: `https://fixture.invalid/fixtures/gemini-canvas/${file}`, mimeType, alt: `Gemini Canvas fixture ${operation}`, width, height, durationSeconds }],
    });
  });
}

for (const closeWhenDone of [true, false]) {
  test(`Media success preserves result and ${closeWhenDone ? "releases owned" : "retains attached"} page`, async (t) => {
    const h = mediaOperationHarness(t, app, { closeWhenDone }), result = await h.run();
    assert.deepEqual(result, {
      operation: "image", pageUrl: mediaPageUrl, bodyText: "fixture result", appPath: "/app/0123456789abcdef",
      canvasProgramInvokeContract: null, media: [{ ...imageAsset, bodyBase64: "Zml4dHVyZQ==" }], networkEvents: [], rpcCaptures: [],
    });
    assert.deepEqual(stages(h, "reset")[0].slice(2), ["https://fixture.invalid", 5000, mediaPageUrl]);
    assert.ok(h.calls.findIndex(([s]) => s === "reset") < h.calls.findIndex(([s]) => s === "capture"));
    assertReleased(h, 1, Number(closeWhenDone));
  });
}

test("Media lease acquisition failure does not close an unacquired page", async (t) => {
  const h = mediaOperationHarness(t, app, { failureAt: "lease" });
  await assert.rejects(h.run(), (error) => error === h.failure);
  assertReleased(h, 0, 0);
});

for (const failureAt of ["resolve", "reset", "capture"]) {
  for (const closeWhenDone of [true, false]) {
    test(`Media ${closeWhenDone ? "regression releases owned" : "retains attached"} lease after ${failureAt} failure`, async (t) => {
      const h = mediaOperationHarness(t, app, { failureAt, closeWhenDone });
      await assert.rejects(h.run(), (error) => error === h.failure);
      assertReleased(h, 0, Number(closeWhenDone));
    });
  }
}

for (const failureAt of ["mode", "submit", "snapshot", "action", "invoke", "select", "extract", "handle"]) {
  test(`Media ${failureAt} failure preserves the error and releases capture and page`, async (t) => {
    const h = mediaOperationHarness(t, app, { failureAt });
    await assert.rejects(h.run(), (error) => error === h.failure);
    assertReleased(h);
  });
}

test("Media timeout preserves last snapshot and capture state with bounded waits", async (t) => {
  const h = mediaOperationHarness(t, app, { snapshots: [{ pageState: { url: mediaPageUrl, bodyText: "waiting" }, assets: [] }] });
  await assert.rejects(h.run(), (error) => {
    assert.equal(error.status, 504);
    assert.equal(error.code, "gemini_canvas_media_timeout");
    assert.equal(error.bodyText, "waiting");
    assert.equal(error.captureState, h.state);
    return true;
  });
  assert.deepEqual(stages(h, "wait"), [["wait", 2000], ["wait", 2000], ["wait", 2000]]);
  assert.equal(stages(h, "snapshot").length, 3);
  assertReleased(h);
});

test("Media resume skips mode and submission while retaining reset and capture ownership", async (t) => {
  const h = mediaOperationHarness(t, app);
  await h.run({ resumeExistingMedia: true });
  assert.deepEqual([stages(h, "mode"), stages(h, "submit")], [[], []]);
  assert.equal(stages(h, "reset").length, 1);
  assertReleased(h);
});

test("Media wait failure releases capture and page without masking rejection", async (t) => {
  const h = mediaOperationHarness(t, app, { failureAt: "wait", snapshots: [{ pageState: { url: mediaPageUrl, bodyText: "waiting" }, assets: [] }] });
  await assert.rejects(h.run(), (error) => error === h.failure);
  assertReleased(h);
});
