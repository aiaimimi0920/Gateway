// Compare the existing AgentService catalog with pinned official CLI AiService selection.
// Diagnostic only: metadata aliases never rewrite a Run model or establish Free eligibility.
import { catalogSummary } from "./catalog_wire.mjs";
import { readParameterModels } from "./parameter_models_wire.mjs";
import { connectJsonHeaders, endpoints, parseJson, ProbeError, request, settledRequests } from "./transport.mjs";

function aliasField(value, name) {
  const snake = name.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
  if (snake !== name && Object.hasOwn(value, snake) && Object.hasOwn(value, name)) {
    throw new ProbeError("invalid_selection_metadata");
  }
  const result = Object.hasOwn(value, name) ? value[name] : value[snake];
  if (result === null) throw new ProbeError("invalid_selection_metadata");
  return result;
}

function optionalFlag(value, name) {
  const flag = aliasField(value, name);
  if (flag !== undefined && typeof flag !== "boolean") throw new ProbeError("invalid_selection_metadata");
  return flag ?? null;
}

export function parameterSelectionSummary(value, selected) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ProbeError("invalid_selection_metadata");
  const entries = Object.hasOwn(value, "models") ? value.models : [];
  if (!Array.isArray(entries)) throw new ProbeError("invalid_selection_metadata");
  let direct = 0, aliases = 0, needsMapping = false, definitions = 0, variants = 0;
  let parameterValues = 0, maxMode = false, supportsAgent = null, requiresRetention = null;
  for (const entry of entries) {
    if (!entry || typeof entry !== "object" || Array.isArray(entry) || typeof entry.name !== "string") {
      throw new ProbeError("invalid_selection_metadata");
    }
    const slugs = aliasField(entry, "legacySlugs") ?? [];
    if (!Array.isArray(slugs) || slugs.some((slug) => typeof slug !== "string")) {
      throw new ProbeError("invalid_selection_metadata");
    }
    const exact = entry.name === selected, alias = slugs.includes(selected);
    if (exact) direct++;
    if (alias) aliases++;
    if (alias && !exact) needsMapping = true;
    if (exact || alias) {
      const params = aliasField(entry, "parameterDefinitions") ?? [];
      const modes = Object.hasOwn(entry, "variants") ? entry.variants : [];
      if (!Array.isArray(params) || !Array.isArray(modes)) throw new ProbeError("invalid_selection_metadata");
      definitions += params.length;
      variants += modes.length;
      for (const mode of modes) {
        if (!mode || typeof mode !== "object" || Array.isArray(mode)) throw new ProbeError("invalid_selection_metadata");
        const values = aliasField(mode, "parameterValues") ?? [];
        if (!Array.isArray(values) || values.some((item) => !item || typeof item !== "object" ||
          typeof item.id !== "string" || typeof item.value !== "string")) throw new ProbeError("invalid_selection_metadata");
        parameterValues += values.length;
        const variantMaxMode = optionalFlag(mode, "isMaxMode");
        maxMode ||= variantMaxMode === true;
      }
      if (exact) {
        supportsAgent = optionalFlag(entry, "supportsAgent");
        requiresRetention = optionalFlag(entry, "requiresDataRetention");
      }
    }
  }
  return { parameterizedCount: entries.length, defaultNameMatches: direct, defaultAliasMatches: aliases,
    defaultNeedsMapping: needsMapping, defaultParameterDefinitions: definitions, defaultVariants: variants,
    defaultVariantParameterValues: parameterValues, defaultVariantMaxMode: maxMode,
    defaultSupportsAgent: supportsAgent, defaultRequiresDataRetention: requiresRetention };
}

export async function inspectModelSelection(token, agentCatalog, options, call = request) {
  const headers = { ...connectJsonHeaders, authorization: `Bearer ${token}` };
  const read = async (operation, url, body = "{}") => {
    try { return parseJson(await call(url, { ...options, headers, body })); }
    catch (error) {
      if (error instanceof ProbeError) error.operation = operation;
      throw error;
    }
  };
  const [available, selected, parameterized] = await settledRequests([
    read("cli_models", endpoints.cliModels), read("cli_default_model", endpoints.cliDefaultModel),
    readParameterModels(token, options, call),
  ]);
  const cli = catalogSummary(available, selected);
  return {
    cliEntryCount: cli.entryCount, cliSelectorSafe: cli.selectorSafe, cliDefaultListed: cli.defaultListed,
    sameDefaultSelector: agentCatalog.defaultModel === cli.defaultModel,
    ...parameterSelectionSummary(parameterized, cli.defaultModel),
    selectionMappingImplemented: false,
  };
}
