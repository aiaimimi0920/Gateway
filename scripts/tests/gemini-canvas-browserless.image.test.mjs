import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";
import { operationHarness, operationResult, inlineBody, assetBytes, plain } from "./gemini-canvas-browserless.operation-fixtures.mjs";

const app = await importTestableProbe();
const labels = ["clients6_signed_app_text_image", "clients6_signed_share_text_image", "clients6_signed_share_image_only",
  "clients6_plain_share_text_image", "clients6_plain_minimal_image_only", "google_api_official", "google_api_imagen4_predict"];
const imagenBody = () => ({ predictions: [{ bytesBase64Encoded: assetBytes.toString("base64"), mimeType: "image/webp" }] });

for (const status of [400, 403, 429]) {
  test(`browserless image exact gate ${status} precedes an official asset`, async () => {
    const exact = operationResult({ error: { message: "fixture billing" } }, { ok: false, status, text: "billing" });
    const h = operationHarness(app, { json: [operationResult(inlineBody())], exact: [exact] });
    const result = await h.run("probeImage");
    assert.deepEqual(h.calls.map((c) => c.kind), ["json", "exact"]);
    assert.equal(result.error, "image_official_gate"); assert.equal(result.ok, false);
    assert.equal(result.imageAsset, null); assert.equal(result.status, status);
    assert.equal(result.requestBody, exact.request.body); assert.equal(result.requestUrl, exact.request.url);
    assert.equal(h.calls[0].body, h.calls[1].body);
  });
}

test("browserless image official JSON gate short-circuits without an exact gate", async () => {
  const h = operationHarness(app, { json: [operationResult({ error: { message: " RESOURCE_EXHAUSTED " } }, { status: 403, ok: false })] });
  const result = await h.run("probeImage");
  assert.equal(result.error, "image_official_gate"); assert.equal(result.imageAsset, null);
  assert.deepEqual(h.calls.map((c) => c.kind), ["json", "exact"]);
});

for (const [label, responseJson, ok, extension] of [
  ["inline priority", { ...imagenBody(), ...inlineBody() }, true, ".png"],
  ["Imagen fallback on failed HTTP", imagenBody(), false, ".webp"],
]) {
  test(`browserless image official ${label} preserves asset and response identity`, async () => {
    const official = operationResult(responseJson, { ok, status: ok ? 200 : 500 });
    const h = operationHarness(app, { json: [official] });
    const result = await h.run("probeImage");
    assert.deepEqual(h.calls.map((c) => c.kind), ["json", "exact", "write"]);
    assert.equal(result.requestBody, official.request.body); assert.equal(result.ok, ok);
    assert.equal(result.error, null);
    assert.equal(result.imageAsset.assetPath, path.join(h.context.outDir, "image-asset" + extension));
    assert.equal(result.imageAsset.bytesLength, 4); assert.deepEqual(h.writes[0].bytes, assetBytes);
  });
}

for (let target = 0; target < labels.length; target += 1) {
  test(`browserless image fallback stops at asset from ${labels[target]}`, async () => {
    const replies = labels.map((_, index) => operationResult(index === target ? target === 6 ? imagenBody() : inlineBody() : {}));
    const h = operationHarness(app, { custom: replies });
    const result = await h.run("probeImage");
    const attempts = h.calls.filter((c) => c.kind === "custom");
    assert.deepEqual(attempts.map((c) => c.label), labels.slice(0, target + 1).map((label) => "image." + label));
    assert.equal(result.ok, true);
    assert.equal(result.kind, target === 6 ? "imagen_predict_image" : "official_like_generate_content_image");
    assert.equal(result.requestBody, attempts.at(-1).body); assert.equal(result.requestUrl, attempts.at(-1).url);
    assert.equal(h.writes.length, 1); assert.deepEqual(h.writes[0].bytes, assetBytes);
    for (const attempt of attempts) { assert.equal(attempt.context, h.context); assert.equal(attempt.timeout, 45000); }
  });
}

test("browserless image attempts retain signature referer modality and predict contracts", async () => {
  const h = operationHarness(app);
  const result = await h.run("probeImage");
  const calls = h.calls.filter((c) => c.kind === "custom"), attempts = calls.map((c) => c.attempts[0]);
  const appUrl = h.context.browserState.canvasProgramUrl, shareUrl = h.context.browserState.shareUrl;
  assert.deepEqual(plain(attempts.map((a) => [a.label, a.includeSignedHeaders, a.preserveCrossOriginOrigin, a.preserveCrossOriginReferer, a.signedOriginOverride, a.refererOverride])), [
    [labels[0], true, true, true, h.context.pageOrigin, appUrl],
    [labels[1], true, true, true, h.context.pageOrigin, shareUrl],
    [labels[2], true, true, true, h.context.pageOrigin, shareUrl],
    [labels[3], false, true, true, null, shareUrl],
    [labels[4], false, false, false, null, null],
    [labels[5], false, false, true, null, appUrl],
    [labels[6], false, false, true, null, appUrl],
  ]);
  assert.equal(attempts[0].requestBody, h.calls[0].body);
  assert.equal(attempts[1].requestBody, attempts[0].requestBody);
  assert.equal(attempts[3].requestBody, attempts[0].requestBody);
  assert.equal(attempts[4].requestBody.contents, attempts[0].requestBody.contents);
  assert.deepEqual(plain(attempts[2].requestBody.generationConfig.responseModalities), ["IMAGE"]);
  assert.equal(attempts[4].requestBody.generationConfig.responseModalities, undefined);
  assert.deepEqual(plain(attempts[6].requestBody), { instances: [{ prompt: " fixture prompt " }], parameters: { sampleCount: 1, aspectRatio: "9:16" } });
  assert.equal(calls[6].url, "https://api.fixture.test/v1/models/imagen-4.0-generate-001:predict");
  assert.equal(result.ok, false); assert.equal(result.imageAsset, null);
  assert.equal(result.kind, "imagen_predict_image"); assert.equal(result.requestBody, calls[6].body);
});

test("browserless image missing material URLs use page referer and base app fallbacks", async () => {
  const h = operationHarness(app, { context: { browserState: {} }, custom: [operationResult(), operationResult(inlineBody())] });
  await h.run("probeImage");
  const attempts = h.calls.filter((c) => c.kind === "custom").map((c) => c.attempts[0]);
  assert.equal(attempts[0].refererOverride, "https://page.fixture.test/app");
  assert.equal(attempts[1].refererOverride, h.context.pageReferer);
});

test("browserless image rejects assets on failed fallback responses and retains final failure", async () => {
  const replies = labels.map(() => operationResult(inlineBody(), { ok: false, status: 503 }));
  const h = operationHarness(app, { custom: replies });
  const result = await h.run("probeImage");
  assert.equal(result.ok, false); assert.equal(result.imageAsset, null); assert.equal(result.status, 503);
  assert.equal(result.kind, "official_like_generate_content_image");
  assert.equal(result.requestBody, h.calls.at(-1).body); assert.deepEqual(h.writes, []);
});

for (const [failAt, expected] of [["json:1", ["json"]], ["exact:1", ["json", "exact"]],
  ["custom:1", ["json", "exact", "custom"]], ["write:1", ["json", "exact", "custom", "write"]]]) {
  test(`browserless image propagates ${failAt} and stops later work`, async () => {
    const h = operationHarness(app, { failAt, custom: [operationResult(inlineBody())] });
    await assert.rejects(h.run("probeImage"), (error) => error === h.failure);
    assert.deepEqual(h.calls.map((c) => c.kind), expected);
  });
}
