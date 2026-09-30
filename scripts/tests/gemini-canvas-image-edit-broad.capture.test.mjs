import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import test from "node:test";
import { captureFixture, requestFixture, responseFixture, dispatch } from "./gemini-canvas-image-edit-broad.fixtures.mjs";

const uploadUrl = "https://push.clients6.google.com/upload/fixture";
const finalizeHeaders = { "x-goog-upload-command": "upload, finalize" };
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const maxBodyBytes = 4 * 1024 * 1024;

async function prepareCdpResponse(f, requestId, { status = 200, headers = {}, dataLength } = {}) {
  await f.emitCdp("Network.responseReceived", { requestId, response: { status, headers } });
  if (dataLength !== undefined) await f.emitCdp("Network.dataReceived", { requestId, dataLength });
}

test("broad capture live state preserves first marker and exact response ordering", async (t) => {
  const f = captureFixture(t), events = f.capture.events;
  await f.emit("request", requestFixture({ data: "fixture-marker" }));
  const first = f.capture.markerTransport;
  assert.equal(first, events[0]);
  assert.equal(f.capture.markerRequestSeenAt, 1000);
  f.advance(50);
  await f.emit("response", responseFixture(requestFixture({ url: "https://gemini.google.com/other" })));
  assert.equal(f.capture.markerResponseSeenAt, null);
  await f.emit("response", responseFixture());
  assert.equal(f.capture.markerResponseSeenAt, 1050);
  await f.emit("request", requestFixture({ data: "fixture-marker second" }));
  assert.equal(f.capture.markerTransport, first);
  assert.equal(f.capture.events, events);
  const other = f.app.createImageEditBroadCapture({ marker: "other", outDir: f.outDir });
  assert.equal(other.events.length, 0);
  assert.equal(other.markerTransport, null);
});

test("broad capture retains the last 1600 events without losing first marker", async (t) => {
  const f = captureFixture(t);
  await f.emit("request", requestFixture({ data: "fixture-marker" }));
  const first = f.capture.markerTransport;
  for (let i = 0; i < 1602; i += 1) await f.emit("request", requestFixture({ data: String(i) }));
  assert.equal(f.capture.events.length, 1600);
  assert.equal(f.capture.events[0].postData, "2");
  assert.equal(f.capture.events.at(-1).postData, "1601");
  assert.equal(f.capture.markerTransport, first);
});

for (const event of ["request", "response"]) {
  test("broad capture " + event + " preserves method resource and hostname filters", async (t) => {
    const f = captureFixture(t);
    const cases = [
      [{ url: "https://fixture.invalid/data" }, false], [{ method: "HEAD" }, false],
      [{ method: "GET", type: "script" }, false], [{ method: "GET", type: "image" }, true],
      [{ method: "get", type: "document" }, true], [{ method: "PUT" }, true], [{ method: "PATCH" }, true],
      [{ url: "https://files.googleusercontent.com/a", method: "GET", type: "media" }, true],
    ];
    for (const [options, accepted] of cases) {
      const request = requestFixture(options), before = f.capture.events.length;
      await f.emit(event, event === "request" ? request : responseFixture(request));
      assert.equal(f.capture.events.length - before, Number(accepted), JSON.stringify(options));
    }
  });
}

for (const event of ["request", "response"]) {
  test("broad capture " + event + " retains fallback headers after allHeaders rejection", async (t) => {
    const f = captureFixture(t), headers = { "content-type": "application/json", fixture: "fallback" };
    const overrides = { headers: () => headers, allHeaders: async () => { throw f.failure; } };
    await f.emit(event, event === "request" ? requestFixture({ overrides }) : responseFixture(undefined, overrides));
    assert.equal(f.capture.events[0].headers, headers);
  });
}

test("broad capture writes exact binary finalize bytes with matching metadata", async (t) => {
  const f = captureFixture(t), body = Buffer.from([0, 255, 128, 13, 10]);
  await f.emit("request", requestFixture({ url: uploadUrl, data: "text fallback", headers: finalizeHeaders, overrides: { postDataBuffer: async () => body } }));
  const capture = f.capture.finalizeUploadCapture, event = f.capture.events[0];
  assert.equal(capture.sha256, sha256(body));
  assert.equal(capture.fileName, "upload-finalize-body-" + sha256(body).slice(0, 12) + ".bin");
  assert.deepEqual(fs.readFileSync(capture.outputPath), body);
  assert.equal(capture.byteLength, body.length);
  assert.equal(event.postData, null);
  assert.deepEqual(structuredClone(event.binaryBodyMeta), { byteLength: body.length, sha256: capture.sha256, fileName: capture.fileName, outputPath: capture.outputPath });
});

test("broad capture retains textual metadata when binary finalize reading fails", async (t) => {
  const f = captureFixture(t);
  await f.emit("request", requestFixture({ url: uploadUrl, data: "text fallback", headers: finalizeHeaders, overrides: { postDataBuffer: async () => { throw f.failure; } } }));
  assert.equal(f.capture.finalizeUploadCapture, null);
  assert.equal(f.capture.events[0].postData, "text fallback");
  assert.equal(f.capture.events[0].binaryBodyMeta, null);
  assert.deepEqual(fs.readdirSync(f.outDir), []);
});

