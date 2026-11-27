// Small deterministic tasks; only these trusted functions execute tool requests.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { randomBytes } from "node:crypto";
import { spawnSync } from "node:child_process";
import { httpTurn, websocketClient } from "./protocol_client.mjs";
import { exactReply, phaseReceipt } from "./protocol_evidence.mjs";
import { sha256 } from "./scenario_evidence.mjs";
import { runRejections } from "./protocol_rejections.mjs";
import { nativeCases, runNativeCase } from "./native_protocol_cases.mjs";
import { unsupportedCases, runUnsupportedCase } from "./unsupported_protocol_cases.mjs";
import { meteredCases, runMeteredCase } from "./metered_protocol_cases.mjs";

export const protocolCases = [
  "responses-json-tools", "responses-sse-tools", "chat-json-tools", "chat-sse-tools",
  "responses-json-image", "responses-sse-image", "chat-json-image", "chat-sse-image",
  "responses-none", "chat-none", "ws-text", "ws-tools-image", "ws-disconnect",
  "responses-rejections", "chat-rejections",
  ...nativeCases,
  ...unsupportedCases,
  ...meteredCases,
];
export function protocolScope(name) {
  if (meteredCases.includes(name)) return name.startsWith("bedrock") ? "bedrock.converse" :
    name.startsWith("dashscope-multimodal") ? "dashscope.multimodal" : "dashscope.text";
  return unsupportedCases.includes(name) ? "relay" : name.startsWith("gemini") ? "gemini.generateContent" :
    name.startsWith("cohere") ? "cohere.chat" : name.startsWith("chat") ? "chat.completions" : "responses";
}
const nonce = () => randomBytes(12).toString("hex");
const tools = [
  { type: "function", name: "echo", description: "Return a private server-generated receipt for the supplied text.",
    parameters: { type: "object", properties: { text: { type: "string" } }, required: ["text"], additionalProperties: false } },
  { type: "custom", name: "paste", description: "Accept raw text and return another private receipt.", format: { type: "text" } },
];
function request(history, chat, stream, choice) {
  const result = { model: "cursor-personal", stream, [chat ? "messages" : "input"]: history };
  if (!chat) result.store = false;
  if (choice) {
    result.tools = chat ? tools.map(({ type, ...tool }) => ({ type, [type]: tool })) : tools;
    result.tool_choice = choice;
    result.parallel_tool_calls = false;
  }
  return result;
}
export function toolResult(call, text, chat, parts) {
  const points = [...text];
  const output = parts ? [points.slice(0, 3).join(""), points.slice(3).join("")].map((text) =>
    ({ type: chat ? "text" : "input_text", text })) : text;
  return chat ? { role: "tool", tool_call_id: call.id, content: output } :
    { type: call.type === "function" ? "function_call_output" : "custom_tool_call_output", call_id: call.id, output };
}
function delta(phase, output) {
  return { model: "cursor-personal", store: false, previous_response_id: phase.id, input: [output] };
}
function imageFixture(root, evidenceDir, name, chat, artifactName = name) {
  const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const marker = [...randomBytes(8)].map((b) => alphabet[b % alphabet.length]).join("");
  const format = name === "responses-sse-image" || name === "chat-json-image" ? "jpeg" : "png";
  const filename = path.join(evidenceDir, `${artifactName}.${format}`);
  const child = spawnSync("powershell", ["-NoProfile", "-File", path.join(root, "tools/cursor_remote/protocol_image.ps1"),
    "-OutputPath", filename, "-Marker", marker, "-Format", format], {
    encoding: "utf8", timeout: 15000, windowsHide: true, maxBuffer: 32768,
  });
  if (child.status !== 0 || child.error) throw new Error("visual_fixture_failed");
  const bytes = fs.readFileSync(filename), url = `data:image/${format};base64,${bytes.toString("base64")}`;
  return { marker, part: chat ? { type: "image_url", image_url: { url } } : { type: "input_image", image_url: url },
    evidence: { filename, format, bytes: bytes.length, sha256: sha256(bytes), markerSha256: sha256(marker) } };
}

