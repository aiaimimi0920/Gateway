// Narrow protobuf/Connect subset from the pinned descriptor; no local tool execution.
import { randomUUID } from "node:crypto";
import { ProbeError } from "./transport.mjs";
import { uniqueJson } from "./json_wire.mjs";
import { field, fields, frame } from "./protobuf_wire.mjs";
import { validateEndStream } from "./connect_error.mjs";
import { connectMessageSignals, noMessageSignals } from "./error_signals.mjs";
import { TextSession } from "./text_session.mjs";
import { userMessage } from "./user_message.mjs";
export { field, fields, frame } from "./protobuf_wire.mjs";

export const expectedReply = "CURSOR REMOTE OK - 2 + 3 = 5";

export function textRequest(model, prompt = `Reply with exactly: ${expectedReply}`, maxBytes = 1024, images) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:/+-]{0,159}$/.test(model) ||
      !Number.isInteger(maxBytes) || maxBytes < 1 || maxBytes > 64 * 1024 ||
      typeof prompt !== "string" || Buffer.byteLength(prompt) > maxBytes) {
    throw new ProbeError("invalid_text_request");
  }
  const conversation = randomUUID();
  const user = userMessage(prompt, 2, images);
  // Main CLI UserMessageAction carries RequestContext. This client has no workspace
  // or tools; explicitly disable the two optional remote web capabilities.
  const context = Buffer.concat([field(17, 0), field(24, 0)]);
  const action = field(1, Buffer.concat([field(1, user), field(2, context)]));
  const run = Buffer.concat([
    field(1, Buffer.alloc(0)), field(2, action), field(4, Buffer.alloc(0)),
    field(5, conversation), field(9, field(1, model)), field(16, conversation),
    // SDK constructs an empty McpTools, a fresh runId and present optional uint32 zero.
    // These preserve field presence without declaring tools or expanding ASK permissions.
    field(25, randomUUID()), field(34, 0),
  ]);
  return frame(field(1, run));
}

export function oneof(bytes, metadata = [], diagnose = () => {}) {
  const parsed = fields(bytes);
  diagnose(parsed.map((item) => item.tag).slice(0, 16));
  const seen = new Set(), messages = [];
  for (const item of parsed) {
    if (seen.has(item.tag)) throw new ProbeError("unexpected_protobuf_message");
    seen.add(item.tag);
    const meta = metadata.find(([tag]) => item.tag === tag);
    if (meta) {
      if (item.wire !== meta[1]) throw new ProbeError("unexpected_protobuf_message");
    } else messages.push(item);
  }
  if (messages.length > 1 || messages.some((item) => item.wire !== 2) || !parsed.length) {
    throw new ProbeError("unexpected_protobuf_message");
  }
  return messages[0];
}

function validateTurnEnd(bytes) {
  const seen = new Set();
  for (const item of fields(bytes)) {
    // Known usage counters are varints; do not report them as billable/free units.
    if (item.tag <= 5 && (item.wire !== 0 || seen.has(item.tag))) {
      throw new ProbeError("invalid_turn_end");
    }
    seen.add(item.tag);
  }
}

export class TextDecoderState {
  constructor({ allowSessionMessages = false } = {}) {
    this.session = allowSessionMessages ? new TextSession() : null;
    this.replies = [];
  }
  buffer = Buffer.alloc(0);
  text = "";
  thinking = "";
  thinkingBytes = 0;
  thinkingTotalBytes = 0;
  turnEnded = false;
  endStream = false;
  remoteConnectCode = null;
  remoteCursorError = null;
  remoteCursorDetailStatus = "absent";
  remoteCursorRetryable = null;
  remoteCursorAction = null;
  remoteCursorHintsStatus = "absent";
  remoteCursorMessageSignals = [];
  remoteCursorMessageStatus = "absent";
  messageProjection = { ...noMessageSignals(), ...connectMessageSignals() };
  frames = 0;
  events = new Set();
  lastServerTags = [];
  lastUpdateTags = [];
  lastTextTags = [];

