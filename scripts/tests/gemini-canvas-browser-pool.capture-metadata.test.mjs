import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();

for (const url of [
  "https://fixture.invalid/StreamGenerate", "https://fixture.invalid/batchexecute",
  "https://fixture.invalid/assistant.lamda.BardFrontendService", "https://clients6.google.com/punctual",
]) {
  test(`program traffic captures POST to ${url} regardless of operation`, () => {
    assert.equal(app.shouldCaptureProgramHandleTraffic(url, "post", "text"), true);
    assert.equal(app.shouldCaptureProgramHandleTraffic(url, "POST", "unrecognized"), true);
    assert.equal(app.shouldCaptureProgramHandleTraffic(url, "GET", "bootstrap_program"), false);
  });
}

for (const operation of ["image", "music", "video", "bootstrap_program"]) {
  test(`program traffic captures supported model mutations for ${operation}`, () => {
    for (const suffix of ["generateContent", "streamGenerateContent", ":predict", "/predict", "imagen"]) {
      assert.equal(app.shouldCaptureProgramHandleTraffic(`https://fixture.invalid/${suffix}`, "POST", operation), true);
    }
    assert.equal(app.shouldCaptureProgramHandleTraffic("https://fixture.invalid/ordinary", "POST", operation), false);
  });
}

test("program traffic retains strict method and operation gates for model mutations", () => {
  const url = "https://fixture.invalid/generateContent";
  for (const method of ["GET", "PUT", "PATCH", null]) assert.equal(app.shouldCaptureProgramHandleTraffic(url, method, "image"), false);
  for (const operation of ["text", "tts", "IMAGE", null]) assert.equal(app.shouldCaptureProgramHandleTraffic(url, "POST", operation), false);
});

test("RPC classification preserves the complete allowlist and source-path decoding", () => {
  for (const rpcId of ["ujx1Bf", "ESY5D", "L5adhe", "XhaU0b", "PCck7e", "aPya6c", "MaZiqc", "hNvQHb", "kwDCne", "MUAZcd", "qpEbW", "k81mDb"]) {
    const url = `https://fixture.invalid/batchexecute?rpcids=${rpcId}&source-path=%2Fshare%2Fabcd1234`;
    assert.deepEqual(app.classifyProgramRpcCapture(url, "post"), { rpcId, label: rpcId, sourcePath: "/share/abcd1234", method: "POST" });
  }
  assert.equal(app.classifyProgramRpcCapture("https://fixture.invalid/?rpcids=unknown", "POST"), null);
  assert.equal(app.classifyProgramRpcCapture("https://fixture.invalid/?rpcids=ujx1bf", "POST"), null);
});

test("RPC ID reading preserves query values and rejects malformed URLs without throwing", () => {
  assert.equal(app.readRpcIdFromRequestUrl("https://fixture.invalid/?rpcids=a%2Cb&rpcids=last"), "a,b");
  for (const url of [null, "not a URL", "/relative?rpcids=MaZiqc", "https://fixture.invalid/"]) assert.equal(app.readRpcIdFromRequestUrl(url), null);
});

test("StreamGenerate classification keeps fallback and known RPC precedence", () => {
  assert.deepEqual(app.classifyProgramRpcCapture("https://fixture.invalid/StreamGenerate?rpcids=unknown", null), {
    rpcId: null, label: "StreamGenerate", sourcePath: null, method: "GET",
  });
  assert.deepEqual(app.classifyProgramRpcCapture("https://fixture.invalid/StreamGenerate?rpcids=MaZiqc", "post"), {
    rpcId: "MaZiqc", label: "MaZiqc", sourcePath: null, method: "POST",
  });
});

test("ordinary RPC text trims at the default bound and supports explicit limits", () => {
  const text = "x".repeat(40001);
  assert.equal(app.trimProgramRpcCaptureText(text), "x".repeat(40000) + "...[truncated]");
  assert.equal(app.trimProgramRpcCaptureText("abcd", 4), "abcd");
  assert.equal(app.trimProgramRpcCaptureText("abcde", 4), "abcd...[truncated]");
  assert.equal(app.trimProgramRpcCaptureText(undefined), null);
  assert.equal(app.trimProgramRpcCaptureText(0), 0);
  const object = Object.freeze({ fixture: true });
  assert.equal(app.trimProgramRpcCaptureText(object), object);
});

for (const marker of ["Browser API Proxy Client", "Routing Google API requests via WebSocket", "System Logs Output"]) {
  test(`RPC text keeps the existing unbounded exception for ${marker}`, () => {
    const text = marker + "x".repeat(40001);
    assert.equal(app.trimProgramRpcCaptureText(text), text);
  });
}

test("server endpoint marker alone does not bypass RPC text truncation", () => {
  assert.equal(app.trimProgramRpcCaptureText("Server WS Endpoint", 6), "Server...[truncated]");
});

test("RPC capture retains the latest 80 records in its original array", (t) => {
  t.mock.method(Date, "now", () => 123456);
  const state = {};
  app.pushProgramRpcCapture(state, Object.freeze({ sequence: 0 }));
  const records = state.rpcCaptures;
  for (let sequence = 1; sequence < 85; sequence += 1) app.pushProgramRpcCapture(state, { sequence });
  assert.equal(state.rpcCaptures, records);
  assert.deepEqual(records.map((record) => record.sequence), Array.from({ length: 80 }, (_, index) => index + 5));
  assert.ok(records.every((record) => record.t === 123456));
  app.pushProgramRpcCapture(state, { sequence: 85, t: 7 });
  assert.equal(records.length, 80);
  assert.equal(records[0].sequence, 6);
  assert.equal(records.at(-1).t, 7);
});

test("cookie header capture preserves normalized casing precedence and validation", () => {
  assert.equal(app.extractRequestCookieHeader({ cookie: " fixture=one ", Cookie: "fallback=two" }), "fixture=one");
  assert.equal(app.extractRequestCookieHeader({ cookie: " ", Cookie: " fallback=two " }), "fallback=two");
  assert.equal(app.extractRequestCookieHeader({ cookie: "invalid", Cookie: "fallback=two" }), null);
  for (const headers of [null, false, "fixture=one", {}, { cookie: "invalid" }]) assert.equal(app.extractRequestCookieHeader(headers), null);
});

test("context cookie capture queries the exact origin and request URL and preserves order", async () => {
  const requestUrl = "https://fixture.invalid:8443/rpc?slot=2", calls = [];
  const page = { context: () => ({ cookies: async (urls) => {
    calls.push(urls);
    return [{ name: "fixture", value: "one" }, { name: "second", value: "two" }];
  } }) };
  assert.equal(await app.captureCookieHeaderFromContext(page, requestUrl), "fixture=one; second=two");
  assert.deepEqual(calls, [["https://fixture.invalid:8443", requestUrl]]);
});

test("context cookie capture returns null for an empty list", async () => {
  const page = { context: () => ({ cookies: async () => [] }) };
  assert.equal(await app.captureCookieHeaderFromContext(page, "https://fixture.invalid/rpc"), null);
});

test("context cookie capture rejects malformed URLs before accessing the page", async () => {
  let accessed = false;
  const page = { context() { accessed = true; throw new Error("must not be called"); } };
  assert.equal(await app.captureCookieHeaderFromContext(page, "/relative"), null);
  assert.equal(accessed, false);
});

test("context cookie capture absorbs context API rejection", async () => {
  const page = { context: () => ({ cookies: async () => { throw new Error("context closed"); } }) };
  assert.equal(await app.captureCookieHeaderFromContext(page, "https://fixture.invalid/rpc"), null);
});
