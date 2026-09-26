import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { importTestableProbe, fixtureSession } from "./gemini-canvas-browserless.fixtures.mjs";
import { operationHarness, operationResult, inlineBody, assetBytes, plain } from "./gemini-canvas-browserless.operation-fixtures.mjs";

const app = await importTestableProbe();

for (const ok of [true, false]) {
  test(`browserless text operation preserves response content and HTTP ok=${ok}`, async () => {
    const responseJson = { candidates: [{ content: { parts: [{ text: " result " }] } }] };
    const h = operationHarness(app, { json: [operationResult(responseJson, { ok, status: ok ? 201 : 500 })] });
    const result = await h.run("probeText"), call = h.calls[0];
    assert.equal(call.context, h.context);
    assert.equal(call.label, "text.generate-content");
    assert.equal(call.url, "https://api.fixture.test/v1/models/fixture-model:generateContent");
    assert.equal(call.timeout, 45000);
    assert.equal(result.requestBody, call.body);
    assert.deepEqual(plain(result), { kind: "official_like_generate_content", requestUrl: call.url,
      requestBody: { contents: [{ role: "user", parts: [{ text: " fixture prompt " }] }] },
      status: ok ? 201 : 500, ok, responseText: "result", bodySummary: {
        name: null, done: false, error: null, hasCandidates: 1, hasGeneratedImages: 0, hasPredictions: 0, videoUri: null,
      } });
    assert.deepEqual(h.calls.map((c) => c.kind), ["json"]);
  });
}

test("browserless text missing response preserves null status and empty content", async () => {
  const h = operationHarness(app, { json: [operationResult(null, { ok: false, response: null })] });
  const result = await h.run("probeText");
  assert.equal(result.status, null);
  assert.equal(result.responseText, null);
  assert.equal(result.bodySummary, null);
  assert.equal(result.ok, false);
});

for (const mime of ["audio/L16;rate=8000", "audio/pcm;rate=16000", "audio/ogg"]) {
  test(`browserless TTS writes ordered assets for ${mime}`, async () => {
    const h = operationHarness(app, { json: [operationResult(inlineBody(mime), { status: 202 })] });
    const result = await h.run("probeTts");
    const pcm = mime !== "audio/ogg", rawPath = path.join(h.context.outDir, `tts-audio${pcm ? ".pcm" : ".ogg"}`);
    assert.equal(result.kind, "official_like_generate_content_tts");
    assert.equal(result.requestBody, h.calls[0].body);
    assert.equal(h.calls[0].label, "tts.generate-content");
    assert.equal(result.status, 202);
    assert.equal(result.ok, true);
    assert.deepEqual(plain(result.audioAsset), { mimeType: mime, bytesLength: 4, rawPath, wavPath: pcm ? path.join(h.context.outDir, "tts-audio.wav") : null });
    assert.equal(h.writes[0].filePath, rawPath);
    assert.deepEqual(h.writes[0].bytes, assetBytes);
    assert.deepEqual(h.calls.map((c) => c.kind), pcm ? ["json", "write", "write"] : ["json", "write"]);
    if (pcm) {
      assert.equal(h.writes[1].bytes.toString("ascii", 0, 4), "RIFF");
      assert.equal(h.writes[1].bytes.readUInt32LE(24), mime.includes("8000") ? 8000 : 16000);
      assert.deepEqual([...h.writes[1].bytes.subarray(44)], mime.includes("L16") ? [128, 0, 10, 255] : [...assetBytes]);
    }
  });
}

test("browserless TTS retains no-audio and failed-HTTP-with-audio result semantics", async () => {
  const missing = operationHarness(app, { json: [operationResult({}, { ok: false, response: null })] });
  const empty = await missing.run("probeTts");
  assert.equal(empty.audioAsset, null); assert.equal(empty.status, null); assert.equal(empty.ok, false);
  assert.deepEqual(missing.writes, []);
  const present = operationHarness(app, { json: [operationResult(inlineBody("audio/ogg"), { ok: false, status: 500 })] });
  const failed = await present.run("probeTts");
  assert.equal(failed.ok, false); assert.equal(failed.audioAsset.bytesLength, 4);
  assert.equal(present.writes.length, 1);
});

for (const [operation, failAt, expected] of [
  ["probeText", "json:1", ["json"]], ["probeTts", "json:1", ["json"]],
  ["probeTts", "write:1", ["json", "write"]], ["probeTts", "write:2", ["json", "write", "write"]],
]) {
  test(`browserless ${operation} propagates ${failAt} without later writes`, async () => {
    const h = operationHarness(app, { failAt, json: [operationResult(inlineBody("audio/pcm"))] });
    await assert.rejects(h.run(operation), (error) => error === h.failure);
    assert.deepEqual(h.calls.map((c) => c.kind), expected);
  });
}

test("browserless native text operation composes request and response owners with isolated archives", async (t) => {
  const outDir = await fs.mkdtemp(path.join(os.tmpdir(), "gateway-browserless-operations-"));
  const requests = [];
  const fetchMock = t.mock.method(globalThis, "fetch", async (url, init) => {
    requests.push({ url, init });
    return new Response(JSON.stringify({ candidates: [{ content: { parts: [{ text: "native fixture" }] } }] }), { status: 201 });
  });
  try {
    const result = await app.probeText({ outDir, apiBaseUrl: "https://fixture.test/v1", model: "fixture", prompt: "fixture prompt",
      timeoutMs: 1000, session: fixtureSession(), apiKey: "fixture-api-key", pageOrigin: "https://fixture.test", pageReferer: "https://fixture.test/app", locale: "en-US" });
    assert.equal(requests.length, 1);
    assert.equal(result.status, 201); assert.equal(result.responseText, "native fixture");
    assert.equal(result.ok, true);
    assert.deepEqual(JSON.parse(requests[0].init.body), result.requestBody);
    const files = await fs.readdir(outDir);
    assert.equal(files.length, 3);
    const requestFile = files.find((name) => name.endsWith(".request.json"));
    const responseFile = files.find((name) => name.endsWith(".response.json"));
    const bodyFile = files.find((name) => name.endsWith(".response.body.txt"));
    assert.deepEqual(JSON.parse(await fs.readFile(path.join(outDir, requestFile), "utf8")).body, result.requestBody);
    assert.equal(JSON.parse(await fs.readFile(path.join(outDir, responseFile), "utf8")).status, 201);
    assert.match(await fs.readFile(path.join(outDir, bodyFile), "utf8"), /native fixture/);
  } finally {
    fetchMock.mock.restore();
    assert.equal(path.dirname(outDir), os.tmpdir());
    assert.ok(path.basename(outDir).startsWith("gateway-browserless-operations-"));
    await fs.rm(outDir, { recursive: true, force: true });
  }
});
