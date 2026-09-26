import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { collectMediaUrls } from "../producer-browser/conversation-stream.mjs";
import { pollProducerPageVideoStatus } from "../producer-browser/page-status.mjs";

const url = "https://cdn.invalid/video.mp4";
function nested(depth) {
  let value = url;
  for (let index = 0; index < depth; index++) value = { child: value };
  return value;
}

test("Producer media traversal preserves depth-first ordering and repeated references", () => {
  const shared = { clip: url };
  const result = collectMediaUrls({ first: [shared, "https://cdn.invalid/next.mov"], second: shared });
  assert.deepEqual(result, [url, "https://cdn.invalid/next.mov", url]);
});

for (const [name, make] of [
  ["excessive nesting", () => nested(65)],
  ["wide payload", () => Array(100_000).fill(null)],
  ["URL flood", () => Array(1025).fill(url)],
  ["cycle", () => { const value = {}; value.self = value; return value; }],
]) {
  test(`Producer media traversal rejects ${name} without exposing the payload`, () => {
    assert.throws(() => collectMediaUrls(make()), (error) => {
      assert.equal(error.code, "producer_media_payload_too_complex");
      assert.equal(error.status, 502);
      assert.ok(!error.message.includes(url));
      return true;
    });
  });
}

test("Producer media traversal accepts the documented depth and URL limits", () => {
  assert.deepEqual(collectMediaUrls(nested(64)), [url]);
  assert.equal(collectMediaUrls(Array(1024).fill(url)).length, 1024);
  assert.deepEqual(collectMediaUrls(Array(99_999).fill(null)), []);
});

test("Producer serialized status rejects an overly deep provider payload explicitly", async () => {
  const response = new Response(JSON.stringify({ status: "completed", nested: nested(65) }));
  const callback = vm.runInNewContext(`(${pollProducerPageVideoStatus.toString()})`, {
    AbortController, TextDecoder, setTimeout, clearTimeout, fetch: async () => response,
  });
  const result = await callback({ baseUrl: "https://producer.invalid", headers: {}, jobId: "fixture", timeoutMs: 1000 });
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "producer_media_payload_too_complex");
  assert.equal(result.error.status, 502);
  assert.ok(!JSON.stringify(result).includes(url));
});
