import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";
import { operationHarness, operationResult, doneVideo, assetBytes, plain } from "./gemini-canvas-browserless.operation-fixtures.mjs";

const app = await importTestableProbe();
const videoUri = "https://download.fixture.test/video";

test("browserless video invoke URL helper preserves absolute relative and default paths", () => {
  for (const [value, expected] of [["", "https://api.test/v1/models/m:predictLongRunning"],
    [" /operations/create ", "https://api.test/v1/operations/create"], ["custom", "https://api.test/v1/custom"],
    [" https://override.test/create ", "https://override.test/create"], ["http://override.test/create", "http://override.test/create"]]) {
    assert.equal(app.buildVideoInvokeUrl("https://api.test/v1///", value, "m"), expected);
  }
});

test("browserless quota detection preserves status and message boundaries", () => {
  for (const status of [400, 403, 429]) for (const message of ["quota", "paid plans", "BILLING", "resource_exhausted", "rate limit", "rate limits"]) assert.equal(app.looksLikeQuotaOrPlanGate(status, message), true);
  for (const [status, message] of [[200, "quota"], [500, "billing"], ["429", "quota"], [429, "origin mismatch"], [403, null]]) assert.equal(app.looksLikeQuotaOrPlanGate(status, message), false);
});

for (const [label, browserState, expected] of [
  ["contract", { canvasProgramInvokeContract: { requestUrl: " https://override.test/contract " }, videoInvokePath: "/ignored" }, "https://override.test/contract"],
  ["relative path", { videoInvokePath: "custom" }, "https://api.fixture.test/v1/custom"],
  ["absolute path", { videoInvokePath: "http://override.test/path" }, "http://override.test/path"],
  ["default", {}, "https://api.fixture.test/v1/models/fixture-model:predictLongRunning"],
]) {
  test(`browserless video ${label} URL reaches preflight and create unchanged`, async () => {
    const h = operationHarness(app, { context: { browserState }, json: [operationResult(doneVideo())] });
    const result = await h.run("probeVideoCreate");
    assert.equal(result.requestUrl, expected); assert.equal(result.ok, true);
    assert.deepEqual(h.calls.map((c) => c.kind), ["exact", "json"]);
    for (const call of h.calls) { assert.equal(call.url, expected); assert.equal(call.body, result.requestBody); assert.equal(call.context, h.context); }
    assert.deepEqual(plain(result.requestBody), { instances: [{ prompt: " fixture prompt " }], parameters: { aspectRatio: "9:16", durationSeconds: 8 } });
  });
}

test("browserless video exact xd3 retries three times with each recovery delay", async () => {
  const exact = Array.from({ length: 3 }, () => operationResult({ error: { message: "Origin doesn't match host for xd3" } }, { ok: false, status: 400 }));
  const h = operationHarness(app, { exact, json: [operationResult(doneVideo())] });
  const result = await h.run("probeVideoCreate");
  assert.equal(result.ok, true);
  assert.deepEqual(h.sleeps, [750, 750, 750]);
  assert.deepEqual(h.calls.map((c) => c.kind), ["exact", "sleep", "exact", "sleep", "exact", "sleep", "json"]);
  assert.deepEqual(h.calls.filter((c) => c.kind === "exact").map((c) => c.label), ["video.create.exact-01", "video.create.exact-02", "video.create.exact-03"]);
});

test("browserless video exact non-xd3 errors stop preflight retries and still create", async () => {
  const h = operationHarness(app, { exact: [operationResult({ error: { message: "fixture other failure" } }, { ok: false })], json: [operationResult(doneVideo())] });
  await h.run("probeVideoCreate");
  assert.deepEqual(h.calls.map((c) => c.kind), ["exact", "json"]);
});

test("browserless video exact quota gate retains its request and null polling fields", async () => {
  const exact = operationResult({ error: { message: "quota" } }, { ok: false, status: 429 });
  const h = operationHarness(app, { exact: [exact] });
  const result = await h.run("probeVideoCreate");
  assert.equal(result.error, "video_official_gate"); assert.equal(result.ok, false);
  assert.equal(result.requestBody, exact.request.body); assert.equal(result.requestUrl, exact.request.url);
  assert.equal(result.status, 429); assert.equal(result.asset, null);
  assert.equal(result.finalPollSummary, null); assert.equal(result.pollRecords, null);
  assert.deepEqual(h.calls.map((c) => c.kind), ["exact"]);
});

test("browserless video missing operation name retains missing-response status", async () => {
  const h = operationHarness(app, { json: [operationResult({ name: " " }, { response: null, ok: false })] });
  const result = await h.run("probeVideoCreate");
  assert.equal(result.error, "video_operation_name_missing"); assert.equal(result.status, null);
  assert.deepEqual(Object.keys(result).sort(), ["bodySummary", "error", "kind", "ok", "requestBody", "requestUrl", "status"]);
  assert.deepEqual(h.calls.map((c) => c.kind), ["exact", "json"]);
});

