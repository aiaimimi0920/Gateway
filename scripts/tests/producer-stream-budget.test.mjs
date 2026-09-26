import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { parseSseFrames } from "../producer-browser/conversation-stream.mjs";
import { sendProducerPageConversation } from "../producer-browser/page-conversation.mjs";

async function pageStream(text) {
  const callback = vm.runInNewContext(`(${sendProducerPageConversation.toString()})`, {
    AbortController, TextDecoder, setTimeout, clearTimeout,
    fetch: async (_url, options) => new Response(options.method === "POST"
      ? '{"job_id":"fixture"}' : text),
  });
  return callback({ baseUrl: "https://producer.invalid", headers: {}, prompt: "fixture", timeoutMs: 2000 });
}

for (const [name, text] of [
  ["frame count", "event: final\ndata: {}\n\n".repeat(4097)],
  ["single-frame line count", "data: x\n".repeat(65537)],
  ["ignored line count", ": keepalive\n".repeat(65537)],
]) {
  test(`Producer Node SSE rejects excessive ${name}`, () => {
    assert.throws(() => parseSseFrames(text), (error) =>
      error.status === 502 && error.code === "producer_browser_stream_too_complex");
  });
  test(`Producer serialized page SSE rejects excessive ${name}`, async () => {
    const result = await pageStream(text);
    assert.equal(result.ok, false);
    assert.equal(result.error.status, 502);
    assert.equal(result.error.code, "producer_browser_stream_too_complex");
    assert.ok(!JSON.stringify(result).includes("keepalive"));
  });
}

test("Producer SSE admits exact frame and line boundaries", async () => {
  const frames = "event: final\ndata: {}\n\n".repeat(4096);
  assert.equal(parseSseFrames(frames).length, 4096);
  assert.equal((await pageStream(frames)).stream.finalSeen, true);
  const lines = ": heartbeat\n".repeat(65534) + "event: final\ndata: {}\n";
  assert.equal(parseSseFrames(lines).length, 1);
  assert.equal((await pageStream(lines)).stream.finalSeen, true);
});

test("Producer SSE recognizes CR-only frames across Node and page readers", async () => {
  const text = 'event: conversation_id\rdata: {"id":"fixture-conversation"}\r\revent: final\rdata: {}\r\r';
  assert.equal(parseSseFrames(text).at(-1).event, "final");
  const result = await pageStream(text);
  assert.equal(result.stream.conversationId, "fixture-conversation");
  assert.equal(result.stream.finalSeen, true);
});

test("Producer SSE keeps CRLF multiline data and EOF frame material", () => {
  assert.deepEqual(parseSseFrames('event: message\r\ndata: first\r\ndata: second\r\n\r\nevent: final\ndata: {}'), [
    { event: "message", dataText: "first\nsecond" },
    { event: "final", dataText: "{}" },
  ]);
});
