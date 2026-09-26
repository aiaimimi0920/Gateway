import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const app = await importTestableProgramHandle();

test("standalone capture metadata truncation preserves values and caller limits", () => {
  const object = {};
  assert.equal(app.trimProgramRpcCaptureText(object), object);
  assert.equal(app.trimProgramRpcCaptureText(undefined), null);
  assert.equal(app.trimProgramRpcCaptureText("abcd", 4), "abcd");
  assert.equal(app.trimProgramRpcCaptureText("abcde", 4), "abcd...[truncated]");
  assert.equal(app.trimProgramRpcCaptureText("x".repeat(40001)).length, 40014);
});

for (const marker of ["Browser API Proxy Client", "Routing Google API requests via WebSocket", "System Logs Output"]) {
  test("standalone capture metadata retains complete proxy text for " + marker, () => {
    const text = "x".repeat(40001) + marker;
    assert.equal(app.trimProgramRpcCaptureText(text, 2), text);
  });
}

test("standalone capture metadata keeps the last eighty entries and caller timestamps", (t) => {
  t.mock.method(Date, "now", () => 42);
  const state = {};
  for (let id = 0; id < 82; id += 1) app.pushProgramRpcCapture(state, { id });
  assert.equal(state.rpcCaptures.length, 80);
  assert.deepEqual(state.rpcCaptures[0], { t: 42, id: 2 });
  app.pushProgramRpcCapture(state, { id: 82, t: 99 });
  assert.deepEqual(state.rpcCaptures.at(-1), { t: 99, id: 82 });
  assert.equal(state.rpcCaptures[0].id, 3);
});

test("standalone capture metadata cookie header precedence remains case specific", () => {
  assert.equal(app.extractRequestCookieHeader({ cookie: " a=b ", Cookie: "c=d" }), "a=b");
  assert.equal(app.extractRequestCookieHeader({ cookie: " ", Cookie: "c=d" }), "c=d");
  assert.equal(app.extractRequestCookieHeader({ cookie: "invalid", Cookie: "c=d" }), null);
  for (const headers of [null, false, "a=b", { COOKIE: "a=b" }, { cookie: 12 }]) {
    assert.equal(app.extractRequestCookieHeader(headers), null);
  }
});

test("standalone capture metadata context cookies preserve origin port and order", async () => {
  const url = "https://fixture.invalid:9443/StreamGenerate?source-path=%2Fapp%2Fabc";
  const page = { context: () => ({ async cookies(urls) {
    assert.deepEqual(urls, ["https://fixture.invalid:9443", url]);
    return [{ name: "one", value: "first" }, { name: "two", value: "second" }];
  } }) };
  assert.equal(await app.captureCookieHeaderFromContext(page, url), "one=first; two=second");
});

test("standalone capture metadata context cookie failures remain null", async () => {
  assert.equal(await app.captureCookieHeaderFromContext(null, "invalid"), null);
  assert.equal(await app.captureCookieHeaderFromContext({ context() { throw new Error("closed"); } }, "https://fixture.invalid"), null);
  assert.equal(await app.captureCookieHeaderFromContext({ context: () => ({ cookies: async () => [] }) }, "https://fixture.invalid"), null);
});

for (const rpcId of ["ujx1Bf", "hNvQHb", "kwDCne", "MUAZcd", "qpEbW", "aPya6c", "MaZiqc", "ESY5D", "XhaU0b", "k81mDb"]) {
  test("standalone capture metadata preserves RPC " + rpcId, () => {
    assert.deepEqual(app.classifyProgramRpcCapture("https://fixture.invalid/batchexecute?rpcids=" + rpcId + "&source-path=%2Fapp%2Fabc", "post"), {
      rpcId, label: rpcId, sourcePath: "/app/abc", method: "POST",
    });
  });
}

test("standalone capture metadata does not adopt the wider pool RPC set", () => {
  for (const id of ["L5adhe", "PCck7e", "unknown", "ujx1Bf,ESY5D"]) {
    assert.equal(app.classifyProgramRpcCapture("https://fixture.invalid/batchexecute?rpcids=" + id, "POST"), null);
  }
});

test("standalone capture metadata retains StreamGenerate source path and URL failures", () => {
  assert.deepEqual(app.classifyProgramRpcCapture("https://fixture.invalid/StreamGenerate?source-path=%2Fapp%2Fabc", null), {
    rpcId: null, label: "StreamGenerate", sourcePath: "/app/abc", method: "GET",
  });
  assert.equal(app.readRpcIdFromUrl("invalid"), null);
  assert.equal(app.readSourcePathFromUrl("invalid"), null);
});

test("standalone capture metadata distinguishes proxy surface and extracts endpoint fields", () => {
  assert.deepEqual(app.classifyHandlePairSurface("plain body"), { sourceSurface: null, sourceWsUrl: null, sourceTargetDomain: null });
  const result = app.classifyHandlePairSurface('Browser API Proxy Client\nDEFAULT_ENDPOINT = "wss://fixture.invalid/ws"\ntargetDomain = "generativelanguage.googleapis.com"');
  assert.equal(result.sourceSurface, "canvas_proxy_client");
  assert.equal(result.sourceWsUrl, "wss://fixture.invalid/ws");
  assert.equal(result.sourceTargetDomain, "generativelanguage.googleapis.com");
});