for (const [name, operationUrl] of [[" /operations/fixture ", "https://api.fixture.test/v1/operations/fixture"], ["https://ops.fixture.test/job", "https://ops.fixture.test/job"]]) {
  test(`browserless video polling normalizes operation ${name.trim()}`, async () => {
    const h = operationHarness(app, { json: [operationResult({ name }, { status: 202 })], poll: [operationResult({ name }), operationResult(doneVideo(videoUri))], bytes: [operationResult({}, { headers: { "content-type": " video/webm " } })] });
    const result = await h.run("probeVideoCreate");
    assert.equal(result.operationUrl, operationUrl); assert.equal(result.status, 202); assert.equal(result.ok, true);
    assert.deepEqual(h.sleeps, [5000, 5000]);
    const polls = h.calls.filter((c) => c.kind === "poll");
    assert.deepEqual(polls.map((c) => [c.label, c.url, c.timeout]), [["video.poll-01", operationUrl, 30000], ["video.poll-02", operationUrl, 30000]]);
    assert.deepEqual(plain(result.pollRecords.map((r) => [r.status, r.ok, r.bodySummary.done])), [[200, true, false], [200, true, true]]);
    assert.equal(h.calls.find((c) => c.kind === "bytes").timeout, 120000);
    assert.deepEqual(plain(result.asset), { url: videoUri, contentType: "video/webm", bytesLength: 4, assetPath: path.join(h.context.outDir, "video-asset.webm") });
    assert.equal(h.writes[0].bytes, assetBytes);
  });
}

test("browserless video poll failure retains collected history and stops download", async () => {
  const h = operationHarness(app, { json: [operationResult({ name: "operations/fixture" })], poll: [operationResult({}, { ok: false, status: 503 })] });
  const result = await h.run("probeVideoCreate");
  assert.equal(result.error, "video_poll_failed"); assert.equal(result.ok, false);
  assert.equal(result.pollRecords.length, 1); assert.equal(result.pollRecords[0].status, 503);
  assert.deepEqual(h.calls.map((c) => c.kind), ["exact", "json", "sleep", "poll"]);
  assert.equal(Object.hasOwn(result, "asset"), false);
});

test("browserless video deadline preserves the poll after its fixed sleep", async () => {
  const h = operationHarness(app, { context: { videoPollTimeoutMs: 1, timeoutMs: 1000 }, json: [operationResult({ name: "operations/fixture" })] });
  const result = await h.run("probeVideoCreate");
  assert.deepEqual(h.sleeps, [5000]); assert.equal(result.pollRecords.length, 1);
  assert.equal(h.calls.find((c) => c.kind === "poll").timeout, 1000);
  assert.equal(result.ok, false); assert.equal(result.asset, null);
});

test("browserless video expired budget can still download a supplied URI without claiming completion", async () => {
  const h = operationHarness(app, { context: { videoPollTimeoutMs: 0 }, json: [operationResult({ ...doneVideo(videoUri), done: false })] });
  const result = await h.run("probeVideoCreate");
  assert.equal(result.ok, false); assert.equal(result.asset.bytesLength, 4);
  assert.equal(h.calls.find((c) => c.kind === "bytes").timeout, 0);
  assert.deepEqual(h.calls.map((c) => c.kind), ["exact", "json", "bytes", "write"]);
});

test("browserless video completed error and failed asset download retain separate outcomes", async () => {
  const errored = operationHarness(app, { json: [operationResult({ ...doneVideo(), error: { code: 7 } })] });
  assert.equal((await errored.run("probeVideoCreate")).ok, false);
  const download = operationHarness(app, { json: [operationResult(doneVideo(videoUri))], bytes: [operationResult({}, { ok: false })] });
  const result = await download.run("probeVideoCreate");
  assert.equal(result.ok, true); assert.equal(result.asset, null); assert.deepEqual(download.writes, []);
});

for (const failAt of ["exact:1", "json:1", "sleep:1", "poll:1", "bytes:1", "write:1"]) {
  test(`browserless video propagates ${failAt} without later work`, async () => {
    const h = operationHarness(app, { failAt, json: [operationResult({ name: "operations/fixture" })], poll: [operationResult(doneVideo(videoUri))] });
    await assert.rejects(h.run("probeVideoCreate"), (error) => error === h.failure);
    const order = ["exact", "json", "sleep", "poll", "bytes", "write"], failedKind = failAt.split(":")[0];
    assert.deepEqual(h.calls.map((c) => c.kind), order.slice(0, order.indexOf(failedKind) + 1));
  });
}
