import assert from "node:assert/strict";
import test from "node:test";
import { appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { selectCanvasHostFrame } from "../gemini-canvas-browser-pool-host-fetch.mjs";
import { executeCanvasProxyPreviewNoKeyFetch } from "../gemini-canvas-browser-pool-no-key-fetch.mjs";

const origin = "https://fixture.scf.usercontent.goog";
const wrapped = '<script data-bard-client-injected>const modelInformation = {"textModelName":"fixture"}; let googleLlmBaseApiUrls = []; const nativeFetch = window.fetch; window.fetch = (...args) => nativeFetch(...args);</script>';
const request = { url: "https://generativelanguage.googleapis.com/v1beta/models/fixture:generateContent", method: "POST", bodyText: "offline" };

async function fixture(t, preview = wrapped) {
  const f = await appFixture(t, { html: input => new URL(input.url()).origin === origin
    ? preview : `<iframe src="${origin}/preview"></iframe>` });
  const frame = f.page.frames().find(frame => frame.url().startsWith(origin));
  const requests = [];
  await f.context.route("https://generativelanguage.googleapis.com/**", async route => {
    requests.push(route.request().method());
    await route.fulfill({ contentType: "application/json", body: '{"synthetic":"ok"}',
      headers: { "access-control-allow-origin": origin, "access-control-allow-credentials": "true" } });
  });
  return { ...f, frame, requests };
}

test("real offline blob Canvas frame passes atomic identity admission exactly once", browserTest, async t => {
  const f = await fixture(t);
  await f.frame.evaluate(html => { location.href = URL.createObjectURL(new Blob([html], { type: "text/html" })); }, wrapped);
  await f.frame.waitForURL("blob:**");
  await f.frame.waitForLoadState("domcontentloaded");
  const selected = await selectCanvasHostFrame(f.page);
  assert.equal(selected.frame, f.frame);
  assert.equal(selected.identity.origin, origin);
  assert.deepEqual(f.requests, []);
  const result = await executeCanvasProxyPreviewNoKeyFetch(selected.frame,
    { ...request, canvasHostIdentity: selected.identity }, 5000);
  assert.equal(result.status, 200);
  assert.equal(result.bodyText, '{"synthetic":"ok"}');
  assert.deepEqual(f.requests, ["POST"]);
});

test("same real Frame and URL after navigation cannot reuse its previous document identity", browserTest, async t => {
  const f = await fixture(t);
  const selected = await selectCanvasHostFrame(f.page);
  const url = f.frame.url();
  await f.frame.goto(url);
  assert.equal(f.frame.isDetached(), false);
  assert.equal(f.frame.url(), url);
  const result = await executeCanvasProxyPreviewNoKeyFetch(f.frame,
    { ...request, canvasHostIdentity: selected.identity }, 5000);
  assert.equal(result.status, 503);
  assert.equal(result.errorCode, "gemini_canvas_host_identity_changed");
  assert.deepEqual(f.requests, []);
});

test("real offline Canvas rejects an unadvertised model before native network dispatch", browserTest, async t => {
  const f = await fixture(t);
  const selected = await selectCanvasHostFrame(f.page);
  const result = await executeCanvasProxyPreviewNoKeyFetch(f.frame,
    { ...request, url: request.url.replace("fixture:", "lyria-3-clip-preview:"), canvasHostIdentity: selected.identity }, 5000);
  assert.equal(result.status, 503);
  assert.equal(result.errorCode, "gemini_canvas_host_model_unavailable");
  assert.deepEqual(f.requests, []);
});

test("real offline Canvas accepts a declared image alias but never operation polling", browserTest, async t => {
  const preview = wrapped.replace('"textModelName":"fixture"',
    '"textModelName":"fixture","deprecatedGenerateImageModelNames":["gemini-2.5-flash-image"]');
  const f = await fixture(t, preview);
  const selected = await selectCanvasHostFrame(f.page);
  const result = await executeCanvasProxyPreviewNoKeyFetch(f.frame, { ...request,
    url: request.url.replace("fixture:", "gemini-2.5-flash-image:"), canvasHostIdentity: selected.identity }, 5000);
  assert.equal(result.status, 200);
  assert.deepEqual(f.requests, ["POST"]);
  const blocked = await executeCanvasProxyPreviewNoKeyFetch(f.frame, { ...request, method: "GET",
    url: "https://generativelanguage.googleapis.com/v1beta/operations/fixture", canvasHostIdentity: selected.identity }, 5000);
  assert.equal(blocked.errorCode, "gemini_canvas_host_model_unavailable");
  assert.deepEqual(f.requests, ["POST"]);
});
