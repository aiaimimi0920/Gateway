import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { pollProducerPageVideoStatus } from "../producer-browser/page-status.mjs";

const limit = 16 * 1024 * 1024;
const request = {
  baseUrl: "https://producer.invalid", headers: {}, jobId: "fixture-job", timeoutMs: 2000,
};
function callback(fetch) {
  return vm.runInNewContext(`(${pollProducerPageVideoStatus.toString()})`, {
    AbortController, TextDecoder, fetch, setTimeout, clearTimeout,
  });
}

test("Producer browser status rejects oversized decoded bodies and cancels the stream", async () => {
  let cancelled = false;
  let streamController;
  const bytes = new Uint8Array(limit + 1);
  const response = new Response(new ReadableStream({
    start(controller) { streamController = controller; controller.enqueue(bytes); },
    cancel() { cancelled = true; },
  }));
  // A fallback closes the unbounded baseline reader so the regression cannot hang.
  const timer = setTimeout(() => streamController.close(), 100);
  try {
    const result = await callback(async () => response)(request);
    assert.equal(result.error.code, "producer_browser_status_response_too_large");
    assert.equal(cancelled, true);
    assert.equal(response.body.locked, false);
  } finally { clearTimeout(timer); }
});

test("Producer browser status accepts exactly 16 MiB and preserves decoded UTF8", async () => {
  const base = JSON.stringify({ status: "completed", label: "\u4f60\u597d", padding: "" });
  const text = base.replace('"padding":""', `"padding":"${" ".repeat(limit - Buffer.byteLength(base))}"`);
  const bytes = new TextEncoder().encode(text);
  const response = new Response(new ReadableStream({
    start(controller) {
      const split = bytes.indexOf(0xe4) + 1;
      controller.enqueue(bytes.subarray(0, split));
      controller.enqueue(bytes.subarray(split));
      controller.close();
    },
  }));
  const result = await callback(async () => response)(request);
  assert.equal(result.ok, true);
  assert.equal(result.result.statusPayload.label, "\u4f60\u597d");
  assert.equal(bytes.length, limit);
  assert.equal(response.body.locked, false);
});

test("Producer browser status disallows credential-bearing redirects", async () => {
  let redirect;
  const result = await callback(async (_url, options) => {
    redirect = options.redirect;
    return new Response('{"status":"completed"}');
  })(request);
  assert.equal(result.ok, true);
  assert.equal(redirect, "error");
});
