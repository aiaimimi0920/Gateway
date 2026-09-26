import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const requestUrl = "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=MaZiqc&source-path=%2Fapp%2Fdeadbeef";
const pairText = '"c_deadbeef", "r_cafebabe"';

function captureFixture(t, body) {
  const listeners = new Map();
  const page = {
    on(name, listener) { assert.equal(listeners.has(name), false); listeners.set(name, listener); },
    off(name, listener) { assert.equal(listeners.get(name), listener); listeners.delete(name); },
  };
  const capture = app.startNetworkCapture(page, "bootstrap_program");
  t.after(() => { capture.stop(); assert.equal(listeners.size, 0); });
  const request = { url: () => requestUrl, method: () => "POST", headers: () => ({}), postData: () => body };
  const response = {
    url: () => requestUrl, request: () => request, status: () => 200,
    headers: () => ({
      "content-type": "application/json", "content-length": String(Buffer.byteLength(body, "utf8")),
    }), text: async () => body,
  };
  return {
    state: capture.state,
    async dispatch(kind) { await listeners.get(kind)(kind === "request" ? request : response); },
  };
}

for (const kind of ["request", "response"]) {
  for (const [label, text, expectedWs, expectedDomain, hasPair] of [
    ["default endpoint", `${pairText} Browser API Proxy Client DEFAULT_ENDPOINT = "ws://127.0.0.1:42322/ws"; targetDomain = "generativelanguage.googleapis.com"`, "ws://127.0.0.1:42322/ws", "generativelanguage.googleapis.com", true],
    ["constructor endpoint and escaped pair", String.raw`\"c_deadbeef\", \"r_cafebabe\" Routing Google API requests via WebSocket constructor(endpoint = "wss://proxy.invalid/ws") targetDomain = "fixture.invalid"`, "wss://proxy.invalid/ws", "fixture.invalid", true],
    ["marker without endpoint", `${pairText} Server WS Endpoint`, null, null, true],
    ["proxy marker without pair", "Browser API Proxy Client", null, null, false],
  ]) {
    test(`proxy capture binding ${kind} retains ${label}`, async (t) => {
      const { state, dispatch } = captureFixture(t, text);
      await dispatch(kind);
      assert.equal(state.handlePairs.length, hasPair ? 1 : 0);
      if (hasPair) {
        const { ts, ...pair } = state.handlePairs[0];
        assert.deepEqual(pair, {
          appPath: "/app/deadbeef", programUrl: "https://gemini.google.com/app/deadbeef",
          conversationId: "c_deadbeef", responseId: "r_cafebabe", sourceUrl: requestUrl,
          sourceRpc: "MaZiqc", sourceKind: kind, sourceSurface: "canvas_proxy_client",
          sourceWsUrl: expectedWs, sourceTargetDomain: expectedDomain,
        });
        assert.ok(Number.isFinite(Date.parse(ts)));
      }
      assert.equal(state.events.length, 1);
      assert.equal(state.events[0].type, kind);
      assert.equal(state.rpcCaptures.length, 1);
      assert.equal(state.rpcCaptures[0].type, kind);
      assert.equal(state.rpcCaptures[0].bodyText, text);
      assert.equal(state.rpcCaptures[0].sourcePath, "/app/deadbeef");
    });
  }

  test(`ordinary handle capture ${kind} keeps proxy metadata absent`, async (t) => {
    const { state, dispatch } = captureFixture(t, pairText);
    await dispatch(kind);
    assert.equal(state.handlePairs.length, 1);
    const pair = state.handlePairs[0];
    assert.equal(pair.sourceKind, kind);
    assert.equal(pair.sourceSurface, null);
    assert.equal(pair.sourceWsUrl, null);
    assert.equal(pair.sourceTargetDomain, null);
    assert.equal(state.rpcCaptures[0].bodyText, pairText);
  });
}
