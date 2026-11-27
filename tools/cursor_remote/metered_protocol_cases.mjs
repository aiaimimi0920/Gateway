// Opt-in strict metered text/image subset. No tools or generation-limit simulation.
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { meteredTurn } from "./metered_protocol_client.mjs";
import { exactReply, phaseReceipt } from "./protocol_evidence.mjs";

export const meteredCases = ["bedrock-json-text", "bedrock-stream-text", "bedrock-json-image", "bedrock-stream-image",
  "dashscope-json-text", "dashscope-stream-text", "dashscope-json-message",
  "dashscope-multimodal-json-text", "dashscope-multimodal-stream-text",
  "dashscope-multimodal-json-image", "dashscope-multimodal-stream-image",
  "bedrock-controls-rejections", "dashscope-controls-rejections", "dashscope-multimodal-controls-rejections"];

export async function runMeteredCase({ name, base, token, receipt, makeImage }) {
  const kind = name.startsWith("bedrock") ? "bedrock" :
    name.startsWith("dashscope-multimodal") ? "dashscope-multimodal" : "dashscope";
  const stream = name.includes("-stream-");
  receipt.endpoint = "chat_completions";
  receipt.protocolFamily = kind === "bedrock" ? "bedrock_converse" :
    kind === "dashscope" ? "dashscope_text" : "dashscope_multimodal";
  receipt.transport = stream ? kind === "bedrock" ? "aws-eventstream" : "http-sse" : "http-json";
  receipt.phases = []; receipt.rejections = [];
  const image = name.endsWith("-image") ? makeImage(stream ? "responses-sse-image" : "responses-json-image") : null;
  const expected = image ? { visual: image.marker } : { marker: randomBytes(12).toString("hex") };
  const prompt = image ? 'Read the eight-character image code. Return only JSON {"visual":"the code you see"}.' :
    `Return only ${JSON.stringify(expected)}. Do not call tools.`;
  const parts = [{ text: prompt }];
  if (image) {
    receipt.image = image.evidence;
    const [header, data] = image.part.image_url.split(",");
    parts.push(kind === "bedrock" ? { image: { format: header.includes("jpeg") ? "jpeg" : "png", source: { bytes: data } } } :
      { image: image.part.image_url });
  }
  const body = kind === "bedrock" ? { system: [{ text: "Follow the user's format precisely." }],
    messages: [{ role: "user", content: parts }] } :
    { model: "cursor-personal", input: { messages: [{ role: "user", content: kind === "dashscope" ? prompt : parts }] } };
  if (name.endsWith("-message")) body.parameters = { result_format: "message" };
  if (stream && kind !== "bedrock") body.parameters = { incremental_output: true };
  if (name.endsWith("-rejections")) {
    const variants = kind === "bedrock" ?
      [{ inferenceConfig: { maxTokens: 16 } }, { additionalModelRequestFields: { temperature: 0 } }] :
      [{ parameters: { max_tokens: 16 } }, { parameters: { tools: [] } }, { parameters: { temperature: 0 } }];
    for (const change of variants) {
      const result = await meteredTurn(base, token, kind, false, { ...body, ...change });
      assert.equal(result.rejected, true); assert.equal(result.status, 400);
      assert.ok(result.code.startsWith("cursor_remote_unsupported"));
      receipt.rejections.push(result);
    }
    if (kind === "bedrock") {
      const result = await meteredTurn(base, token, kind, false, body, { "x-amzn-bedrock-guardrailidentifier": "must-not-ignore" });
      assert.equal(result.rejected, true); assert.equal(result.status, 400);
      assert.equal(result.code, "cursor_remote_unsupported_request");
      receipt.rejections.push(result);
    }
  } else {
    const phase = await meteredTurn(base, token, kind, stream, body);
    if (phase.rejected) { receipt.remoteFailure = phase; throw new Error(phase.code); }
    receipt.phases.push({ ...phaseReceipt(phase), usage: phase.usage });
    assert.equal(exactReply(phase.text, expected), true, "metered_final_reply_mismatch");
    receipt.exactFinalReply = true;
  }
  receipt.wirePassed = true;
}