  push(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 5) {
      const flags = this.buffer[0], size = this.buffer.readUInt32BE(1);
      if (![0, 2].includes(flags)) throw new ProbeError("unsupported_connect_flags");
      if (size > 256 * 1024) throw new ProbeError("connect_frame_too_large");
      if (this.endStream) throw new ProbeError("data_after_end_stream");
      if (this.buffer.length < 5 + size) return;
      const bytes = this.buffer.subarray(5, 5 + size);
      this.buffer = this.buffer.subarray(5 + size);
      if (++this.frames > 4096) throw new ProbeError("connect_frame_limit");
      if (flags === 2) {
        let ending;
        try { ending = uniqueJson(bytes); }
        catch { throw new ProbeError("invalid_end_stream"); }
        const error = validateEndStream(ending);
        this.remoteConnectCode = error.remoteConnectCode;
        this.remoteCursorError = error.remoteCursorError;
        this.remoteCursorDetailStatus = error.remoteCursorDetailStatus;
        this.remoteCursorRetryable = error.remoteCursorRetryable;
        this.remoteCursorAction = error.remoteCursorAction;
        this.remoteCursorHintsStatus = error.remoteCursorHintsStatus;
        this.remoteCursorMessageSignals = error.remoteCursorMessageSignals;
        this.remoteCursorMessageStatus = error.remoteCursorMessageStatus;
        for (const key of Object.keys(this.messageProjection)) this.messageProjection[key] = error[key];
        this.endStream = true;
      } else this.message(bytes);
    }
  }

  message(bytes) {
    // The current schema has timing metadata outside the oneof, not a second action.
    const envelope = oneof(bytes, [[8, 2]], (tags) => { this.lastServerTags = tags; });
    if (!envelope) return;
    this.events.add(`server:${envelope.tag}`);
    if (this.session && [2, 4].includes(envelope.tag)) {
      if (this.turnEnded) throw new ProbeError("remote_client_action_required");
      this.replies.push(this.session.reply(envelope));
      return;
    }
    if ([2, 4, 5, 7].includes(envelope.tag)) throw new ProbeError("remote_client_action_required");
    if ([3, 8].includes(envelope.tag)) return; // Opaque checkpoint or timing metadata; never store/replay.
    if (envelope.tag !== 1) throw new ProbeError("unknown_server_event");
    const event = oneof(envelope.value, [[25, 0]], (tags) => { this.lastUpdateTags = tags; });
    if (!event) throw new ProbeError("unexpected_protobuf_message");
    this.events.add(`update:${event.tag}`);
    if ([2, 3, 7, 12, 15, 19, 21, 22, 27, 30].includes(event.tag)) {
      throw new ProbeError("remote_tool_or_interaction_required");
    }
    if (event.tag === 14) {
      if (this.turnEnded) throw new ProbeError("duplicate_turn_end");
      validateTurnEnd(event.value);
      this.turnEnded = true;
    } else if (event.tag === 4) {
      if (this.turnEnded) throw new ProbeError("thinking_after_turn_end");
      const parts = fields(event.value);
      if (parts.some((item) => ![1, 2].includes(item.tag)) ||
          new Set(parts.map((item) => item.tag)).size !== parts.length) throw new ProbeError("unexpected_thinking_delta");
      const text = parts.find((item) => item.tag === 1), style = parts.find((item) => item.tag === 2);
      if ((text && text.wire !== 2) || (style && style.wire !== 0)) throw new ProbeError("unexpected_thinking_delta");
      let delta;
      try { delta = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(text?.value ?? Buffer.alloc(0)); }
      catch { throw new ProbeError("invalid_thinking_utf8"); }
      const size = Buffer.byteLength(delta);
      if (this.thinkingBytes + size > 16 * 1024 || this.thinkingTotalBytes + size > 64 * 1024) {
        throw new ProbeError("thinking_too_large");
      }
      this.thinkingBytes += size;
      this.thinkingTotalBytes += size;
      this.thinking += delta;
    } else if (event.tag === 1) {
      if (this.turnEnded) throw new ProbeError("text_after_turn_end");
      const parts = fields(event.value);
      this.lastTextTags = parts.map((item) => item.tag).slice(0, 16);
      if (parts.some((item) => ![1, 2].includes(item.tag)) ||
          new Set(parts.map((item) => item.tag)).size !== parts.length) throw new ProbeError("unexpected_text_delta");
      const text = parts.find((item) => item.tag === 1), notice = parts.find((item) => item.tag === 2);
      if ((text && text.wire !== 2) ||
          (notice && (notice.wire !== 0 || notice.value > 1n))) throw new ProbeError("unexpected_text_delta");
      // Server notices are not model output and cannot make an empty turn successful.
      if (notice?.value === 1n) return;
      try { this.text += new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(text?.value ?? Buffer.alloc(0)); }
      catch { throw new ProbeError("invalid_text_utf8"); }
      if (Buffer.byteLength(this.text) > 64 * 1024) throw new ProbeError("text_too_large");
    } else if (![5, 6, 8, 9, 10, 11, 13, 16, 17, 18, 20, 23, 24, 26, 28, 31].includes(event.tag)) {
      throw new ProbeError("unknown_interaction_event");
    }
  }

  finish() {
    if (this.buffer.length) throw new ProbeError("connect_truncated");
    if (this.remoteConnectCode !== null) throw new ProbeError("connect_remote_error");
    if (!this.text.trim() || !this.turnEnded || !this.endStream) {
      throw new ProbeError("successful_terminal_not_confirmed");
    }
    return { text: this.text, thinking: this.thinking, frames: this.frames, turnEnded: true, endStream: true };
  }

  takePhase() {
    const phase = { text: this.text, thinking: this.thinking };
    this.text = "";
    this.thinking = "";
    this.thinkingBytes = 0; // The cumulative Run budget is deliberately not reset.
    return phase;
  }

  takeReplies() {
    if (this.endStream && this.replies.length) throw new ProbeError("remote_client_action_required");
    return this.replies.splice(0);
  }

  close() {
    this.session?.close();
    this.replies.length = 0;
  }

  receipt() {
    return { frames: this.frames, textBytes: Buffer.byteLength(this.text),
      turnEnded: this.turnEnded, endStream: this.endStream, remoteConnectCode: this.remoteConnectCode,
      remoteCursorError: this.remoteCursorError, remoteCursorDetailStatus: this.remoteCursorDetailStatus,
      remoteCursorRetryable: this.remoteCursorRetryable, remoteCursorAction: this.remoteCursorAction,
      remoteCursorHintsStatus: this.remoteCursorHintsStatus,
      remoteCursorMessageSignals: this.remoteCursorMessageSignals,
      remoteCursorMessageStatus: this.remoteCursorMessageStatus,
      ...this.messageProjection,
      lastServerTags: this.lastServerTags, lastUpdateTags: this.lastUpdateTags, lastTextTags: this.lastTextTags,
      events: [...this.events].sort() };
  }
}
