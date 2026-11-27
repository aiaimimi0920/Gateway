// Independent bounded JSON/SSE/AWS EventStream acceptance. Never record prompts or secrets.
import assert from "node:assert/strict";
import { safeCursorErrorCode } from "./gateway_acceptance.mjs";

const utf8 = (b) => new TextDecoder("utf-8", { fatal: true }).decode(b);
export function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
export function awsEvents(bytes) {
  const events = [];
  for (let offset = 0; offset < bytes.length;) {
    assert.ok(bytes.length - offset >= 16, "aws_truncated");
    const size = bytes.readUInt32BE(offset), headersLength = bytes.readUInt32BE(offset + 4);
    assert.ok(size >= 16 && size <= 2 * 1024 * 1024 && headersLength <= size - 16, "aws_frame_bound");
    assert.ok(offset + size <= bytes.length, "aws_truncated");
    const frame = bytes.subarray(offset, offset + size);
    assert.equal(crc32(frame.subarray(0, 8)), frame.readUInt32BE(8), "aws_prelude_crc");
    assert.equal(crc32(frame.subarray(0, size - 4)), frame.readUInt32BE(size - 4), "aws_message_crc");
    const headers = {};
    for (let i = 12; i < 12 + headersLength;) {
      const length = frame[i++];
      assert.ok(i + length + 3 <= 12 + headersLength, "aws_header_bound");
      const key = utf8(frame.subarray(i, i + length)); i += length;
      assert.equal(frame[i++], 7, "aws_header_type");
      const n = frame.readUInt16BE(i); i += 2;
      assert.ok(i + n <= 12 + headersLength && headers[key] === undefined, "aws_header_bound");
      headers[key] = utf8(frame.subarray(i, i + n)); i += n;
    }
    assert.equal(headers[":message-type"], "event");
    assert.equal(headers[":content-type"], "application/json");
    assert.deepEqual(Object.keys(headers).sort(), [":content-type", ":event-type", ":message-type"]);
    events.push({ kind: headers[":event-type"], body: JSON.parse(utf8(frame.subarray(12 + headersLength, size - 4))) });
    assert.ok(events.length <= 128, "aws_event_bound");
    offset += size;
  }
  return events;
}
export function exactMeteredUsage(phases, audits) {
  if (phases.length !== 1 || audits.length !== 1) return false;
  const usage = phases[0].usage, audit = audits[0];
  return !!usage && Object.values(usage).length === 3 &&
    [usage.input, usage.output, usage.total].every((n) => Number.isSafeInteger(n) && n > 0) &&
    usage.total === usage.input + usage.output &&
    audit.promptTokens === usage.input && audit.completionTokens === usage.output && audit.totalTokens === usage.total;
}
export function meteredPhase(bytes, kind, stream, resultFormat = "text") {
  let value, eventCount = 0;
  if (kind === "bedrock" && stream) {
    const events = awsEvents(bytes); eventCount = events.length;
    assert.deepEqual(events.map((e) => e.kind),
      ["messageStart", "contentBlockDelta", "contentBlockStop", "messageStop", "metadata"], "aws_order");
    assert.deepEqual(events[0].body, { role: "assistant" });
    assert.equal(events[1].body.contentBlockIndex, 0);
    assert.deepEqual(events[2].body, { contentBlockIndex: 0 });
    assert.equal(events[3].body.stopReason, "end_turn");
    value = { output: { message: { role: "assistant", content: [{ text: events[1].body.delta.text }] } },
      stopReason: events[3].body.stopReason, ...events[4].body };
  } else if (stream) {
    const raw = utf8(bytes);
    assert.ok(raw.startsWith("event: result\ndata: ") && raw.endsWith("\n\n"), "dashscope_sse_frame");
    assert.equal(raw.split("\n\n").length, 2, "dashscope_phase_bound");
    value = JSON.parse(raw.slice("event: result\ndata: ".length, -2));
    eventCount = 1;
  } else value = JSON.parse(utf8(bytes));
  const rawUsage = value.usage;
  const usage = kind === "bedrock" ?
    { input: rawUsage?.inputTokens, output: rawUsage?.outputTokens, total: rawUsage?.totalTokens } :
    { input: rawUsage?.input_tokens, output: rawUsage?.output_tokens, total: rawUsage?.total_tokens };
  // These fixed live cases have nonempty input and output; all-zero placeholders must fail.
  assert.ok(Object.values(usage).every((n) => Number.isSafeInteger(n) && n > 0), "missing_real_usage");
  assert.equal(usage.total, usage.input + usage.output, "usage_total_mismatch");
  let text;
  if (kind === "bedrock") {
    assert.equal(value.stopReason, "end_turn");
    assert.equal(value.output?.message?.role, "assistant");
    assert.equal(value.output.message.content.length, 1);
    text = value.output.message.content[0].text;
    assert.ok(Number.isSafeInteger(value.metrics?.latencyMs) && value.metrics.latencyMs >= 0, "missing_latency");
  } else {
    assert.ok(typeof value.request_id === "string" && value.request_id.length <= 256);
    if (kind === "dashscope-multimodal" || resultFormat === "message") {
      assert.equal(value.output.text, undefined);
      const choice = value.output.choices?.[0];
      assert.ok(choice, "missing_message_format");
      assert.equal(value.output.choices.length, 1); assert.equal(choice.finish_reason, "stop");
      assert.equal(choice.message.role, "assistant"); assert.equal(choice.message.tool_calls, undefined);
      if (kind === "dashscope-multimodal") {
        assert.ok(Array.isArray(choice.message.content));
        assert.equal(choice.message.content.length, 1);
        assert.deepEqual(Object.keys(choice.message.content[0]), ["text"]);
        text = choice.message.content[0].text;
      } else text = choice.message.content;
    } else {
      assert.equal(kind, "dashscope"); assert.equal(value.output.finish_reason, "stop");
      assert.equal(value.output.choices, undefined);
      text = value.output.text;
    }
  }
  assert.equal(typeof text, "string");
  return { id: value.request_id ?? "aws-terminal", text, calls: [], thinking: "", events: eventCount, usage };
}
export function meteredEndpoint(kind, stream) {
  return kind === "bedrock" ? `/model/cursor-personal/converse${stream ? "-stream" : ""}` :
    `/api/v1/services/aigc/${kind === "dashscope" ? "text" : "multimodal"}-generation/generation`;
}
export async function meteredTurn(base, token, kind, stream, body, extra = {}) {
  const response = await fetch(base + meteredEndpoint(kind, stream), {
    method: "POST", headers: { authorization: `Bearer ${token}`, "content-type": "application/json",
      ...(stream && kind !== "bedrock" ? { "x-dashscope-sse": "enable" } : {}), ...extra },
    body: JSON.stringify(body), redirect: "error", signal: AbortSignal.timeout(100000),
  });
  const chunks = []; let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 2 * 1024 * 1024) throw new Error("metered_response_bound");
    chunks.push(chunk);
  }
  const bytes = Buffer.concat(chunks);
  if (!response.ok) {
    let value; try { value = JSON.parse(utf8(bytes)); } catch {}
    return { rejected: true, status: response.status,
      code: safeCursorErrorCode(value?.error?.code ?? value?.code) ?? "metered_remote_error" };
  }
  assert.equal(response.headers.get("content-type")?.split(";")[0], !stream ? "application/json" :
    kind === "bedrock" ? "application/vnd.amazon.eventstream" : "text/event-stream");
  return meteredPhase(bytes, kind, stream, body.parameters?.result_format ?? "text");
}
