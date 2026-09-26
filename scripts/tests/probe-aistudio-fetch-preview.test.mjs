import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { addHookScript } from "../aistudio-live-probe/browser-hook.mjs";

function hook(fetch) {
  const timers = new Set();
  const window = { fetch, addEventListener() {} };
  vm.runInNewContext(addHookScript, { window, TextDecoder,
    setTimeout: (callback) => { timers.add(callback); return callback; },
    clearTimeout: (callback) => timers.delete(callback),
  });
  return { window, timers };
}

test("fetch preview returns the original response before a stalled body and times out", async () => {
  let cancelled = 0;
  let released = 0;
  const response = { status: 200, clone: () => ({ body: { getReader: () => ({
    read: () => new Promise(() => {}),
    cancel: () => { cancelled++; return new Promise(() => {}); },
    releaseLock: () => { released++; },
  }) } }) };
  const { window, timers } = hook(async () => response);
  assert.equal(await window.fetch("fixture"), response);
  assert.equal(timers.size, 1);
  for (const timeout of timers) timeout();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(cancelled, 1);
  assert.equal(released, 1);
  assert.equal(timers.size, 0);
  assert.equal(window.__AISTUDIO_LIVE_CAPTURE__.at(-1).bodyPreview, null);
});

test("fetch preview caps retained bytes while leaving the complete real response readable", async () => {
  const response = new Response("x".repeat(16384));
  const { window, timers } = hook(async () => response);
  assert.equal(await window.fetch("fixture"), response);
  assert.equal((await response.text()).length, 16384);
  for (let i = 0; i < 100 && timers.size; i++) {
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  assert.equal(window.__AISTUDIO_LIVE_CAPTURE__.at(-1).bodyPreview.length, 4096);
  assert.equal(timers.size, 0);
});

test("fetch preview admits at most eight stalled clones and reopens slots after deadline", async () => {
  let clones = 0;
  const response = { status: 200, clone: () => {
    clones++;
    return { body: { getReader: () => ({ read: () => new Promise(() => {}),
      cancel: async () => {}, releaseLock() {} }) } };
  } };
  const { window, timers } = hook(async () => response);
  for (let i = 0; i < 20; i++) assert.equal(await window.fetch("fixture"), response);
  assert.equal(clones, 8);
  for (const timeout of timers) timeout();
  await new Promise((resolve) => setImmediate(resolve));
  await window.fetch("fixture");
  assert.equal(clones, 9);
  for (const timeout of timers) timeout();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(timers.size, 0);
});

test("fetch preview retains admission slots while tee cancellation remains pending", async () => {
  let clones = 0;
  const response = { status: 200, clone: () => {
    clones++;
    return { body: { getReader: () => ({ read: () => new Promise(() => {}),
      cancel: () => new Promise(() => {}), releaseLock() {} }) } };
  } };
  const { window, timers } = hook(async () => response);
  for (let i = 0; i < 8; i++) await window.fetch("fixture");
  for (const timeout of timers) timeout();
  await new Promise((resolve) => setImmediate(resolve));
  for (let i = 0; i < 100; i++) assert.equal(await window.fetch("fixture"), response);
  assert.equal(clones, 8);
  assert.equal(timers.size, 0);
});
