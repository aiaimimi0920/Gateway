// Independent finite-wire validator. No model text or credentials leave memory.
import assert from "node:assert/strict";
import { safeCursorErrorCode } from "./gateway_acceptance.mjs";
import { sseEvents } from "./protocol_evidence.mjs";

const object = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
function cohereStream(events) {
  assert.equal(events[0]?.type, "message-start", "cohere_start");
  assert.equal(events[0]?.delta?.message?.role, "assistant", "cohere_role");
  const value = { id: events[0].id, message: { role: "assistant" } };
  let index = 1;
  if (events[index]?.type === "content-start") {
    const start = events[index++], delta = events[index++], end = events[index++];
    assert.equal(start.index, 0);
    assert.deepEqual(start.delta.message.content, { type: "text", text: "" });
    assert.equal(delta.type, "content-delta"); assert.equal(delta.index, 0);
    assert.equal(typeof delta.delta?.message?.content?.text, "string");
    assert.deepEqual(end, { type: "content-end", index: 0 });
    value.message.content = [{ type: "text", text: delta.delta.message.content.text }];
  }
  if (events[index]?.type === "tool-call-start") {
    const start = events[index++], delta = events[index++], end = events[index++];
    assert.equal(start.index, 0); assert.equal(delta.type, "tool-call-delta"); assert.equal(delta.index, 0);
    const call = structuredClone(start.delta.message.tool_calls);
    assert.ok(object(call), "cohere_single_call_object");
    assert.equal(call.function.arguments, "");
    assert.equal(typeof delta.delta?.message?.tool_calls?.function?.arguments, "string");
    call.function.arguments = delta.delta.message.tool_calls.function.arguments;
    assert.deepEqual(end, { type: "tool-call-end", index: 0 });
    value.message.tool_calls = [call];
  }
  assert.equal(events[index]?.type, "message-end", "cohere_end");
  value.finish_reason = events[index++].delta.finish_reason;
  assert.equal(index, events.length, "cohere_data_after_end");
  assert.ok(events.every((e) => e.usage === undefined && e.delta?.usage === undefined), "invented_usage");
  return value;
}

export function nativePhase(raw, gemini, stream) {
  let value, events = [];
  if (stream) {
    const decoded = sseEvents(raw);
    assert.equal(decoded.done, false, "native_openai_done");
    events = decoded.events;
    if (gemini) {
      assert.equal(events.length, 1, "gemini_buffered_phase_count");
      assert.ok(!raw.includes("event:"), "gemini_named_event");
      value = events[0];
    } else {
      const names = raw.split("\n").filter((l) => l.startsWith("event:")).map((l) => l.slice(6).trim());
      assert.deepEqual(names, events.map((e) => e.type), "cohere_missing_event_name");
      value = cohereStream(events);
    }
  } else value = JSON.parse(raw);
  assert.equal(value.usage, undefined);
  assert.equal(value.usageMetadata, undefined);
  assert.equal(value.modelVersion, undefined);
  let message, text, calls, id;
  if (gemini) {
    assert.equal(value.candidates?.length, 1, "gemini_candidate_count");
    const candidate = value.candidates[0];
    assert.equal(candidate.index, 0); assert.equal(candidate.finishReason, "STOP");
    message = candidate.content; assert.equal(message.role, "model");
    assert.ok(Array.isArray(message.parts), "gemini_parts");
    const texts = [], functions = [];
    for (const part of message.parts) {
      assert.equal(Object.keys(part).length, 1);
      if (typeof part.text === "string") texts.push(part.text);
      else {
        const call = part.functionCall;
        assert.ok(object(call?.args), "gemini_args_object");
        functions.push({ id: call.id, type: "function", name: call.name, input: JSON.stringify(call.args) });
      }
    }
    text = texts.join(""); calls = functions; id = value.responseId;
  } else {
    message = value.message; assert.equal(message.role, "assistant");
    assert.equal(message.tool_plan, undefined); assert.equal(message.reasoning_content, undefined);
    text = (message.content ?? []).map((p) => { assert.equal(p.type, "text"); assert.equal(typeof p.text, "string"); return p.text; }).join("");
    calls = (message.tool_calls ?? []).map((call) => {
      assert.equal(call.type, "function");
      assert.ok(object(JSON.parse(call.function.arguments)), "cohere_args_object");
      return { id: call.id, type: "function", name: call.function.name, input: call.function.arguments };
    });
    assert.equal(value.finish_reason, calls.length ? "TOOL_CALL" : "COMPLETE", "cohere_finish");
    id = value.id;
  }
  assert.ok(typeof id === "string" && id.length > 0 && id.length <= 256, "native_response_id");
  assert.ok(calls.length <= 1, "native_serial_call_count");
  for (const call of calls) {
    assert.ok(typeof call.id === "string" && call.id.length > 0 && call.id.length <= 256, "native_call_id");
    assert.ok(typeof call.name === "string" && call.name.length > 0, "native_call_name");
  }
  return { id, text, calls, history: [message], thinking: "", events: events.length };
}

export function nativeEndpoint(gemini, stream) {
  return gemini ? `/v1beta/models/cursor-personal:${stream ? "streamGenerateContent?alt=sse" : "generateContent"}` : "/v2/chat";
}

export async function nativeTurn(base, token, gemini, stream, body) {
  const response = await fetch(base + nativeEndpoint(gemini, stream), {
    method: "POST", headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify(body), redirect: "error", signal: AbortSignal.timeout(100000),
  });
  const chunks = []; let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 2 * 1024 * 1024) throw new Error("native_response_bound");
    chunks.push(chunk);
  }
  const raw = new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks));
  if (!response.ok) {
    let value; try { value = JSON.parse(raw); } catch {}
    return { rejected: true, status: response.status, code: safeCursorErrorCode(value?.error?.code) ?? "native_remote_error" };
  }
  assert.ok(response.headers.get("content-type")?.includes(stream ? "text/event-stream" : "application/json"), "native_content_type");
  return nativePhase(raw, gemini, stream);
}
