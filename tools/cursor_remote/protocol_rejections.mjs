// Real ingress checks with known unsupported semantics. Every request must stop before upstream send.
import assert from "node:assert/strict";
import { httpTurn } from "./protocol_client.mjs";

export function rejectionCases(chat) {
  const baseline = { model: "cursor-personal", stream: false,
    [chat ? "messages" : "input"]: [{ role: "user", content: "This request must be rejected before inference." }] };
  if (!chat) baseline.store = false;
  const wrap = ({ type, ...tool }) => chat ? { type, [type]: tool } : { type, ...tool };
  const func = { type: "function", name: "echo", parameters: { type: "object" } };
  const unsupported = "cursor_remote_unsupported_request", badTool = "cursor_remote_unsupported_tool_request";
  const fields = [
    ["store", { store: true }, unsupported],
    ["previous-id", { previous_response_id: "resp_unavailable" }, "cursor_remote_previous_response_unavailable"],
    ["strict-tool", { tools: [wrap({ ...func, strict: true })] }, badTool],
    ["grammar-tool", { tools: [wrap({ type: "custom", name: "edit",
      format: { type: "grammar", syntax: "lark", definition: 'start: "ok"' } })] }, badTool],
    ["required-tool", { tools: [wrap(func)], tool_choice: "required" }, unsupported],
    ["reasoning", chat ? { reasoning_effort: "high" } : { reasoning: { effort: "high" } }, unsupported],
    ["structured-output", chat ? { response_format: { type: "json_object" } } :
      { text: { format: { type: "json_schema", name: "x", schema: { type: "object" }, strict: true } } }, unsupported],
  ];
  if (chat) {
    fields.push(["multiple-choices", { n: 2 }, unsupported], ["token-limit", { max_tokens: 16 }, unsupported]);
  }
  const result = fields.map(([name, change, code]) => ({ name, body: { ...baseline, ...change }, code }));
  if (!chat) {
    const { stream: _, ...compact } = baseline;
    result.push({ name: "compact", body: compact, code: "cursor_remote_compact_unsupported",
      endpoint: "/v1/responses/compact" });
  }
  return result;
}

export async function runRejections({ base, token, chat, receipt }) {
  receipt.endpoint = chat ? "chat_completions" : "responses";
  receipt.transport = "http-json-negative";
  receipt.phases = [];
  receipt.rejections = [];
  for (const entry of rejectionCases(chat)) {
    const response = await httpTurn(base, token, chat, entry.body, entry.endpoint);
    receipt.rejections.push({ ...response, name: entry.name });
    assert.equal(response.rejected, true, "unsupported_request_was_accepted");
    assert.equal(response.status, 400, "unexpected_rejection_status");
    assert.equal(response.code, entry.code, "unexpected_rejection_code");
  }
  receipt.wirePassed = true;
}
