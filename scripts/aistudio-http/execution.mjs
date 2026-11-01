/** One selected account and one generation POST. Never use browser or official-API fallbacks. */
import { randomUUID } from "node:crypto";
import { ORIGIN, RPC, USER_AGENT, validateBundle, cookieHeader, requestHeaders } from "./auth.mjs";
import { buildWire, decodeResponse, parseWire } from "./wire.mjs";
import { prepareProof } from "./bootstrap.mjs";
import { createTransport } from "./transport.mjs";
import { helperPath, nativeProof } from "./native.mjs";
import { RESPONSE_LIMIT } from "./parts.mjs";
import { audioFormat, formatAudio } from "./audio.mjs";

export async function runHttpRequest(input, dependencies = {}) {
  const bundle = validateBundle(input.cookies), request = buildWire(input.body, input.model);
  const encoder = audioFormat(input.outputFormat);
  const executable = dependencies.makeProof ? undefined : helperPath();
  const send = dependencies.send ?? createTransport(input.timeoutMs);
  const makeProof = dependencies.makeProof ?? (value => nativeProof(value, executable));
  const page = await send("bootstrap", `${ORIGIN}/prompts/new_chat`, { headers: { "User-Agent": USER_AGENT,
    Cookie: cookieHeader(bundle, `${ORIGIN}/prompts/new_chat`), Accept: "text/html" } }, 8 * 1024 * 1024);
  const key = /"WIu0Nc":"([^"\\]+)"/.exec(page.text)?.[1];
  if (!key) throw new Error("aistudio_http_bootstrap_key_missing");
  const visit = `v1_${Buffer.from(randomUUID()).toString("base64")}`;
  const headers = requestHeaders(bundle, key, visit);
  const catalog = await send("models", `${RPC}ListModels`, { method: "POST", headers, body: "[]" });
  const models = parseWire(catalog.text)?.[0];
  if (!Array.isArray(models) || !models.some(row => row?.[0] === request.wire[0])) throw new Error("aistudio_http_model_unavailable");
  await send("preferences", `${RPC}GetUserPreferences`, { method: "POST", headers, body: "[]" });
  // Inline bytes work without Drive OAuth consent. Never upload persistent user files as a fallback.
  const prepared = await prepareProof(send, bundle, page, key, visit, request.binding, input.model);
  request.wire[4] = await makeProof(prepared.input);
  if (typeof request.wire[4] !== "string" || !request.wire[4].startsWith("!")) throw new Error("aistudio_http_waa_failed");
  const result = await send("generate", `${RPC}GenerateContent`, { method: "POST",
    headers: prepared.headers, body: JSON.stringify(request.wire) }, RESPONSE_LIMIT);
  return await formatAudio(decodeResponse(result.text), input.outputFormat, encoder);
}