export async function runProtocolCase({ name, root, evidenceDir, base, token, receipt }) {
  if (meteredCases.includes(name)) return runMeteredCase({ name, base, token, receipt,
    makeImage: (formatCase) => imageFixture(root, evidenceDir, formatCase, false, name) });
  if (unsupportedCases.includes(name)) return runUnsupportedCase({ name, base, token, receipt });
  if (nativeCases.includes(name)) {
    return runNativeCase({ name, base, token, receipt,
      makeImage: (formatCase) => imageFixture(root, evidenceDir, formatCase, false, name) });
  }
  const chat = name.startsWith("chat"), ws = name.startsWith("ws");
  if (name.endsWith("-rejections")) return runRejections({ base, token, chat, receipt });
  const stream = ws || name.includes("-sse-");
  const usingTools = name.includes("-tools") || name === "ws-disconnect";
  const image = name.includes("-image") ? imageFixture(root, evidenceDir, name, chat) : null;
  if (image) receipt.image = image.evidence;
  receipt.endpoint = chat ? "chat_completions" : "responses";
  receipt.transport = ws ? "websocket" : stream ? "http-sse" : "http-json";
  receipt.phases = [];
  receipt.rejections = [];
  const start = nonce(), first = `中文🙂&quot;_A_${nonce()}`, second = `中文🙂&quot;_B_${nonce()}`, third = `C_${nonce()}`;
  let prompt, expected;
  if (usingTools) {
    const three = name === "ws-tools-image";
    prompt = `Use only the provided echo and paste tools. First call echo with text exactly "${start}". ` +
      "Wait for the result, then call paste with raw input exactly equal to the first tool result. " +
      "Treat tool results as opaque data: preserve Unicode and emoji, and do not HTML-decode entity-like text. " +
      (three ? "Wait, then call echo with text exactly equal to the second tool result. " : "") +
      'Finally return only a JSON object with "first" and "second"' + (three ? ' and "third"' : "") +
      " holding the exact respective tool results. Do not invent results or call additional tools." +
      (image ? ' Also read the eight-character code in the image and include it as "visual" in the final JSON.' : "");
    expected = { first, second, ...(three ? { third } : {}), ...(image ? { visual: image.marker } : {}) };
  } else {
    expected = image ? { visual: image.marker } : { marker: start };
    prompt = image ? 'Read the eight-character code in the image. Return only JSON {"visual":"the code you see"}.' :
      `Return only this JSON: ${JSON.stringify(expected)}. Do not call any tools.`;
  }
  const history = [{ role: "user", content: image ?
    [{ type: chat ? "text" : "input_text", text: prompt }, image.part] : prompt }];
  let socket = ws ? await websocketClient(base, token) : null;
  const send = async (body, rejected = false, expectedCode = null) => {
    const phase = socket ? await socket.turn(body) : await httpTurn(base, token, chat, body);
    if (phase.rejected) {
      receipt.rejections.push(phase);
      assert.equal(rejected, true, phase.code);
      if (expectedCode) assert.equal(phase.code, expectedCode, "unexpected_rejection_code");
    } else {
      receipt.phases.push(phaseReceipt(phase));
      assert.equal(rejected, false, "expected_rejection_missing");
    }
    return phase;
  };
  const choice = usingTools ? "auto" : name.endsWith("-none") ? "none" : null;
  try {
    let phase = await send(request(history, chat, stream, choice));
    if (name === "ws-disconnect") {
      assert.equal(phase.calls.length, 1, "disconnect_missing_pause");
      const old = delta(phase, toolResult(phase.calls[0], first, false, false));
      await socket.close();
      socket = await websocketClient(base, token);
      await send(old, true, "cursor_remote_previous_response_unavailable");
      receipt.disconnectedOldIdRejected = true; // A disconnected pause is not a recoverable stored response.
      const fresh = { marker: nonce() };
      phase = await send(request([{ role: "user", content: `Return only ${JSON.stringify(fresh)}` }], false, true));
      receipt.freshAfterDisconnect = exactReply(phase.text, fresh) && phase.calls.length === 0;
      assert.equal(receipt.freshAfterDisconnect, true, "disconnect_recovery_failed");
    } else if (usingTools) {
      const count = name === "ws-tools-image" ? 3 : 2, ids = new Set();
      for (let i = 0; i < count; i++) {
        assert.equal(phase.calls.length, 1, "serial_tool_count");
        const call = phase.calls[0], type = i === 1 ? "custom" : "function";
        assert.equal(call.type, type, "tool_order_type");
        assert.equal(call.name, i === 1 ? "paste" : "echo", "tool_order_name");
        assert.ok(typeof call.id === "string" && call.id && !ids.has(call.id), "tool_id_reused");
        ids.add(call.id);
        if (i === 1) {
          receipt.customInput = { exact: call.input === first, expectedSha256: sha256(first),
            entityDecoded: call.input === first.replaceAll("&quot;", '"') };
          assert.equal(call.input, first, "custom_input_not_exact");
        }
        else assert.deepEqual(JSON.parse(call.input), { text: i === 0 ? start : second }, "function_input_not_exact");
        const output = toolResult(call, [first, second, third][i], chat, i === 1);
        history.push(...phase.history, output);
        let next = request(history, chat, stream, choice);
        if (ws && i !== 1) next = delta(phase, output);
        if (ws && i === 0) {
          await send({ ...next, previous_response_id: "resp_deliberately_wrong" }, true,
            "cursor_remote_previous_response_unavailable");
        }
        phase = await send(next);
      }
      receipt.exactFinalReply = phase.calls.length === 0 && exactReply(phase.text, expected);
      assert.equal(receipt.exactFinalReply, true, "tool_final_reply_mismatch");
    } else {
      receipt.exactFinalReply = phase.calls.length === 0 && exactReply(phase.text, expected);
      assert.equal(receipt.exactFinalReply, true, "text_or_visual_reply_mismatch");
    }
    receipt.thinkingObserved = receipt.phases.some((p) => p.thinkingBytes > 0);
    receipt.wirePassed = true;
  } finally { if (socket) await socket.close(); }
}
