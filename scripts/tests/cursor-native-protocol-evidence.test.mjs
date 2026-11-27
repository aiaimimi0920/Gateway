import test from "node:test";
import assert from "node:assert/strict";
import { nativePhase } from "../../tools/cursor_remote/native_protocol_client.mjs";

const call = { id: "call_one", type: "function", function: { name: "echo", arguments: '{"text":"中文🦀"}' } };
const cohere = { id: "native_one", message: { role: "assistant",
  content: [{ type: "text", text: "prelude" }], tool_calls: [call] }, finish_reason: "TOOL_CALL" };
const gemini = { responseId: "native_one", candidates: [{ index: 0, finishReason: "STOP",
  content: { role: "model", parts: [{ text: "prelude" }, {
    functionCall: { id: call.id, name: "echo", args: { text: "中文🦀" } },
  }] } }] };
const events = [
  { type: "message-start", id: "native_one", delta: { message: { role: "assistant" } } },
  { type: "content-start", index: 0, delta: { message: { content: { type: "text", text: "" } } } },
  { type: "content-delta", index: 0, delta: { message: { content: { text: "prelude" } } } },
  { type: "content-end", index: 0 },
  { type: "tool-call-start", index: 0, delta: { message: { tool_calls: {
    id: call.id, type: "function", function: { name: "echo", arguments: "" },
  } } } },
  { type: "tool-call-delta", index: 0, delta: { message: { tool_calls: { function: { arguments: call.function.arguments } } } } },
  { type: "tool-call-end", index: 0 },
  { type: "message-end", delta: { finish_reason: "TOOL_CALL" } },
];
const sse = (values) => values.map((e) => `event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`).join("");

test("native protocol JSON/SSE returns equivalent exact history", () => {
  for (const [isGemini, value, raw] of [
    [true, gemini, `data: ${JSON.stringify(gemini)}\n\n`], [false, cohere, sse(events)],
  ]) {
    const json = nativePhase(JSON.stringify(value), isGemini, false);
    const stream = nativePhase(raw, isGemini, true);
    assert.equal(json.text, "prelude");
    assert.deepEqual(json.calls, stream.calls);
    assert.deepEqual(json.history, stream.history);
    assert.equal(json.thinking, "");
    assert.deepEqual(JSON.parse(json.calls[0].input), { text: "中文🦀" });
  }
});

test("native protocol rejects fake usage, incomplete and wrong terminals", () => {
  for (const [isGemini, baseline] of [[true, gemini], [false, cohere]]) {
    const bad = structuredClone(baseline);
    bad.usage = { input_tokens: 0, output_tokens: 0 };
    assert.throws(() => nativePhase(JSON.stringify(bad), isGemini, false));
    if (isGemini) bad.candidates[0].finishReason = "MAX_TOKENS";
    else bad.finish_reason = "COMPLETE";
    delete bad.usage;
    assert.throws(() => nativePhase(JSON.stringify(bad), isGemini, false));
  }
  for (let index = 0; index < events.length; index++) {
    assert.throws(() => nativePhase(sse(events.filter((_, i) => i !== index)), false, true));
  }
  for (const raw of [sse(events).slice(0, -1), sse([...events, events[0]]), sse(events) + "data: [DONE]\n\n"]) {
    assert.throws(() => nativePhase(raw, false, true));
  }
  assert.throws(() => nativePhase(sse(events).replaceAll(/^event:.*\n/gm, ""), false, true));
});

test("native protocol tool contract checks object arguments and content discriminator", () => {
  const badGemini = structuredClone(gemini);
  badGemini.candidates[0].content.parts[1].functionCall.args = "not an object";
  assert.throws(() => nativePhase(JSON.stringify(badGemini), true, false));
  const badCohere = structuredClone(cohere);
  delete badCohere.message.content[0].type;
  assert.throws(() => nativePhase(JSON.stringify(badCohere), false, false));
  const arrayEvent = structuredClone(events);
  arrayEvent[4].delta.message.tool_calls = [arrayEvent[4].delta.message.tool_calls];
  assert.throws(() => nativePhase(sse(arrayEvent), false, true));
});
