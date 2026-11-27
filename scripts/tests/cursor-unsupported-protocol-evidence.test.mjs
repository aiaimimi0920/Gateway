import test from "node:test";
import assert from "node:assert/strict";
import { unsupportedCases, unsupportedRequests, unsupportedReply } from "../../tools/cursor_remote/unsupported_protocol_cases.mjs";
import { protocolCases, protocolScope } from "../../tools/cursor_remote/protocol_cases.mjs";

test("unsupported endpoints have native JSON/stream requests and explicit relay scope", () => {
  assert.equal(unsupportedCases.length, 2);
  const paths = new Set();
  for (const name of unsupportedCases) {
    assert.ok(protocolCases.includes(name));
    assert.equal(protocolScope(name), "relay");
    const entries = unsupportedRequests(name);
    assert.deepEqual(entries.map((e) => e.name), ["json", "stream"]);
    assert.equal(entries[0].family, entries[1].family);
    entries.forEach((e) => paths.add(e.path));
  }
  assert.equal(paths.size, 2);
  assert.deepEqual(unsupportedCases.map((name) => unsupportedRequests(name)[0].family),
    ["anthropic_messages", "openai_legacy_completions"]);
  assert.equal(unsupportedRequests(unsupportedCases[0])[0].body.max_tokens, 16);
  assert.equal(unsupportedRequests(unsupportedCases[1])[0].body.max_tokens, undefined);
  assert.equal(protocolScope("bedrock-json-text"), "bedrock.converse");
  assert.equal(protocolScope("dashscope-json-text"), "dashscope.text");
  assert.equal(protocolScope("dashscope-multimodal-stream-image"), "dashscope.multimodal");
  assert.equal(protocolScope("gemini-json-tools"), "gemini.generateContent");
  assert.equal(protocolScope("cohere-sse-tools"), "cohere.chat");
  assert.equal(protocolScope("chat-json-tools"), "chat.completions");
  assert.equal(protocolScope("ws-tools-image"), "responses");
  assert.throws(() => unsupportedRequests("unknown"));
});

test("negative acceptance rejects successful responses, scope failures and generic errors", () => {
  for (const body of [
    { error: { code: "cursor_remote_unsupported_endpoint" } }, { code: "cursor_remote_unsupported_endpoint" },
  ]) assert.equal(unsupportedReply(400, JSON.stringify(body)).rejected, true);
  for (const [status, body] of [
    [200, { error: { code: "cursor_remote_unsupported_endpoint" } }],
    [403, { error: { code: "permission_denied" } }],
    [400, { error: { code: "cursor_remote_unsupported_request" } }],
    [400, { code: "bad_request" }],
  ]) assert.throws(() => unsupportedReply(status, JSON.stringify(body)));
});
