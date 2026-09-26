import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { pollProducerPageVideoStatus } from "../producer-browser/page-status.mjs";

function serializedStatus(fetch, globals = {}) {
  return vm.runInNewContext(`(${pollProducerPageVideoStatus.toString()})`, {
    AbortController, TextDecoder, fetch, setTimeout, clearTimeout, ...globals,
  });
}

test("Producer page status deadline interrupts the polling delay and releases timers", async () => {
  const timers = new Set();
  const callback = serializedStatus(async () => new Response('{"status":"processing"}'), {
    setTimeout(fn, ms) {
      const timer = setTimeout(() => { timers.delete(timer); fn(); }, ms);
      timers.add(timer);
      return timer;
    },
    clearTimeout(timer) { timers.delete(timer); clearTimeout(timer); },
  });
  const result = await callback(request);
  assert.equal(result.error.code, "producer_browser_video_timeout");
  assert.equal(JSON.parse(result.error.body).status, "processing");
  assert.equal(timers.size, 0);
});

for (const boundary of ["network", "body"]) {
  test(`Producer page status reports sanitized ${boundary} failure`, async () => {
    const fail = () => { throw new Error("SYNTHETIC_PRIVATE_DETAIL"); };
    const callback = serializedStatus(async () => boundary === "network"
      ? fail() : new Response(new ReadableStream({ start(controller) { controller.error(new Error("SYNTHETIC_PRIVATE_DETAIL")); } })));
    const result = await callback(request);
    assert.equal(result.error.status, 502);
    assert.equal(result.error.code, "producer_browser_status_fetch_failed");
    assert.ok(!JSON.stringify(result).includes("SYNTHETIC_PRIVATE_DETAIL"));
  });
}

const request = {
  baseUrl: "https://producer.invalid", headers: {}, jobId: "fixture-job", timeoutMs: 25,
};

for (const boundary of ["headers", "body"]) {
  test(`Producer page status deadline aborts stalled ${boundary}`, async () => {
    let aborted = false;
    const wait = (signal) => new Promise((resolve, reject) => {
      const fallback = setTimeout(() => resolve('{"status":"completed"}'), 100);
      signal?.addEventListener("abort", () => {
        aborted = true;
        clearTimeout(fallback);
        reject(new Error("SYNTHETIC_PRIVATE_DETAIL"));
      }, { once: true });
    });
    const callback = serializedStatus(async (_url, options) => {
      if (boundary === "headers") await wait(options.signal);
      return boundary === "body" ? new Response(new ReadableStream({
        start(controller) {
          wait(options.signal).then((text) => {
            controller.enqueue(new TextEncoder().encode(text));
            controller.close();
          }).catch(() => {});
        },
      })) : new Response('{"status":"completed"}');
    });
    const result = await callback(request);
    assert.equal(aborted, true);
    assert.equal(result.ok, false);
    assert.equal(result.error.status, 504);
    assert.equal(result.error.code, "producer_browser_video_timeout");
    assert.ok(!JSON.stringify(result).includes("SYNTHETIC_PRIVATE_DETAIL"));
  });
}
