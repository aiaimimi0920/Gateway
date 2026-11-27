// Account-selected selector admission. Display metadata never determines routing or Free eligibility.
import { wireFields } from "./account_wire.mjs";
import { ProbeError } from "./transport.mjs";

export function modelId(model) {
  const value = typeof model === "string" ? model : model?.modelId;
  return typeof value === "string" && /^[A-Za-z0-9][A-Za-z0-9._:/+-]{0,159}$/.test(value)
    ? value : null;
}

const names = ["modelId", "thinkingDetails", "displayModelId", "displayName", "displayNameShort",
  "aliases", "maxMode", "apiKeyCredentials", "azureCredentials", "bedrockCredentials"];
const credentials = ["apiKeyCredentials", "azureCredentials", "bedrockCredentials"];

function details(value) {
  const fields = wireFields(value, names);
  const id = modelId(fields);
  if (!id || ["displayModelId", "displayName", "displayNameShort"]
    .some((key) => Object.hasOwn(fields, key) && typeof fields[key] !== "string") ||
    (Object.hasOwn(fields, "aliases") && (!Array.isArray(fields.aliases) ||
      fields.aliases.some((alias) => typeof alias !== "string"))) ||
    (Object.hasOwn(fields, "maxMode") && typeof fields.maxMode !== "boolean")) {
    throw new ProbeError("invalid_model_details");
  }
  // Credentials are neither copied nor logged. ThinkingDetails presence cannot be
  // losslessly expressed by RequestedModel; even an empty message is a variant.
  return { id, credentialVariant: credentials.some((key) => Object.hasOwn(fields, key)),
    thinkingVariant: Object.hasOwn(fields, "thinkingDetails"), maxMode: fields.maxMode === true };
}

export function catalogSummary(available, selected) {
  const catalog = wireFields(available, ["models"]);
  const selection = wireFields(selected, ["model"]);
  const entries = Object.hasOwn(catalog, "models") ? catalog.models : [];
  if (!Array.isArray(entries)) throw new ProbeError("invalid_model_catalog");
  const items = entries.map(details);
  const chosen = Object.hasOwn(selection, "model") ? details(selection.model) : null;
  const matches = chosen ? items.filter((item) => item.id === chosen.id) : [];
  const candidates = chosen ? [chosen, ...matches] : [];
  const risks = [];
  if (!chosen || matches.length === 0) risks.push("default_not_listed");
  if (matches.length > 1) risks.push("ambiguous_model_id");
  if (candidates.some((item) => item.credentialVariant)) risks.push("byok_credentials");
  if (candidates.some((item) => item.thinkingVariant)) risks.push("unsupported_thinking_variant");
  if (candidates.some((item) => item.maxMode)) risks.push("max_mode");
  return {
    entryCount: items.length, uniqueModelCount: new Set(items.map((item) => item.id)).size,
    defaultModel: chosen?.id ?? null, defaultListed: matches.length > 0,
    selectorSafe: risks.length === 0, selectorRisks: risks,
  };
}

export function requireAccountModel(catalog) {
  if (!modelId(catalog.defaultModel) || catalog.defaultListed !== true || catalog.selectorSafe !== true) {
    throw new ProbeError("account_default_model_not_verified");
  }
  // Opaque auto/default IDs are not rewritten to another model or reported as actualModel.
  return catalog.defaultModel;
}