test("broad CDP capture restores Latin1 finalize bytes before response capture", async (t) => {
  const body = Buffer.from([0, 255, 128, 1]);
  const f = captureFixture(t, { send: async (name) => name === "Network.getRequestPostData" ? { postData: body.toString("latin1") } : { body: Buffer.from("reply").toString("base64"), base64Encoded: true } });
  await f.emitCdp("Network.requestWillBeSent", { requestId: "one", request: { url: uploadUrl, method: "POST", headers: finalizeHeaders } });
  await prepareCdpResponse(f, "one", { headers: { "content-length": "5" }, dataLength: 5 });
  await f.emitCdp("Network.loadingFinished", { requestId: "one" });
  assert.deepEqual(fs.readFileSync(f.capture.finalizeUploadCapture.outputPath), body);
  assert.equal(f.capture.finalizeUploadCapture.source, "Network.getRequestPostData");
  assert.equal(f.capture.events[0].bodyText, "reply");
  assert.equal(f.capture.events[0].resourceType, "cdp");
  assert.deepEqual(f.calls.map(([name]) => name), ["Network.getRequestPostData", "Network.getResponseBody"]);
  await f.emitCdp("Network.loadingFinished", { requestId: "one" });
  assert.equal(f.calls.filter(([name]) => name === "Network.getRequestPostData").length, 1);
});

test("broad CDP capture filters unknown requests and bounds decoded response previews", async (t) => {
  const f = captureFixture(t, { send: async () => ({ body: "x".repeat(120010), base64Encoded: false }) });
  await f.emitCdp("Network.loadingFinished", { requestId: "unknown" });
  await f.emitCdp("Network.requestWillBeSent", { requestId: "other", request: { url: "https://fixture.invalid/StreamGenerate" } });
  await f.emitCdp("Network.loadingFinished", { requestId: "other" });
  await f.emitCdp("Network.requestWillBeSent", { requestId: "static", request: { url: "https://gstatic.com/icon" } });
  await f.emitCdp("Network.loadingFinished", { requestId: "static" });
  assert.equal(f.calls.length, 0);
  await f.emitCdp("Network.requestWillBeSent", { requestId: "stream", request: { url: "https://gemini.google.com/StreamGenerate", method: "POST", postData: "fixture" } });
  await prepareCdpResponse(f, "stream", { headers: { "content-length": "120010" }, dataLength: 120010 });
  await f.emitCdp("Network.loadingFinished", { requestId: "stream" });
  assert.equal(f.capture.events[0].bodyText.length, 120000);
  assert.equal(f.capture.events[0].postData, "fixture");
});

test("broad CDP capture suppresses unavailable response body errors", async (t) => {
  const f = captureFixture(t);
  await f.emitCdp("Network.requestWillBeSent", { requestId: "one", request: { url: "https://gemini.google.com/StreamGenerate" } });
  await prepareCdpResponse(f, "one", { headers: { "content-length": "1" }, dataLength: 1 });
  await f.emitCdp("Network.loadingFinished", { requestId: "one" });
  assert.equal(f.calls.length, 1);
  assert.equal(f.capture.events.length, 0);
});

test("broad CDP capture rejects unknown and cumulatively oversized body sizes before native reads", async (t) => {
  const f = captureFixture(t, { send: async () => ({ body: "unexpected", base64Encoded: false }) });
  await f.emitCdp("Network.requestWillBeSent", { requestId: "unknown-size", request: { url: "https://gemini.google.com/StreamGenerate" } });
  await prepareCdpResponse(f, "unknown-size");
  await f.emitCdp("Network.loadingFinished", { requestId: "unknown-size" });

  await f.emitCdp("Network.requestWillBeSent", { requestId: "large", request: { url: "https://gemini.google.com/StreamGenerate" } });
  await prepareCdpResponse(f, "large");
  await f.emitCdp("Network.dataReceived", { requestId: "large", dataLength: maxBodyBytes });
  await f.emitCdp("Network.dataReceived", { requestId: "large", dataLength: 1 });
  await f.emitCdp("Network.loadingFinished", { requestId: "large" });

  assert.equal(f.calls.filter(([name]) => name === "Network.getResponseBody").length, 0);
  assert.equal(f.capture.events.length, 0);
});

test("broad CDP capture drops returned bodies beyond the decoded byte limit", async (t) => {
  let reads = 0;
  const f = captureFixture(t, { send: async () => { reads += 1; return { body: "x".repeat(maxBodyBytes + 1), base64Encoded: false }; } });
  await f.emitCdp("Network.requestWillBeSent", { requestId: "understated", request: { url: "https://gemini.google.com/StreamGenerate" } });
  await prepareCdpResponse(f, "understated", { headers: { "content-length": "1" }, dataLength: 1 });
  await f.emitCdp("Network.loadingFinished", { requestId: "understated" });
  assert.equal(reads, 1);
  assert.equal(f.capture.events.length, 0);
});

