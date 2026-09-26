import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { browserFetch } from "../udio-browser/transport.mjs";

function fixture(chunks, { readError = false } = {}) {
  const calls = [];
  let index = 0;
  const reader = {
    read: async () => {
      calls.push("read");
      if (readError) throw new Error("synthetic read failure");
      return index < chunks.length ? { value: chunks[index++], done: false } : { done: true };
    },
    cancel: async () => { calls.push("cancel"); },
    releaseLock: () => { calls.push("release"); },
  };
  const response = { ok: true, status: 200, headers: { get: () => "application/json" },
    body: { getReader: () => reader }, text: async () => "unbounded" };
  const page = { evaluate: async (callback, input) => vm.runInNewContext(
    `(${callback.toString()})(input)`, {
      input, fetch: async () => response, AbortController, TextDecoder,
      setTimeout: () => 1, clearTimeout: () => { calls.push("clearTimeout"); },
    }) };
  return { page, calls, response };
}

test("browser JSON response overflow cancels its reader and clears timeout", async () => {
  const { page, calls } = fixture([new Uint8Array(16 * 1024 * 1024 + 1)]);
  const result = await browserFetch(page, { requestUrl: "https://example.test", timeoutMs: 1000 });
  assert.equal(result.transportError, true);
  assert.equal(result.status, 502);
  assert.deepEqual(calls, ["read", "cancel", "release", "clearTimeout"]);
});

test("browser text decoding preserves UTF8 across input and fixed-block boundaries", async () => {
  const text = "x".repeat(65535) + "é中";
  const encoded = new TextEncoder().encode(text);
  const { page, calls } = fixture([encoded.subarray(0, 65536), encoded.subarray(65536)]);
  const result = await browserFetch(page, { requestUrl: "https://example.test", timeoutMs: 1000 });
  assert.equal(result.ok, true);
  assert.equal(result.text, text);
  assert.deepEqual(calls, ["read", "read", "read", "release", "clearTimeout"]);
});

test("browser stream failure is a transport failure, not an empty successful body", async () => {
  const { page, calls } = fixture([], { readError: true });
  const result = await browserFetch(page, { requestUrl: "https://example.test", timeoutMs: 1000 });
  assert.equal(result.transportError, true);
  assert.equal(result.status, 504);
  assert.deepEqual(calls, ["read", "cancel", "release", "clearTimeout"]);
});

test("browser absent response body preserves empty text and clears timeout", async () => {
  const { page, calls, response } = fixture([]);
  response.body = null;
  const result = await browserFetch(page, { requestUrl: "https://example.test", timeoutMs: 1000 });
  assert.equal(result.text, "");
  assert.equal(result.ok, true);
  assert.deepEqual(calls, ["clearTimeout"]);
});

test("one-byte and empty browser chunks preserve content without per-chunk retention", async () => {
  const byte = new Uint8Array([120]);
  const { page } = fixture([...Array.from({ length: 65537 }, () => byte), new Uint8Array(0)]);
  const result = await browserFetch(page, { requestUrl: "https://example.test", timeoutMs: 1000 });
  assert.equal(result.text, "x".repeat(65537));
});
