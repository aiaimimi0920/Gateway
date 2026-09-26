import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { executeBrowserTransport } from "../producer-browser/browser-transport.mjs";
import { sendProducerPageConversation } from "../producer-browser/page-conversation.mjs";
import { pollProducerPageVideoStatus } from "../producer-browser/page-status.mjs";
import { readNodeResponseText } from "../producer-browser/response-body.mjs";

const frames = 'event: conversation_id\ndata: {"id":"fixture"}\n\nevent: final\ndata: {}\n\n';
for (const [name, callback, args, responses] of [
  ["browser context", executeBrowserTransport, { url: "https://producer.invalid", options: {}, timeoutMs: 1000 }, ["fixture"]],
  ["page conversation", sendProducerPageConversation, { baseUrl: "https://producer.invalid", headers: {}, timeoutMs: 1000 }, ['{"job_id":"fixture"}', frames]],
  ["page status", pollProducerPageVideoStatus, { baseUrl: "https://producer.invalid", headers: {}, jobId: "fixture", timeoutMs: 1000 }, ['{"status":"completed"}']],
]) {
  test(`Producer ${name} does not reserve a maximum-sized buffer for tiny responses`, async () => {
    const allocations = [];
    class TrackedBytes extends Uint8Array {
      constructor(...args) {
        super(...args);
        if (typeof args[0] === "number") allocations.push(args[0]);
      }
    }
    const replies = [...responses];
    const run = vm.runInNewContext(`(${callback.toString()})`, {
      AbortController, TextDecoder, Uint8Array: TrackedBytes, setTimeout, clearTimeout,
      location: { href: "https://producer.invalid" }, document: { title: "fixture" },
      fetch: async () => new Response(replies.shift()),
    });
    const result = await run(args);
    assert.equal(result.ok, true);
    assert.ok(allocations.length > 0);
    assert.ok(allocations.every((bytes) => bytes <= 64 * 1024), `allocations=${allocations}`);
  });
}

test("Producer Node reader avoids maximum-sized allocation for a tiny response", async (t) => {
  const response = new Response("fixture");
  const allocations = [];
  const allocate = Buffer.allocUnsafe;
  t.mock.method(Buffer, "allocUnsafe", (bytes) => { allocations.push(bytes); return allocate(bytes); });
  assert.equal(await readNodeResponseText(response, new AbortController().signal), "fixture");
  assert.ok(allocations.length > 0);
  assert.ok(allocations.every((bytes) => bytes <= 64 * 1024), `allocations=${allocations}`);
});

for (const mode of ["Node", "browser"]) {
  test(`Producer ${mode} response retains all bytes across repeated buffer growth`, async () => {
    const expected = `first-${"a".repeat(90_000)}-middle-${"b".repeat(220_000)}-last`;
    const bytes = new TextEncoder().encode(expected);
    const response = new Response(new ReadableStream({
      start(controller) {
        const cuts = [0, 3, 65_537, 200_000, bytes.length];
        for (let index = 1; index < cuts.length; index++) {
          controller.enqueue(bytes.subarray(cuts[index - 1], cuts[index]));
        }
        controller.close();
      },
    }));
    let text;
    if (mode === "Node") {
      text = await readNodeResponseText(response, new AbortController().signal);
    } else {
      const run = vm.runInNewContext(`(${executeBrowserTransport.toString()})`, {
        AbortController, TextDecoder, setTimeout, clearTimeout,
        location: { href: "https://producer.invalid" }, document: { title: "fixture" },
        fetch: async () => response,
      });
      text = (await run({ url: "https://producer.invalid", options: {}, timeoutMs: 1000 })).text;
    }
    assert.equal(text, expected);
    assert.equal(response.body.locked, false);
  });
}
