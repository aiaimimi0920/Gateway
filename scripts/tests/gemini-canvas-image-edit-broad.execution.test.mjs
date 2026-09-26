import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { executionFixture } from "./gemini-canvas-image-edit-broad.fixtures.mjs";

const count = (f, name) => f.calls.filter(([kind]) => kind === name).length;

test("broad root preserves output schema capture getters and final artifact ordering", async (t) => {
  const f = executionFixture(t);
  assert.equal(await f.run(), undefined);
  const artifact = f.artifact(), summary = f.printed[0];
  assert.deepEqual(Object.keys(artifact).sort(), ["ok", "capturedAt", "runtimeStateObjectKey", "profileDir", "profileSource", "sampleImagePath", "prompt", "uploadTrigger", "markerTransport", "markerRequestSeenAt", "markerResponseSeenAt", "secondarySignalerPollUrl", "secondarySignalerPollSeenAt", "secondarySignalerPollCompletedAt", "finalizeUploadCapture", "imageNodes", "anchorNodes", "cssBgNodes", "canvasNodes", "buttonNodes", "pageState", "exportedPageBlobs", "pageUrl", "strongSuccessSignal", "hasGeneratedImage", "hasAiGeneratedImage", "hasUploadedPreviewImage", "hasInterestingAnchor", "hasInterestingCssBackground", "hasLargeCanvas", "hasDownloadUi", "events", "note"].sort());
  assert.equal(artifact.ok, true);
  assert.equal(artifact.profileDir, "fixture-profile");
  assert.equal(artifact.profileSource, "env-override");
  assert.equal(artifact.markerTransport.kind, "request");
  assert.equal(artifact.markerTransport.postData, "fixture-marker");
  assert.ok(artifact.markerResponseSeenAt >= artifact.markerRequestSeenAt);
  assert.equal(artifact.events.length, 2);
  assert.equal(artifact.uploadTrigger, "upload-menu-local-images-files-uploader-button");
  assert.equal(summary.exportedPageBlobCount, 0);
  assert.deepEqual(Object.keys(summary).sort(), ["ok", "outDir", "runtimeStateObjectKey", "uploadTrigger", "markerTransportKind", "markerTransportUrl", "exportedPageBlobCount", "imageNodeCount", "anchorNodeCount", "pageUrl", "strongSuccessSignal", "hasAiGeneratedImage", "hasUploadedPreviewImage"].sort());
  assert.equal(fs.readFileSync(artifact.sampleImagePath).subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
  assert.equal(fs.readFileSync(path.join(summary.outDir, "page.html"), "utf8"), "<html>fixture</html>");
  assert.deepEqual(f.calls.slice(-5).map(([kind]) => kind), ["blob evaluate", "storage state", "write image-edit-broad.json", "stdout", "close"]);
  assert.deepEqual(structuredClone(f.calls.find(([kind]) => kind === "cdp Network.enable")[1]), { maxTotalBufferSize: 128 * 1024 * 1024, maxResourceBufferSize: 32 * 1024 * 1024 });
  assert.equal(count(f, "close"), 1);
});

test("broad root uses a new page only for empty context and preserves launch settings", async (t) => {
  const f = executionFixture(t, { emptyContext: true });
  await f.run();
  assert.equal(count(f, "new page"), 1);
  const [, profile, settings] = f.calls.find(([name]) => name === "launch");
  assert.equal(profile, "fixture-profile");
  assert.deepEqual(structuredClone(settings), { executablePath: "fixture-browser", headless: false, locale: "zh-CN", args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"] });
});

test("broad root preserves uploaded-preview distinction and bounded image wait", async (t) => {
  const f = executionFixture(t, { snapshot: { imageNodes: [{ src: "blob:uploaded", alt: "上传图片的预览图", width: 512, height: 512 }] } });
  await f.run();
  const artifact = f.artifact();
  assert.equal(artifact.hasGeneratedImage, true);
  assert.equal(artifact.hasUploadedPreviewImage, true);
  assert.equal(artifact.hasAiGeneratedImage, false);
  assert.equal(artifact.strongSuccessSignal, false);
  assert.equal(artifact.ok, false);
  assert.equal(count(f, "snapshot"), 360);
});

test("broad root defers strong success until secondary poll waiting limit", async (t) => {
  const f = executionFixture(t, { secondaryPoll: true });
  await f.run();
  const artifact = f.artifact();
  assert.equal(artifact.ok, true);
  assert.ok(artifact.secondarySignalerPollUrl.endsWith("AID=7"));
  assert.equal(artifact.secondarySignalerPollCompletedAt, null);
  assert.equal(count(f, "snapshot"), 136);
});

test("broad root no-marker path keeps Enter fallback and transport timeout", async (t) => {
  const f = executionFixture(t, { noMarker: true });
  await f.run();
  assert.equal(f.artifact().markerTransport, null);
  assert.equal(f.calls.filter(([kind, key]) => kind === "key" && key === "Enter").length, 1);
  assert.equal(count(f, "snapshot"), 360);
  assert.equal(f.calls.filter(([kind, ms]) => kind === "wait" && ms === 1000).length, 301);
});

test("broad root style selection and platform keyboard contract remain ordered", async (t) => {
  const f = executionFixture(t, { styleVisible: true, platform: "darwin" });
  await f.run();
  assert.equal(f.calls.filter(([kind, key]) => kind === "key" && key === "Meta+A").length, 1);
  assert.equal(count(f, "click send"), 2);
  assert.equal(count(f, 'click img[alt*="珐琅胸针"]'), 1);
  assert.equal(f.calls.filter(([kind, key]) => kind === "key" && key === "Enter").length, 1);
});

for (const boundary of ["noBrowser", "noProfile"]) {
  test("broad root " + boundary + " retains early diagnostic exit", async (t) => {
    const f = executionFixture(t, { [boundary]: true });
    await assert.rejects(f.run(), (error) => error === f.exit);
    assert.equal(count(f, "launch"), 0);
    assert.equal(f.errors[0].ok, false);
    assert.equal(f.writes.length, 0);
  });
}

test("broad root missing source override fails before browser launch", async (t) => {
  const f = executionFixture(t, { env: { GEMINI_CANVAS_IMAGE_EDIT_SOURCE_PATH: "fixture-missing-reference.png" } });
  await assert.rejects(f.run(), (error) => error === f.exit);
  assert.equal(f.errors[0].message, "Override image not found: fixture-missing-reference.png");
  assert.equal(count(f, "launch"), 0);
});

for (const failAt of ["goto", "screenshot", "write image-edit-broad.json"]) {
  test("broad root preserves existing failure outcome at " + failAt, async (t) => {
    const f = executionFixture(t, { failAt });
    await assert.rejects(f.run(), (error) => error === f.failure);
    assert.equal(f.printed.length, 0);
    assert.equal(count(f, "close"), 1);
  });
}

test("broad root suppresses final context close rejection", async (t) => {
  const f = executionFixture(t, { failAt: "close" });
  await f.run();
  assert.equal(f.artifact().ok, true);
  assert.equal(count(f, "close"), 1);
});
