import test from "node:test";
import assert from "node:assert/strict";
import { createNativeModelGate } from "../udio-browser/native-model.mjs";

function fixture({ delayedReady = false } = {}) {
  let handler, release, removed = 0, failed = [], sent = 0, aborted = 0, fallback = 0;
  const ready = delayedReady ? new Promise(resolve => { release = resolve; }) : Promise.resolve();
  const matches = () => true;
  const page = { route: (_matches, fn) => { assert.equal(_matches, matches); handler = fn; return ready; },
    unroute: async (_matches, fn) => { assert.equal(_matches, matches); assert.equal(fn, handler); removed++; } };
  const gate = createNativeModelGate(page, matches, "udio-fixture-native", reason => failed.push(reason));
  const send = (model, method = "POST", throws = false) => handler({
    request: () => ({ method: () => method, postData: () => {
      if (throws) throw new Error("private-post-data");
      return JSON.stringify({ gen_params: { model_type: model } });
    } }), continue: async () => { sent++; }, abort: async () => { aborted++; }, fallback: async () => { fallback++; },
  });
  return { gate, send, release: () => release(), counts: () => ({ removed, failed: failed.map(r => r.code), sent, aborted, fallback }) };
}

test("Udio model gate cleans late registration and preserves exact handler ownership", async () => {
  const f = fixture({ delayedReady: true });
  const stopping = f.gate.stop(); f.release(); await stopping;
  assert.equal(f.counts().removed, 1);
  await f.send("udio-fixture-native");
  assert.equal(f.counts().sent, 0); assert.equal(f.counts().aborted, 1);
});
test("Udio model gate leaves non-POST routes to their existing owners", async () => {
  const f = fixture(); await f.gate.ready; f.gate.arm();
  await f.send(undefined, "GET"); await f.gate.stop();
  assert.deepEqual(f.counts(), { removed: 1, failed: [], sent: 0, aborted: 0, fallback: 1 });
});
test("Udio model gate rejected first request consumes the single slot", async () => {
  const f = fixture(); await f.gate.ready; f.gate.arm();
  await Promise.all([f.send("wrong"), f.send("udio-fixture-native")]); await f.gate.stop();
  assert.equal(f.counts().sent, 0); assert.equal(f.counts().aborted, 2);
  assert.deepEqual(f.counts().failed, ["udio_browser_model_mismatch", "udio_browser_ambiguous_generation"]);
});
test("Udio model gate aborts unreadable native request without leaking diagnostics", async () => {
  const f = fixture(); await f.gate.ready; f.gate.arm();
  await f.send(undefined, "POST", true); await f.gate.stop();
  assert.deepEqual(f.counts(), { removed: 1, failed: ["udio_browser_model_mismatch"], sent: 0, aborted: 1, fallback: 0 });
});
test("Udio model gate rejects POST before the only explicit click is armed", async () => {
  const f = fixture(); await f.gate.ready;
  await f.send("udio-fixture-native"); await f.gate.stop();
  assert.equal(f.counts().sent, 0); assert.equal(f.counts().aborted, 1);
});
