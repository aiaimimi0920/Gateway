import test from "node:test";
import assert from "node:assert/strict";
import { sseEvents, responsesPhase, chatPhase, phaseReceipt, exactReply, exactProtocolAudit } from
  "../../tools/cursor_remote/protocol_evidence.mjs";
import { toolResult } from "../../tools/cursor_remote/protocol_cases.mjs";
import { rejectionCases } from "../../tools/cursor_remote/protocol_rejections.mjs";

const response = { id: "resp_fixture", object: "response", status: "completed", output: [
  { id: "msg_1", type: "message", role: "assistant", content: [{ type: "output_text", text: '{"ok":true}' }] },
] };
const events = () => [
  { type: "response.created" },
  { type: "response.output_item.added", output_index: 0, item: { id: "msg_1", type: "message" } },
  { type: "response.content_part.added", item_id: "msg_1", output_index: 0, content_index: 0 },
  { type: "response.output_text.delta", item_id: "msg_1", output_index: 0, content_index: 0, delta: '{"ok":true}' },
  { type: "response.output_text.done", item_id: "msg_1", output_index: 0, content_index: 0, text: '{"ok":true}' },
  { type: "response.content_part.done", item_id: "msg_1", output_index: 0, content_index: 0, part: response.output[0].content[0] },
  { type: "response.output_item.done", output_index: 0, item: response.output[0] },
  { type: "response.completed", response },
].map((e, i) => ({ ...e, sequence_number: i + 1 }));
test("SSE requires clean boundaries, matching event name, and final DONE", () => {
  assert.deepEqual(sseEvents('data: {"type":"hello"}\n\ndata: [DONE]\n\n'), { events: [{ type: "hello" }], done: true });
  for (const raw of ['data: {}', 'event: bad\ndata: {"type":"hello"}\n\n', 'data: [DONE]\n\ndata: {}\n\n']) {
    assert.throws(() => sseEvents(raw));
  }
});
test("Responses requires complete terminal, ordered sequence, reconstructed output", () => {
  assert.equal(exactReply(responsesPhase(response, events()).text, { ok: true }), true);
  for (const mutate of [
    (e) => { e[1].sequence_number = 4; },
    (e) => { e[6].item = {}; },
    (e) => { e.push({ type: "extra", sequence_number: 9 }); },
    (e) => { e[3].delta = "wrong"; },
    (e) => { e[4].text = "wrong"; },
    (e) => { e[2].type = "error"; },
  ]) { const e = events(); mutate(e); assert.throws(() => responsesPhase(response, e)); }
});
test("Chat reconstructs role, arguments and custom input without lossy normalization", () => {
  const chunk = (delta, finish = null) => ({ id: "c1", object: "chat.completion.chunk",
    choices: [{ delta, finish_reason: finish }] });
  const phase = chatPhase(null, [
    chunk({ role: "assistant", reasoning_content: " 原生\n" }),
    chunk({ tool_calls: [{ index: 0, id: "call1", type: "custom", custom: { name: "paste", input: " \r\n中" } }] }),
    chunk({ tool_calls: [{ index: 0, custom: { input: "文&amp;\t" } }] }),
    chunk({}, "tool_calls"),
  ]);
  assert.equal(phase.calls[0].input, " \r\n中文&amp;\t");
  assert.equal(phase.thinking, " 原生\n");
  assert.equal("index" in phase.history[0].tool_calls[0], false);
  assert.throws(() => chatPhase(null, [chunk({ content: "x" }), chunk({}, "stop")]));
  assert.throws(() => chatPhase(null, [chunk({ role: "assistant" }), chunk({}, "length")]));
});
test("Receipts preserve only classifications and hashes, not arbitrary output", () => {
  const phase = responsesPhase(response), receipt = phaseReceipt(phase);
  assert.equal(JSON.stringify(receipt).includes('"ok"'), false);
  assert.equal(receipt.textSha256.length, 64);
  assert.equal(exactReply('```json\n{"ok":true}\n```', { ok: true }), false);
  assert.equal(exactReply('{"ok":true,"extra":1}', { ok: true }), false);
});
test("Audit endpoint, exact account, delivery, and attempts are mandatory", () => {
  const route = { provider: "cursor", credential: "account", revision: "r1" };
  const row = { completedAt: "now", status: "completed", endpointKind: "chat_completions", upstreamStatus: 200,
    providerAccountId: "cursor", routeTrace: { routeAttemptCount: 1, routeRevision: "r1", realCredentialRef: "account",
      selectedPipelineMode: "cursor_remote", cursorRemoteDeliveryUnknown: false } };
  assert.equal(exactProtocolAudit(row, route, "chat_completions"), true);
  assert.equal(exactProtocolAudit(row, route, "responses"), false);
  for (const change of [{ realCredentialRef: "other" }, { routeAttemptCount: 2 }, { cursorRemoteDeliveryUnknown: undefined }]) {
    assert.equal(exactProtocolAudit({ ...row, routeTrace: { ...row.routeTrace, ...change } }, route, "chat_completions"), false);
  }
});
test("Text parts split Unicode code points without creating invalid JSON strings", () => {
  const text = "中文🙂&quot;\r\n";
  for (const chat of [false, true]) {
    const result = toolResult({ id: "call", type: "custom" }, text, chat, true);
    const parts = result[chat ? "content" : "output"];
    assert.equal(parts.map((p) => p.text).join(""), text);
    assert.ok(parts.every((p) => p.text.isWellFormed()));
  }
  assert.equal(responsesPhase({ ...response, id: "relay-fixture" }).id, "relay-fixture");
});
test("Negative fixtures distinguish endpoint contracts and never ask for supported inference", () => {
  for (const chat of [false, true]) {
    const cases = rejectionCases(chat);
    assert.equal(cases.length, chat ? 9 : 8);
    assert.equal(new Set(cases.map((c) => c.name)).size, cases.length);
    assert.ok(cases.every((c) => c.code.startsWith("cursor_remote_")));
    assert.equal(cases.find((c) => c.name === "strict-tool").body.tools[0][chat ? "function" : "strict"] !== undefined, true);
    if (!chat) assert.equal("stream" in cases.find((c) => c.name === "compact").body, false);
  }
});
