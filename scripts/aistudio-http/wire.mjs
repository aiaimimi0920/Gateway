/** Playground JSON+protobuf codec. EOF without a terminal candidate is never success. */
import { encodeParts, PIPE_INPUT_LIMIT } from "./parts.mjs";
import { generationConfig } from "./generation.mjs";
import { decodeFrames } from "./response.mjs";
export function parseWire(text) {
  text = text.replace(/^\)\]\}'\r?\n/, "");
  try { return JSON.parse(text); } catch { /* Google may omit null array fields. */ }
  const output = []; let quoted = false, escaped = false, previous = "", start = 0;
  for (let index = 0; index < text.length; index++) {
    const c = text[index];
    if (quoted) {
      if (escaped) escaped = false;
      else if (c === "\\") escaped = true;
      else if (c === '"') quoted = false;
      continue;
    }
    if (c === '"') quoted = true;
    if (c === "," && (previous === "[" || previous === ",") || c === "]" && previous === ",") {
      output.push(text.slice(start, index), "null"); start = index;
    }
    if (!/\s/.test(c)) previous = c;
  }
  output.push(text.slice(start));
  return JSON.parse(output.join(""));
}

export function requestBinding(wire) {
  // Google binds every content part in order, including inline bytes or uploaded file IDs.
  const binding = wire[1].flatMap(content => content[0].map(part => part[1] || part[2]?.[1] || part[5]?.[0] || "")).join(" ");
  if (!binding.trim() || Buffer.byteLength(binding) > PIPE_INPUT_LIMIT) throw new Error("aistudio_http_prompt_limit");
  return binding;
}

export function buildWire(body, model) {
  model = model.replace(/^models\//, "");
  if (!/^[A-Za-z0-9._-]{1,128}$/.test(model) || !Array.isArray(body?.contents)
    || body.contents.length === 0 || body.contents.length > 128) throw new Error("aistudio_http_request_invalid");
  const budget = { bytes: 0, parts: 0 };
  const contents = body.contents.map(content => {
    const role = content.role ?? "user";
    if (!["user", "model"].includes(role)) throw new Error("aistudio_http_role_invalid");
    return [encodeParts(content.parts, budget), role];
  });
  const text = contents.flatMap(content => content[0].map(part => part[1] ?? "")).join(" ");
  if (Buffer.byteLength(text) > 8192) throw new Error("aistudio_http_prompt_limit");
  const config = generationConfig(body.generationConfig);
  if (body.tools?.length) throw new Error("aistudio_http_option_unsupported");
  const wire = Array(11).fill(null);
  wire[0] = `models/${model}`; wire[1] = contents; wire[3] = config; wire[10] = 1;
  const system = body.systemInstruction ?? body.system_instruction;
  if (system) wire[5] = [encodeParts(system.parts, budget, true), "user"];
  return { wire, binding: requestBinding(wire) };
}

export function decodeResponse(text) {
  return decodeFrames(parseWire(text));
}
