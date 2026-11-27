// Negative acceptance is not protocol support. All cases must stop before a remote send.
import assert from "node:assert/strict";
import { safeCursorErrorCode } from "./gateway_acceptance.mjs";

export const unsupportedCases = [
  "anthropic-endpoint-rejections", "legacy-endpoint-rejections",
];

export function unsupportedRequests(name) {
  const text = "This unsupported protocol must be rejected before inference.";
  const model = "cursor-personal";
  assert.ok(unsupportedCases.includes(name), "unknown_unsupported_case");
  return [false, true].map((stream) => {
    const headers = {};
    let path, body, family, endpoint;
    if (name.startsWith("anthropic")) {
      path = "/v1/messages"; family = "anthropic_messages"; endpoint = "messages";
      headers["anthropic-version"] = "2023-06-01";
      body = { model, stream, max_tokens: 16, messages: [{ role: "user", content: text }] };
    } else {
      path = "/v1/completions"; family = "openai_legacy_completions"; endpoint = "completions";
      body = { model, stream, prompt: text }; // Omission must not bypass the legacy default cap.
    }
    return { name: stream ? "stream" : "json", path, headers, body, family, endpoint };
  });
}

export function unsupportedReply(status, raw) {
  assert.equal(status, 400, "unsupported_endpoint_status");
  const value = JSON.parse(raw);
  const code = safeCursorErrorCode(value?.error?.code ?? value?.code);
  assert.equal(code, "cursor_remote_unsupported_endpoint", "unsupported_endpoint_code");
  return { rejected: true, status, code };
}

export async function runUnsupportedCase({ name, base, token, receipt }) {
  const entries = unsupportedRequests(name);
  receipt.endpoint = entries[0].endpoint;
  receipt.protocolFamily = entries[0].family;
  receipt.transport = "http-negative-json-and-stream";
  receipt.phases = []; receipt.rejections = [];
  for (const entry of entries) {
    const response = await fetch(base + entry.path, {
      method: "POST", redirect: "error", signal: AbortSignal.timeout(15000),
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json", ...entry.headers },
      body: JSON.stringify(entry.body),
    });
    const chunks = []; let size = 0;
    for await (const chunk of response.body) {
      size += chunk.length;
      if (size > 65536) throw new Error("unsupported_response_bound");
      chunks.push(chunk);
    }
    const raw = new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks));
    receipt.rejections.push({ name: entry.name, ...unsupportedReply(response.status, raw) });
  }
  receipt.wirePassed = true;
}
