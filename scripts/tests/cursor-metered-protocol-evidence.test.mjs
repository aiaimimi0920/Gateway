import test from "node:test";
import assert from "node:assert/strict";
import { awsEvents, crc32, exactMeteredUsage, meteredPhase } from "../../tools/cursor_remote/metered_protocol_client.mjs";

const wire = { request_id: "cursor-test", output: { text: "hello", finish_reason: "stop" },
  usage: { input_tokens: 12, output_tokens: 3, total_tokens: 15 } };
const bytes = (v) => Buffer.from(JSON.stringify(v));

test("metered native JSON/SSE preserves known usage and refuses absence or bad totals", () => {
  assert.equal(meteredPhase(bytes(wire), "dashscope", false).usage.total, 15);
  assert.equal(meteredPhase(Buffer.from(`event: result\ndata: ${JSON.stringify(wire)}\n\n`), "dashscope", true).text, "hello");
  for (const usage of [undefined, null, {}, { input_tokens: 12, output_tokens: 3, total_tokens: 0 },
    { input_tokens: -1, output_tokens: 3, total_tokens: 2 },
    { input_tokens: 0, output_tokens: 0, total_tokens: 0 }]) {
    assert.throws(() => meteredPhase(bytes({ ...wire, usage }), "dashscope", false));
  }
  const multimodal = { ...wire, output: { choices: [{ finish_reason: "stop", message: {
    role: "assistant", content: [{ text: "hello" }] } }] } };
  assert.equal(meteredPhase(bytes(multimodal), "dashscope-multimodal", false).text, "hello");
});

test("DashScope response must match the requested format without extra multimodal parts", () => {
  const message = { ...wire, output: { choices: [{ finish_reason: "stop",
    message: { role: "assistant", content: "hello" } }] } };
  assert.equal(meteredPhase(bytes(message), "dashscope", false, "message").text, "hello");
  assert.throws(() => meteredPhase(bytes(wire), "dashscope", false, "message"));
  assert.throws(() => meteredPhase(bytes(message), "dashscope", false, "text"));
  for (const content of [[{ text: "hello" }, { text: "extra" }], [{ text: "hello", image: "extra" }], "hello"]) {
    const value = structuredClone(message); value.output.choices[0].message.content = content;
    assert.throws(() => meteredPhase(bytes(value), "dashscope-multimodal", false));
  }
});

test("metered audit matching fails closed for missing, zero or mismatched usage", () => {
  const phases = [{ usage: { input: 12, output: 3, total: 15 } }];
  const audits = [{ promptTokens: 12, completionTokens: 3, totalTokens: 15 }];
  assert.equal(exactMeteredUsage(phases, audits), true);
  for (const bad of [[], [{}], [{ usage: null }], [{ usage: { input: 0, output: 0, total: 0 } }],
    [{ usage: { input: true, output: 3, total: 4 } }], [...phases, ...phases]]) {
    assert.equal(exactMeteredUsage(bad, audits), false);
  }
  assert.equal(exactMeteredUsage(phases, []), false);
  assert.equal(exactMeteredUsage(phases, [{ ...audits[0], totalTokens: 16 }]), false);
});

function frame(kind, payload) {
  const headers = [];
  for (const [key, value] of Object.entries({ ":message-type": "event", ":event-type": kind, ":content-type": "application/json" })) {
    const k = Buffer.from(key), v = Buffer.from(value), size = Buffer.alloc(2);
    size.writeUInt16BE(v.length);
    headers.push(Buffer.from([k.length]), k, Buffer.from([7]), size, v);
  }
  const h = Buffer.concat(headers), p = bytes(payload), prelude = Buffer.alloc(12), tail = Buffer.alloc(4);
  prelude.writeUInt32BE(16 + h.length + p.length); prelude.writeUInt32BE(h.length, 4);
  prelude.writeUInt32BE(crc32(prelude.subarray(0, 8)), 8);
  const data = Buffer.concat([prelude, h, p]); tail.writeUInt32BE(crc32(data));
  return Buffer.concat([data, tail]);
}

test("AWS EventStream validator enforces CRC, exact terminal order and complete usage", () => {
  assert.equal(crc32(Buffer.from("123456789")), 0xcbf43926);
  const parts = [
    frame("messageStart", { role: "assistant" }),
    frame("contentBlockDelta", { contentBlockIndex: 0, delta: { text: "hello" } }),
    frame("contentBlockStop", { contentBlockIndex: 0 }),
    frame("messageStop", { stopReason: "end_turn" }),
    frame("metadata", { usage: { inputTokens: 12, outputTokens: 3, totalTokens: 15 }, metrics: { latencyMs: 1 } }),
  ];
  const valid = Buffer.concat(parts);
  assert.equal(meteredPhase(valid, "bedrock", true).usage.total, 15);
  for (const invalid of [valid.subarray(0, -1), Buffer.concat([...parts, parts[0]]), Buffer.concat(parts.slice(0, -1))]) {
    assert.throws(() => meteredPhase(invalid, "bedrock", true));
  }
  const badCrc = Buffer.from(valid); badCrc[16] ^= 1;
  assert.throws(() => awsEvents(badCrc));
});
