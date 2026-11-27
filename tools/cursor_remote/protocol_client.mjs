// Bounded one-shot clients: no reconnection, no retries, no alternate transport/provider.
import { createRequire } from "node:module";
import { sseEvents, responsesPhase, chatPhase } from "./protocol_evidence.mjs";
import { safeCursorErrorCode } from "./gateway_acceptance.mjs";

const require = createRequire(new URL("../../scripts/package.json", import.meta.url));
const WebSocket = require("ws");
const maxBytes = 2 * 1024 * 1024;
const errorCode = (value) => safeCursorErrorCode(value?.error?.code) ?? "protocol_remote_error";
const websocketErrorCode = (event) => {
  if (event?.type !== "error") return "websocket_unexpected_terminal";
  const message = String(event.error?.message ?? "");
  return event.error?.type === "invalid_request_error" &&
    message === "Cursor previous_response_id requires the current delivered tool pause on this WebSocket"
    ? "cursor_remote_previous_response_unavailable" : "websocket_remote_error";
};

export async function httpTurn(base, token, chat, body, endpoint) {
  endpoint ??= chat ? "/v1/chat/completions" : "/v1/responses";
  if (!["/v1/chat/completions", "/v1/responses", "/v1/responses/compact"].includes(endpoint)) {
    throw new Error("invalid_protocol_endpoint");
  }
  const response = await fetch(base + endpoint, {
    method: "POST", headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify(body), redirect: "error", signal: AbortSignal.timeout(100000),
  });
  const parts = [];
  let bytes = 0;
  for await (const chunk of response.body) {
    bytes += chunk.length;
    if (bytes > maxBytes) throw new Error("protocol_response_too_large");
    parts.push(chunk);
  }
  const raw = new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(parts));
  if (!response.ok) {
    let value;
    try { value = JSON.parse(raw); } catch {}
    return { rejected: true, status: response.status, code: errorCode(value) };
  }
  if (body.stream) {
    if (!response.headers.get("content-type")?.includes("text/event-stream")) throw new Error("wrong_sse_content_type");
    const { events, done } = sseEvents(raw);
    if (chat && !done) throw new Error("chat_missing_done");
    return chat ? chatPhase(null, events) :
      responsesPhase(events.find((e) => e.type === "response.completed")?.response, events);
  }
  if (!response.headers.get("content-type")?.includes("application/json")) throw new Error("wrong_json_content_type");
  const value = JSON.parse(raw);
  if (value.usage && ["prompt_tokens_details", "input_tokens_details"]
    .some((field) => Object.hasOwn(value.usage, field))) {
    throw new Error("cursor_unproven_cache_details");
  }
  return chat ? chatPhase(value) : responsesPhase(value);
}

export async function websocketClient(base, token) {
  const socket = new WebSocket(base.replace(/^http:/, "ws:") + "/v1/responses", {
    headers: { authorization: `Bearer ${token}` }, handshakeTimeout: 10000,
    maxPayload: maxBytes, followRedirects: false, perMessageDeflate: false,
  });
  let pending, fault;
  const fail = (code) => {
    fault = code;
    if (pending) { const current = pending; pending = null; clearTimeout(current.timer); current.reject(new Error(code)); }
  };
  socket.on("error", () => fail("websocket_transport_error"));
  socket.on("close", () => { if (pending) fail("websocket_closed_before_terminal"); });
  socket.on("message", (bytes, binary) => {
    if (!pending) { fail("websocket_unsolicited_frame"); socket.terminate(); return; }
    if (binary) { fail("websocket_binary_frame"); socket.terminate(); return; }
    const current = pending;
    try {
      current.bytes += bytes.length;
      if (current.bytes > maxBytes || current.events.length >= 512) throw new Error("websocket_response_bound");
      const event = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
      current.events.push(event);
      if (["error", "response.failed", "response.incomplete", "response.completed"].includes(event.type)) {
        if (event.type !== "response.completed" && event.type !== "error") {
          throw new Error("websocket_upstream_terminal_failure");
        }
        const value = event.type === "response.completed" ? responsesPhase(event.response, current.events) :
          { rejected: true, status: event.status ?? event.error?.status ?? null,
            code: websocketErrorCode(event), type: event.type };
        pending = null;
        clearTimeout(current.timer);
        current.resolve(value);
      }
    } catch (error) {
      pending = null;
      clearTimeout(current.timer);
      current.reject(error);
      socket.terminate();
    }
  });
  await new Promise((resolve, reject) => {
    socket.once("open", resolve);
    socket.once("error", () => reject(new Error("websocket_open_failed")));
    socket.once("close", () => reject(new Error("websocket_open_closed")));
  });
  return {
    turn(body) {
      if (pending || fault || socket.readyState !== WebSocket.OPEN) throw new Error("websocket_not_available");
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => { fail("websocket_turn_timeout"); socket.terminate(); }, 100000);
        pending = { resolve, reject, timer, events: [], bytes: 0 };
        socket.send(JSON.stringify({ ...body, type: "response.create" }), (error) => {
          if (error) fail("websocket_send_failed");
        });
      });
    },
    async close() {
      if (socket.readyState !== WebSocket.CLOSED) {
        await new Promise((resolve) => {
          const timer = setTimeout(() => socket.terminate(), 3000);
          socket.once("close", () => { clearTimeout(timer); resolve(); });
          socket.close(1000);
        });
      }
      if (fault) throw new Error(fault);
    },
  };
}
