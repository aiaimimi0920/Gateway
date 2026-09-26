import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { browserContextFetch, browserContextReadSse } from "../producer-browser/transport.mjs";

function page(fetch) {
  return { evaluate: async (callback, args) => vm.runInNewContext(`(${callback.toString()})`, {
    AbortController, TextDecoder, fetch, setTimeout, clearTimeout,
    location: { href: "https://producer.invalid/fixture" }, document: { title: "fixture" },
  })(structuredClone(args)) };
}

for (const [name, invoke] of [["fetch", browserContextFetch], ["stream", browserContextReadSse]]) {
  test(`Producer browser context ${name} aborts and releases a stalled body`, async () => {
    let cancelled = false;
    const response = new Response(new ReadableStream({ cancel() { cancelled = true; } }));
    const result = await invoke(page(async () => response), "https://producer.invalid/api", {}, 25);
    assert.equal(result.status, 504);
    assert.equal(result.fetchError, `producer_browser_${name}_timeout`);
    assert.equal(cancelled, true);
    assert.equal(response.body.locked, false);
  });

  test(`Producer browser context ${name} accepts exactly 16 MiB and split UTF8`, async () => {
    const label = "\u4f60\u597d";
    const expected = label + " ".repeat(16 * 1024 * 1024 - Buffer.byteLength(label));
    const bytes = new TextEncoder().encode(expected);
    const response = new Response(new ReadableStream({
      start(controller) {
        controller.enqueue(bytes.subarray(0, 1));
        controller.enqueue(bytes.subarray(1));
        controller.close();
      },
    }));
    const result = await invoke(page(async () => response), "https://producer.invalid/api", {}, 2000);
    assert.equal(result.ok, true);
    assert.equal(result.text, expected);
    assert.equal(response.body.locked, false);
  });

  test(`Producer browser context ${name} bounds and releases oversized response bodies`, async () => {
    let cancelled = false;
    let streamController;
    const response = new Response(new ReadableStream({
      start(controller) {
        streamController = controller;
        controller.enqueue(new Uint8Array(16 * 1024 * 1024 + 1));
      },
      cancel() { cancelled = true; },
    }));
    const fallback = setTimeout(() => streamController.close(), 100);
    try {
      const result = await invoke(page(async () => response), "https://producer.invalid/api", {}, 1000);
      assert.equal(result.ok, false);
      assert.equal(result.fetchError, "producer_browser_response_too_large");
      assert.equal(cancelled, true);
      assert.equal(response.body.locked, false);
    } finally { clearTimeout(fallback); }
  });

  test(`Producer browser context ${name} rejects body failures without disclosing raw diagnostics`, async () => {
    const response = new Response(new ReadableStream({
      start(controller) { controller.error(new Error("PRIVATE_CONTEXT_CANARY")); },
    }));
    const result = await invoke(page(async () => response), "https://producer.invalid/api", {}, 1000);
    assert.equal(result.ok, false);
    assert.equal(result.fetchError, `producer_browser_${name}_failed`);
    assert.ok(!JSON.stringify(result).includes("PRIVATE_CONTEXT_CANARY"));
    assert.equal(response.body.locked, false);
  });

  test(`Producer browser context ${name} refuses credential-bearing redirects`, async () => {
    let redirect;
    const result = await invoke(page(async (_url, options) => {
      redirect = options.redirect;
      return new Response("fixture");
    }), "https://producer.invalid/api", { redirect: "follow" }, 1000);
    assert.equal(result.ok, true);
    assert.equal(redirect, "error");
  });
}

test("Producer browser SSE terminal event cancels and unlocks the response reader", async () => {
  let cancelled = false;
  const response = new Response(new ReadableStream({
    start(controller) { controller.enqueue(new TextEncoder().encode("event: final\ndata: {}\n\n")); },
    cancel() { cancelled = true; },
  }));
  const result = await browserContextReadSse(page(async () => response), "https://producer.invalid/stream", {}, 1000);
  assert.equal(result.ok, true);
  assert.match(result.text, /event: final/);
  assert.equal(cancelled, true);
  assert.equal(response.body.locked, false);
});

for (const [name, chunks] of [
  ["fragmented final data", ["event: fi", "nal", '\ndata: {"proof":true}', "\n\n"]],
  ["terminal prefix in a nonterminal event name", ["event: final", "ize\ndata: keep\n\n", "event: final\ndata: end\n\n"]],
  ["overridden event field", ["event: final\n", "event: message\ndata: keep\n\n", "event: error\ndata: end\n\n"]],
]) {
  test(`Producer browser SSE waits for the complete terminal frame: ${name}`, async () => {
    let index = 0;
    const response = new Response(new ReadableStream({
      pull(controller) {
        if (index < chunks.length) controller.enqueue(new TextEncoder().encode(chunks[index++]));
        else controller.close();
      },
    }));
    const result = await browserContextReadSse(page(async () => response), "https://producer.invalid/stream", {}, 1000);
    assert.equal(result.ok, true);
    assert.equal(result.text, chunks.join(""));
    assert.equal(response.body.locked, false);
  });
}

test("Producer browser SSE handles fragmented CRLF and long event whitespace", async () => {
  const chunks = ["event:", " ".repeat(4096), "final\r", '\ndata: {"proof":true}\r', "\n\r", "\n"];
  let index = 0;
  const response = new Response(new ReadableStream({
    pull(controller) {
      if (index < chunks.length) controller.enqueue(new TextEncoder().encode(chunks[index++]));
      else controller.close();
    },
  }));
  const result = await browserContextReadSse(page(async () => response), "https://producer.invalid/stream", {}, 1000);
  assert.equal(result.ok, true);
  assert.ok(result.text.includes('data: {"proof":true}'));
  assert.equal(response.body.locked, false);
});
