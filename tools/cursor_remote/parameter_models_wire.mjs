// Minimal projection of official CLI 2026.11.22 AvailableModels protobuf descriptors.
// Unknown display/price metadata is never serialized. This is not a billing authority.
import { fields, field } from "./protobuf_wire.mjs";
import { endpoints, ProbeError, request } from "./transport.mjs";

const invalid = () => { throw new ProbeError("invalid_selection_metadata"); };
function bytes(item) {
  if (item.wire !== 2) invalid();
  return item.value;
}
function text(item) {
  const raw = bytes(item);
  if (raw.length > 4096) invalid();
  try { return new TextDecoder("utf-8", { fatal: true }).decode(raw); }
  catch { invalid(); }
}
function flag(item) {
  if (item.wire !== 0 || (item.value !== 0n && item.value !== 1n)) invalid();
  return item.value === 1n;
}
function singleton(seen, item) {
  if (seen.has(item.tag)) invalid();
  seen.add(item.tag);
}
function parameter(raw) {
  const result = { id: "", value: "" }, seen = new Set();
  for (const item of fields(raw)) {
    if (![1, 2].includes(item.tag)) continue;
    singleton(seen, item);
    result[item.tag === 1 ? "id" : "value"] = text(item);
  }
  return result;
}
function variant(raw) {
  const result = { parameterValues: [], isMaxMode: false }, seen = new Set();
  for (const item of fields(raw)) {
    if (item.tag === 1) result.parameterValues.push(parameter(bytes(item)));
    if (item.tag === 3) {
      singleton(seen, item);
      result.isMaxMode = flag(item);
    }
  }
  return result;
}
function model(raw) {
  const result = { name: "", legacySlugs: [], parameterDefinitions: [], variants: [] };
  const seen = new Set();
  for (const item of fields(raw)) {
    if ([1, 5, 46].includes(item.tag)) singleton(seen, item);
    if (item.tag === 1) result.name = text(item);
    if (item.tag === 5) result.supportsAgent = flag(item);
    if (item.tag === 46) result.requiresDataRetention = flag(item);
    if (item.tag === 36) result.legacySlugs.push(text(item));
    if (item.tag === 29) {
      // Only count definitions: nonempty definitions are disallowed by the selection gate.
      fields(bytes(item));
      result.parameterDefinitions.push({});
    }
    if (item.tag === 30) result.variants.push(variant(bytes(item)));
  }
  return result;
}

export function parameterModelsResponse(response) {
  if (response.status !== 200) throw new ProbeError("upstream_http_error", response.status);
  if (response.contentType !== "application/proto") throw new ProbeError("unexpected_protobuf_content_type");
  if (!Buffer.isBuffer(response.body) || response.body.length > 512 * 1024) invalid();
  const models = [];
  for (const item of fields(response.body)) {
    if (item.tag === 2) models.push(model(bytes(item)));
  }
  return { models };
}

export async function readParameterModels(token, options, call = request) {
  try {
    // Unary proto is bare, not Connect-framed. No JSON-repair or failed-read fallback.
    return parameterModelsResponse(await call(endpoints.parameterModels, {
      ...options, headers: { authorization: `Bearer ${token}`, accept: "application/proto",
        "content-type": "application/proto", "connect-protocol-version": "1" },
      body: Buffer.concat([field(5, 1), field(7, 1)]),
    }));
  } catch (error) {
    if (error instanceof ProbeError) error.operation = "parameter_models";
    throw error;
  }
}