test("broad capture caps concurrent CDP whole-body reads", async (t) => {
  const pendingReads = [];
  const f = captureFixture(t, { send: async () => new Promise((resolve) => pendingReads.push(resolve)) });
  const completions = [];
  for (let i = 0; i < 9; i += 1) {
    const requestId = `parallel-${i}`;
    await f.emitCdp("Network.requestWillBeSent", { requestId, request: { url: "https://gemini.google.com/StreamGenerate" } });
    await prepareCdpResponse(f, requestId, { dataLength: 5 });
    completions.push(f.emitCdp("Network.loadingFinished", { requestId }));
  }
  assert.equal(pendingReads.length, 8);
  for (const resolve of pendingReads) resolve({ body: "reply", base64Encoded: false });
  await Promise.all(completions);
  assert.equal(f.capture.events.length, 8);
});

test("broad CDP capture preserves known bodyless responses without a native read", async (t) => {
  const f = captureFixture(t);
  await f.emitCdp("Network.requestWillBeSent", { requestId: "not-modified", request: { url: "https://gemini.google.com/StreamGenerate" } });
  await prepareCdpResponse(f, "not-modified", { status: 304, headers: { "content-length": String(maxBodyBytes + 1) } });
  await f.emitCdp("Network.loadingFinished", { requestId: "not-modified" });
  assert.equal(f.calls.length, 0);
  assert.equal(f.capture.events[0].bodyText, "");
});

test("broad response capture preserves body selection and text rejection", async (t) => {
  const f = captureFixture(t);
  let reads = 0;
  for (const [url, type, length, expectedReads] of [["https://gemini.google.com/binary", "application/octet-stream", "8", 0], ["https://gemini.google.com/text", "text/plain", "0", 1], ["https://files.googleusercontent.com/image", "image/png", "8", 2]]) {
    await f.emit("response", responseFixture(requestFixture({ url }), { allHeaders: async () => ({ "content-type": type, "content-length": length }), text: async () => { reads += 1; throw f.failure; } }));
    assert.equal(reads, expectedReads);
    assert.equal(f.capture.events.at(-1).bodyText, null);
  }
});

test("broad response consumer accepts native-admitted bodies independently of declared length", async (t) => {
  const f = captureFixture(t);
  let reads = 0;
  for (const length of [undefined, "invalid", String(maxBodyBytes + 1)]) {
    const headers = { "content-type": "text/plain" };
    if (length !== undefined) headers["content-length"] = length;
    await f.emit("response", responseFixture(requestFixture(), { allHeaders: async () => headers, text: async () => { reads += 1; return "response"; } }));
    assert.equal(f.capture.events.at(-1).bodyText, "response");
  }
  assert.equal(reads, 3);
});

test("broad response capture validates the returned UTF-8 body after a misleading length", async (t) => {
  const f = captureFixture(t);
  let reads = 0;
  await f.emit("response", responseFixture(requestFixture(), {
    allHeaders: async () => ({ "content-type": "text/plain", "content-length": "0" }),
    text: async () => { reads += 1; return "x".repeat(maxBodyBytes + 1); },
  }));
  assert.equal(reads, 1);
  assert.equal(f.capture.events.at(-1).bodyText, null);
});

test("broad capture records websocket phases and bounded payloads with marker precedence", async (t) => {
  const f = captureFixture(t), socket = new EventEmitter();
  socket.url = () => "wss://fixture.invalid/socket";
  await f.emit("websocket", socket);
  await dispatch(socket, "framesent", { payload: "fixture-marker" + "x".repeat(7000) });
  await dispatch(socket, "framereceived", { payload: Buffer.from("reply") });
  await dispatch(socket, "close");
  assert.deepEqual(Array.from(f.capture.events, (event) => event.phase), ["open", "framesent", "framereceived", "close"]);
  assert.equal(f.capture.events[1].payload.length, 6000);
  assert.equal(f.capture.events[2].payload, "reply");
  assert.equal(f.capture.markerTransport, f.capture.events[1]);
  assert.equal(f.capture.markerRequestSeenAt, null);
});

test("broad capture tracks positive secondary signaler polls and exact completion URLs", async (t) => {
  const f = captureFixture(t), base = "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?AID=";
  for (const aid of ["0", "-1", "01"]) await f.emit("request", requestFixture({ url: base + aid }));
  assert.equal(f.capture.secondarySignalerPollUrl, null);
  await f.emit("request", requestFixture({ url: base + "12" }));
  assert.equal(f.capture.secondarySignalerPollSeenAt, 1000);
  f.advance(50);
  await f.emit("response", responseFixture(requestFixture({ url: base + "13" })));
  assert.equal(f.capture.secondarySignalerPollCompletedAt, null);
  await f.emit("response", responseFixture(requestFixture({ url: base + "12" })));
  assert.equal(f.capture.secondarySignalerPollCompletedAt, 1050);
});
