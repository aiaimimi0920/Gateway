import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { addHookScript } from "../aistudio-live-probe/browser-hook.mjs";

function fixture() {
  const forwarded = [];
  const listeners = new Map();
  const window = {
    postMessage: (...args) => { forwarded.push(args); return "sent"; },
    addEventListener: (name, fn) => listeners.set(name, fn),
  };
  vm.runInNewContext(addHookScript, { window });
  return { window, forwarded, listeners };
}

test("message diagnostics preserve forwarding of circular and BigInt messages", () => {
  const { window, forwarded, listeners } = fixture();
  const value = { amount: 1n };
  value.self = value;
  const transfer = [];
  assert.equal(window.postMessage(value, "*", transfer), "sent");
  assert.equal(forwarded[0][0], value);
  assert.equal(forwarded[0][2], transfer);
  const preview = window.__AISTUDIO_LIVE_CAPTURE__.at(-1).messagePreview;
  assert.match(preview, /bigint/);
  assert.match(preview, /circular/);
  assert.doesNotThrow(() => listeners.get("message")({ data: value, origin: "fixture" }));
});

test("message preview never invokes getters or toJSON and bounds broad nested data", () => {
  const { window, forwarded } = fixture();
  let invoked = 0;
  const value = { toJSON() { invoked++; throw new Error("must not run"); } };
  Object.defineProperty(value, "secret", { enumerable: true, get() { invoked++; throw new Error("secret"); } });
  value.large = Array.from({ length: 100 }, () => ({ text: "x".repeat(10000) }));
  window.postMessage(value, "*");
  assert.equal(invoked, 0);
  assert.equal(forwarded[0][0], value);
  const preview = window.__AISTUDIO_LIVE_CAPTURE__.at(-1).messagePreview;
  assert.ok(preview.length <= 2048);
  assert.match(preview, /accessor/);
});

test("failed diagnostic projection still forwards its original object", () => {
  const { window, forwarded } = fixture();
  const value = new Proxy({}, { ownKeys() { throw new Error("private diagnostic"); } });
  assert.equal(window.postMessage(value, "*"), "sent");
  assert.equal(forwarded[0][0], value);
  assert.equal(window.__AISTUDIO_LIVE_CAPTURE__.at(-1).messagePreview, "[unavailable]");
});

test("message preview distinguishes shared references from cycles and preserves sparse positions", () => {
  const { window } = fixture();
  const child = { ok: true };
  const sparse = [];
  sparse[2] = "third";
  sparse.meta = "not a JSON array element";
  window.postMessage({ a: child, b: child, sparse }, "*");
  assert.deepEqual(JSON.parse(window.__AISTUDIO_LIVE_CAPTURE__.at(-1).messagePreview), {
    a: { ok: true }, b: { ok: true }, sparse: [null, null, "third"],
  });
});
