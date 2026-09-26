import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { addHookScript } from "../aistudio-live-probe/browser-hook.mjs";
import { pollProbeCapture } from "../aistudio-live-probe/capture-polling.mjs";
import { shouldRetryAutoPromptDuringPolling } from "../aistudio-live-probe/ui-actions.mjs";

test("AI Studio extracted browser hook preserves forwarding, auth replies and event retention", async () => {
  const listeners = new Map();
  const forwarded = [];
  const response = new Response("fixture response", { headers: { "content-type": "text/plain" } });
  const window = {
    postMessage: (...args) => { forwarded.push(args); return "forwarded"; },
    addEventListener: (name, listener) => listeners.set(name, listener),
    fetch: async () => response,
  };
  const context = vm.createContext({ window, __AISTUDIO_AUTH_INDEX__: 2, setTimeout, clearTimeout, TextDecoder });
  vm.runInContext(addHookScript, context);
  const wrappedFetch = window.fetch;
  vm.runInContext(addHookScript, context);
  assert.equal(window.fetch, wrappedFetch, "initialization must not double-wrap APIs");
  assert.equal(window.postMessage("fixture", "https://fixture.invalid"), "forwarded");
  assert.deepEqual(forwarded[0], ["fixture", "https://fixture.invalid", undefined]);
  const replies = [];
  listeners.get("message")({ data: { type: "requestAuthIndex" }, origin: "https://fixture.invalid",
    source: { postMessage: (payload, origin) => replies.push({ payload, origin }) } });
  assert.equal(replies[0].payload.authIndex, 2);
  assert.equal(replies[0].payload.type, "authIndexResponse");
  assert.equal(await window.fetch("https://fixture.invalid/data"), response);
  for (let i = 0; i < 100 && !window.__AISTUDIO_LIVE_CAPTURE__.some((e) => e.kind === "fetch.response"); i++) {
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  assert.equal(window.__AISTUDIO_LIVE_CAPTURE__.at(-1).bodyPreview, "fixture response");
  for (let index = 0; index < 220; index += 1) window.postMessage(`message-${index}`, "*");
  assert.equal(window.__AISTUDIO_LIVE_CAPTURE__.length, 200);
  assert.equal(window.__AISTUDIO_LIVE_CAPTURE__.at(-1).messagePreview, "message-219");
});

async function pollingFixture({ pendingUntil = 0, prompt = null, timeoutMs = 2000 } = {}) {
  let now = 0;
  let matchReads = 0;
  let promptAttempts = 0;
  const callback = vm.runInNewContext(`(${pollProbeCapture.toString()})`, {
    Date: { now: () => now },
    hasPendingLocalProxyResponse: () => now < pendingUntil,
    countOpenUndispatchedLocalProxyConnections: () => 0,
    maybeDispatchLocalProxyMessages: async () => { throw new Error("unexpected dispatch"); },
    bestEffortDismissAistudioOverlays: async () => false,
    bestEffortApplyRemixModal: async () => false,
    shouldRetryAutoPromptDuringPolling,
    bestEffortAutoPrompt: async () => { promptAttempts += 1; return true; },
  });
  await callback({
    page: { waitForTimeout: async (ms) => { now += ms; } }, capture: {},
    timeoutMs, settleMs: 200, localWebSocketServer: null, localProxyMessages: [],
    localProxyDelayMs: 0, normalizedAutoPrompt: prompt, autoPromptSubmitted: false,
    persistCapture: async () => {},
    getMatchTimes: () => {
      matchReads += 1;
      return now > 0 && !prompt ? { firstMatchAt: 10, lastMatchAt: 10 }
        : { firstMatchAt: null, lastMatchAt: null };
    },
  });
  return { now, matchReads, promptAttempts };
}

test("AI Studio polling observes capture times updated after an awaited page delay", async () => {
  assert.deepEqual(await pollingFixture(), { now: 500, matchReads: 2, promptAttempts: 0 });
});

test("AI Studio polling keeps waiting for a pending local response after traffic settles", async () => {
  assert.deepEqual(await pollingFixture({ pendingUntil: 1000 }), { now: 1000, matchReads: 3, promptAttempts: 0 });
});

test("AI Studio polling stops prompt retries once submission succeeds", async () => {
  const result = await pollingFixture({ prompt: "fixture prompt", timeoutMs: 11000 });
  assert.equal(result.promptAttempts, 1);
  assert.equal(result.now, 11000);
});
