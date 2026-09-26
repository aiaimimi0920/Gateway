import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { sendProducerPageConversation } from "../producer-browser/page-conversation.mjs";

const request = {
  baseUrl: "https://producer.invalid", headers: { authorization: "Bearer fixture" },
  prompt: "fixture", clientContext: {}, modelName: "producer:standard", timeoutMs: 1000,
};
const job = '{"job_id":"fixture-job"}';
const final = 'event: conversation_id\ndata: {"id":"fixture-conversation"}\n\nevent: final\ndata: {}\n\n';
function callback(fetch) {
  return vm.runInNewContext(`(${sendProducerPageConversation.toString()})`, {
    AbortController, TextDecoder, fetch, setTimeout, clearTimeout,
  });
}

for (const phase of ["POST", "GET"]) {
  test(`Producer page conversation cancels a stalled ${phase} reader at the deadline`, async () => {
    let cancelled = false;
    const response = new Response(new ReadableStream({ cancel() { cancelled = true; } }));
    const result = await callback(async (_url, options) => options.method === phase
      ? response : new Response(job))({ ...request, timeoutMs: 25 });
    assert.equal(result.error.code, "producer_browser_conversation_timeout");
    assert.equal(cancelled, true);
    assert.equal(response.body.locked, false);
  });

  test(`Producer page conversation rejects oversized ${phase} bodies`, async () => {
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
      const result = await callback(async (_url, options) => options.method === phase
        ? response : new Response(job))(request);
      assert.equal(result.ok, false);
      assert.equal(result.error.code, "producer_browser_conversation_response_too_large");
      assert.equal(cancelled, true);
      assert.equal(response.body.locked, false);
    } finally { clearTimeout(fallback); }
  });

  test(`Producer page conversation propagates ${phase} body failure without raw diagnostics`, async () => {
    const response = new Response(new ReadableStream({
      start(controller) { controller.error(new Error("PRIVATE_FIXTURE_DETAIL")); },
    }));
    const result = await callback(async (_url, options) => options.method === phase
      ? response : new Response(job))(request);
    assert.equal(result.ok, false);
    assert.equal(result.error.code, "producer_browser_conversation_fetch_failed");
    assert.ok(!JSON.stringify(result).includes("PRIVATE_FIXTURE_DETAIL"));
    assert.equal(response.body.locked, false);
  });
}

for (const phase of ["POST", "GET"]) {
  test(`Producer page conversation accepts exactly 16 MiB at ${phase}`, async () => {
    const base = phase === "POST" ? job : final;
    const response = new Response(base + " ".repeat(16 * 1024 * 1024 - Buffer.byteLength(base)));
    const result = await callback(async (_url, options) => options.method === phase
      ? response : new Response(options.method === "POST" ? job : final))({ ...request, timeoutMs: 2000 });
    assert.equal(result.ok, true);
    assert.equal(result.stream.finalSeen, true);
    assert.equal(response.body.locked, false);
  });
}

test("Producer page POST and SSE share one elapsed budget", async () => {
  let aborted = false;
  const result = await callback(async (_url, options) => {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        options.signal.removeEventListener("abort", abort);
        resolve();
      }, 100);
      const abort = () => {
        aborted = true;
        clearTimeout(timer);
        reject(new Error("PRIVATE_FIXTURE_DETAIL"));
      };
      options.signal.addEventListener("abort", abort, { once: true });
    });
    return new Response(options.method === "POST" ? job : final);
  })({ ...request, timeoutMs: 150 });
  assert.equal(aborted, true);
  assert.equal(result.ok, false);
  assert.equal(result.error.status, 504);
  assert.equal(result.error.code, "producer_browser_conversation_timeout");
});

test("Producer page POST and SSE refuse redirects and retain successful stream material", async () => {
  const redirects = [];
  const result = await callback(async (_url, options) => {
    redirects.push(options.redirect);
    return new Response(options.method === "POST" ? job : final);
  })(request);
  assert.equal(result.ok, true);
  assert.equal(result.jobId, "fixture-job");
  assert.equal(result.stream.conversationId, "fixture-conversation");
  assert.equal(result.stream.finalSeen, true);
  assert.deepEqual(redirects, ["error", "error"]);
});
