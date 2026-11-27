// Validate the public wire independently of account audit; keep model text in memory only.
import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
import { sha256 } from "./scenario_evidence.mjs";
import { responseLifecycle } from "./protocol_lifecycle.mjs";

export function sseEvents(raw) {
  const frames = raw.replaceAll("\r\n", "\n").split("\n\n");
  assert.equal(frames.pop(), "", "sse_missing_final_separator");
  const result = [];
  let done = false;
  for (const frame of frames) {
    const lines = frame.split("\n"), data = lines.filter((s) => s.startsWith("data:"))
      .map((s) => s.slice(5).replace(/^ /, "")).join("\n");
    if (!data) continue;
    assert.equal(done, false, "sse_data_after_done");
    if (data === "[DONE]") { done = true; continue; }
    const value = JSON.parse(data);
    const name = lines.find((s) => s.startsWith("event:"))?.slice(6).trim();
    if (name) assert.equal(name, value.type, "sse_event_name_mismatch");
    result.push(value);
  }
  return { events: result, done };
}

export function responsesPhase(value, events = []) {
  assert.equal(value?.object, "response", "responses_wrong_object");
  assert.equal(value.status, "completed", "responses_not_completed");
  assert.ok(typeof value.id === "string" && value.id.length > 0 && value.id.length <= 256, "responses_missing_id");
  assert.ok(Array.isArray(value.output), "responses_missing_output");
  if (events.length) {
    assert.equal(events.at(-1)?.type, "response.completed", "responses_terminal_not_last");
    assert.ok(events.every((e) => !["error", "response.failed", "response.incomplete"].includes(e.type)),
      "responses_failure_event_present");
    assert.equal(events.filter((e) => e.type === "response.created").length, 1, "responses_created_count");
    assert.equal(events.filter((e) => e.type === "response.completed").length, 1, "responses_terminal_count");
    assert.deepEqual(events.at(-1).response, value, "responses_terminal_not_last");
    events.forEach((e, i) => assert.equal(e.sequence_number, i + 1, "responses_sequence_gap"));
    assert.deepEqual(events.filter((e) => e.type === "response.output_item.done").map((e) => e.item),
      value.output, "responses_output_reconstruction");
    responseLifecycle(value, events);
  }
  const calls = value.output.filter((i) => ["function_call", "custom_tool_call"].includes(i.type))
    .map((i) => ({ id: i.call_id, type: i.type === "function_call" ? "function" : "custom",
      name: i.name, input: i.type === "function_call" ? i.arguments : i.input }));
  const text = value.output.filter((i) => i.type === "message").flatMap((i) => {
    assert.equal(i.role, "assistant", "responses_wrong_role");
    return i.content.filter((c) => c.type === "output_text").map((c) => c.text);
  }).join("");
  const thinking = value.output.filter((i) => i.type === "reasoning").map((i) => {
    assert.deepEqual(i.summary, [], "responses_unexpected_reasoning_summary");
    assert.equal(i.encrypted_content, undefined, "responses_unexpected_ciphertext");
    assert.equal(i.status, "completed", "responses_reasoning_status");
    const parts = i.content.map((c) => {
      assert.equal(c.type, "reasoning_text", "responses_reasoning_type");
      return c.text;
    });
    if (events.length && parts.join("").length) {
      assert.ok(events.some((e) => e.type === "response.reasoning_text.delta" && e.item_id === i.id),
        "responses_reasoning_delta_missing");
    }
    return parts.join("");
  }).join("");
  return { id: value.id, text, calls, thinking, history: value.output, events: events.length };
}

export function chatPhase(value, events = null) {
  let message, finish, id;
  if (events) {
    message = { content: "" };
    const calls = new Map();
    for (const e of events) {
      assert.equal(e.object, "chat.completion.chunk", "chat_wrong_chunk");
      assert.equal(e.usage, undefined, "chat_unexpected_usage");
      id ??= e.id;
      assert.equal(e.id, id, "chat_id_changed");
      assert.equal(e.choices.length, 1, "chat_choice_count");
      const choice = e.choices[0], delta = choice.delta;
      assert.equal(finish, undefined, "chat_data_after_finish");
      if (delta.role) message.role = delta.role;
      if (delta.content != null) message.content += delta.content;
      if (delta.reasoning_content != null) {
        message.reasoning_content = (message.reasoning_content ?? "") + delta.reasoning_content;
      }
      for (const part of delta.tool_calls ?? []) {
        assert.ok(Number.isInteger(part.index) && part.index >= 0 && part.index < 64, "chat_tool_index");
        const call = calls.get(part.index) ?? {};
        if (part.id) call.id = part.id;
        if (part.type) call.type = part.type;
        for (const kind of ["function", "custom"]) if (part[kind]) {
          const input = kind === "function" ? "arguments" : "input";
          call[kind] ??= { name: "", [input]: "" };
          call[kind].name += part[kind].name ?? "";
          call[kind][input] += part[kind][input] ?? "";
        }
        calls.set(part.index, call);
      }
      if (choice.finish_reason != null) finish = choice.finish_reason;
    }
    if (calls.size) {
      assert.deepEqual([...calls.keys()], Array.from({ length: calls.size }, (_, i) => i), "chat_tool_index_gap");
      message.tool_calls = [...calls.values()];
    }
  } else {
    assert.equal(value?.object, "chat.completion", "chat_wrong_object");
    if (value.usage !== undefined) {
      const usage = value.usage;
      assert.ok([usage.prompt_tokens, usage.completion_tokens, usage.total_tokens]
        .every((n) => Number.isSafeInteger(n) && n >= 0), "chat_invalid_usage");
      assert.equal(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens, "chat_invalid_usage");
    }
    assert.equal(value.choices.length, 1, "chat_choice_count");
    ({ message, finish_reason: finish } = value.choices[0]);
    id = value.id;
  }
  assert.equal(message.role, "assistant", "chat_missing_role");
  const calls = (message.tool_calls ?? []).map((c) => ({ id: c.id, type: c.type,
    name: c[c.type].name, input: c[c.type][c.type === "function" ? "arguments" : "input"] }));
  assert.equal(finish, calls.length ? "tool_calls" : "stop", "chat_wrong_finish_reason");
  return { id, text: message.content ?? "", calls, thinking: message.reasoning_content ?? "",
    history: [message], events: events?.length ?? 0 };
}

export function phaseReceipt(phase) {
  return { id: phase.id, textBytes: Buffer.byteLength(phase.text), textSha256: sha256(phase.text),
    thinkingBytes: Buffer.byteLength(phase.thinking), thinkingSha256: sha256(phase.thinking),
    eventCount: phase.events, calls: phase.calls.map((c) => ({ id: c.id, type: c.type, name: c.name,
      inputBytes: Buffer.byteLength(c.input), inputSha256: sha256(c.input) })) };
}

export function exactReply(text, expected) {
  try { return isDeepStrictEqual(JSON.parse(text), expected); } catch { return false; }
}

export function exactProtocolAudit(row, route, endpoint) {
  const t = row.routeTrace;
  return !!row.completedAt && row.status === "completed" && row.endpointKind === endpoint &&
    row.upstreamStatus === 200 && row.providerAccountId === route.provider &&
    t?.routeAttemptCount === 1 && t.routeRevision === route.revision &&
    t.realCredentialRef === route.credential && t.selectedPipelineMode === "cursor_remote" &&
    t.cursorRemoteDeliveryUnknown === false && t.errorCode == null &&
    (t.fallbackEligible == null || t.fallbackEligible === false);
}
