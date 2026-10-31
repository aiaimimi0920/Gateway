import assert from "node:assert/strict";
import test from "node:test";
import { createFetchExecutionOwner } from "../gemini-canvas-browser-pool-fetch-execution.mjs";

function harness(hostFailure = null) {
  const calls = [];
  const { runFetchOperation } = createFetchExecutionOwner({
    DEFAULT_TIMEOUT_MS: 1000,
    resolveProgramPageUrl: () => null,
    startNetworkCapture: () => ({ ready: Promise.resolve(), state: { events: [], rpcCaptures: [] },
      async stop() { calls.push("stop"); } }),
    buildProgramHandleState: () => ({}),
    isFixtureCanvasBaseUrl: () => false,
    async ensureSharePage() { calls.push("share"); },
    async ensureLoopbackConnectedClient() { calls.push("unbound-bootstrap"); },
    listConnectedClients() { calls.push("global-clients"); return [{ connectionId: "other-account" }]; },
    async dispatchConnectedProxyRequest() {
      calls.push("unbound-dispatch");
      return { status: 200, headers: {}, bodyText: "wrong-account-response" };
    },
    async runCanvasHostFetchOperation(input) {
      calls.push(["host", input]);
      if (hostFailure) throw hostFailure;
      return { operation: "fetch", status: 200, ok: true, executionSurface: "canvas_preview_host",
        bodyText: input.entry.owner };
    },
    log() {},
  });
  const entry = (owner) => ({ owner, context: { pages: () => [] },
    page: { url: () => "https://gemini.google.com/share/fixture", isClosed: () => false } });
  const run = (value) => runFetchOperation(value, { googleFetchMode: "canvas_proxy",
    baseUrl: "https://gemini.google.com", shareId: "fixture",
    fetchRequest: { url: "https://generativelanguage.googleapis.com/v1beta/models/fixture:generateContent",
      method: "POST", jsonBody: { contents: [{ parts: [{ text: "synthetic" }] }] } } });
  return { calls, entry, run };
}

test("canvas_proxy dispatches through the current Canvas host, never a global connected client", async () => {
  const h = harness(), current = h.entry("current-account");
  const result = await h.run(current);
  assert.equal(result.executionSurface, "canvas_preview_host");
  assert.equal(result.bodyText, "current-account");
  const host = h.calls.find(call => Array.isArray(call) && call[0] === "host")[1];
  assert.equal(host.entry, current);
  assert.equal(host.method, "POST");
  assert.equal(JSON.parse(host.requestBodyText).contents[0].parts[0].text, "synthetic");
  assert.deepEqual(h.calls.filter(call => typeof call === "string"), ["stop"]);
});

test("missing current Canvas host fails closed even when another account is connected", async () => {
  const failure = Object.assign(new Error("Canvas host identity unavailable"),
    { status: 503, code: "gemini_canvas_host_identity_unavailable" });
  const h = harness(failure);
  await assert.rejects(h.run(h.entry("missing-host")), error => error === failure);
  assert.deepEqual(h.calls.filter(call => typeof call === "string"), ["stop"]);
});

test("concurrent Canvas accounts keep their own host entry and have no global fallback", async () => {
  const h = harness();
  const replies = await Promise.all([h.run(h.entry("account-a")), h.run(h.entry("account-b"))]);
  assert.deepEqual(replies.map(reply => reply.bodyText), ["account-a", "account-b"]);
  assert.deepEqual(h.calls.filter(call => typeof call === "string"), ["stop", "stop"]);
});
