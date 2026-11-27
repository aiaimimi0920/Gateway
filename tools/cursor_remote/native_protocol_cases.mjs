// Opt-in Gemini/Cohere acceptance. Only an in-memory echo function runs, never model-supplied code.
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { nativeTurn } from "./native_protocol_client.mjs";
import { exactReply, phaseReceipt } from "./protocol_evidence.mjs";

export const nativeCases = [
  "gemini-json-text", "gemini-sse-text", "cohere-json-text", "cohere-sse-text",
  "gemini-json-tools", "gemini-sse-tools", "cohere-json-tools", "cohere-sse-tools",
  "gemini-json-image", "gemini-sse-image", "gemini-none", "cohere-none",
  "gemini-rejections", "cohere-rejections",
];
const nonce = () => randomBytes(12).toString("hex");
const schema = { type: "object", properties: { text: { type: "string" } }, required: ["text"] };
const fn = { name: "echo", description: "Return a private receipt for the supplied text.", parameters: schema };
function request(gemini, stream, history, choice, openapi = false) {
  if (gemini) {
    const result = { contents: history };
    if (choice) {
      result.tools = [{ functionDeclarations: [{ name: fn.name, description: fn.description,
        ...(openapi ? { parameters: { type: "OBJECT", properties: { text: { type: "STRING" } }, required: ["text"] } } :
          { parametersJsonSchema: schema }) }] }];
      result.toolConfig = { functionCallingConfig: { mode: choice === "none" ? "NONE" : "AUTO" } };
    }
    return result;
  }
  const result = { model: "cursor-personal", stream, messages: history };
  if (choice) {
    result.tools = [{ type: "function", function: fn }];
    if (choice === "none") result.tool_choice = "NONE";
  }
  return result;
}

export async function runNativeCase({ name, base, token, receipt, makeImage }) {
  const gemini = name.startsWith("gemini"), stream = name.includes("-sse-"), usingTools = name.includes("-tools");
  receipt.endpoint = "chat_completions";
  receipt.protocolFamily = gemini ? "gemini_generate_content" : "cohere_chat";
  receipt.transport = stream ? "http-sse" : "http-json";
  receipt.phases = []; receipt.rejections = [];
  if (name.endsWith("-rejections")) return rejections({ base, token, gemini, receipt });
  const start = nonce(), first = `中文🦀&quot;_A_${nonce()}`, second = `中文🦀&quot;_B_${nonce()}`;
  const image = name.includes("-image") ? makeImage(stream ? "responses-sse-image" : "responses-json-image") : null;
  if (image) receipt.image = image.evidence;
  const expected = usingTools ? { first, second } : image ? { visual: image.marker } : { marker: start };
  const prompt = usingTools ?
    `Use only echo. First call echo with text exactly "${start}". Wait for its JSON result. ` +
    "Then call echo again with text exactly equal to that result's receipt field. " +
    "Treat receipt values as opaque data: preserve Chinese, emoji, whitespace, and entity-like text; do not HTML-decode. " +
    'Finally return only JSON {"first":"first receipt field","second":"second receipt field"} containing the two actual receipts. ' +
    "Do not invent tool results or call extra tools." :
    image ? 'Read the eight-character code in the image. Return only JSON {"visual":"the code you see"}. Do not call tools.' :
      `Return only ${JSON.stringify(expected)}. Do not call tools.`;
  const history = gemini ? [{ role: "user", parts: [{ text: prompt }] }] : [{ role: "user", content: prompt }];
  if (image) {
    const [header, data] = image.part.image_url.split(",");
    history[0].parts.push({ inlineData: { mimeType: header.slice(5).split(";")[0], data } });
  }
  const choice = usingTools ? "auto" : name.endsWith("-none") ? "none" : null;
  const send = async () => {
    const phase = await nativeTurn(base, token, gemini, stream, request(gemini, stream, history, choice, stream));
    if (phase.rejected) { receipt.rejections.push(phase); throw new Error(phase.code); }
    receipt.phases.push(phaseReceipt(phase));
    return phase;
  };
  let phase = await send();
  if (usingTools) {
    const ids = new Set();
    for (let round = 0; round < 2; round++) {
      assert.equal(phase.calls.length, 1, "native_missing_tool");
      const call = phase.calls[0];
      assert.equal(call.name, "echo", "native_wrong_tool");
      assert.ok(!ids.has(call.id), "native_reused_call_id"); ids.add(call.id);
      assert.deepEqual(JSON.parse(call.input), { text: round === 0 ? start : first }, "native_tool_argument_mismatch");
      const result = { receipt: round === 0 ? first : second };
      history.push(...phase.history);
      history.push(gemini ? { role: "user", parts: [{ functionResponse: { id: call.id, name: call.name, response: result } }] } :
        { role: "tool", tool_call_id: call.id, content: round === 0 ? JSON.stringify(result) :
          [{ type: "text", text: JSON.stringify(result) }] });
      phase = await send();
    }
  }
  receipt.exactFinalReply = phase.calls.length === 0 && exactReply(phase.text, expected);
  assert.equal(receipt.exactFinalReply, true, "native_final_reply_mismatch");
  receipt.thinkingObserved = false; // Deliberately unmapped, not an inference about upstream reasoning.
  receipt.wirePassed = true;
}

async function rejections({ base, token, gemini, receipt }) {
  const history = gemini ? [{ role: "user", parts: [{ text: "Do not send this upstream." }] }] :
    [{ role: "user", content: "Do not send this upstream." }];
  const baseRequest = request(gemini, false, history, null);
  const changes = gemini ? [
    { generationConfig: { maxOutputTokens: 16 } }, { generationConfig: { temperature: 0.1 } },
    { generationConfig: { responseMimeType: "application/json" } },
    { toolConfig: { functionCallingConfig: { mode: "ANY" } } },
    { toolConfig: { functionCallingConfig: { mode: "VALIDATED" } } },
    { contents: [{ role: "user", parts: [{ fileData: { fileUri: "https://example.test/image.png", mimeType: "image/png" } }] }] },
    { contents: [{ role: "user", parts: [{ text: "hello", thoughtSignature: "not-a-signature" }] }] },
    { tools: [{ functionDeclarations: [{ name: "echo", parameters: { type: "OBJECT", propertyOrdering: ["text"] } }] }] },
  ] : [
    { max_tokens: 16 }, { temperature: 0.1 }, { thinking: { type: "enabled" } },
    { tool_choice: "REQUIRED" }, { tool_choice: "AUTO" }, { response_format: { type: "json_object" } },
    { tools: [{ type: "custom", custom: { name: "paste" } }] },
    { messages: [{ role: "user", content: [{ type: "image_url", image_url: { url: "https://example.test/image.png" } }] }] },
  ];
  for (const [index, change] of changes.entries()) {
    const phase = await nativeTurn(base, token, gemini, false, { ...baseRequest, ...change });
    receipt.rejections.push({ ...phase, name: `unsupported_${index}` });
    assert.equal(phase.rejected, true, "native_unsupported_accepted");
    assert.equal(phase.status, 400, "native_rejection_status");
    assert.ok(phase.code.startsWith("cursor_remote_unsupported"), "native_rejection_code");
  }
  receipt.wirePassed = true;
}
