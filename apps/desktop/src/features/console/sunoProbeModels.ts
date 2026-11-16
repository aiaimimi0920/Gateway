import { isRecord } from "./routeDocument";
import { validModelId } from "./providerModelId";
import { readProviderModelMappings } from "./providerModelMappingDocument";

export const isSunoMediaProvider = (provider: unknown) => isRecord(provider) && provider.preset === "suno";
const implemented = new Set(["chirp-v3-5", "chirp-v3-0", "chirp-auk-turbo", "chirp-auk"]);
const strings = (value: unknown): string[] => Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];

/** Existing mapper capabilities only, not a current Suno catalogue or entitlement claim. */
export function sunoProbeModels(provider: Record<string, unknown>, credential: Record<string, unknown>): string[] {
  const allowed = new Set(strings(credential.supported_models)), mappings = readProviderModelMappings(provider);
  const byModel = new Map<string, string[]>();
  for (const row of mappings) byModel.set(row.model, [...(byModel.get(row.model) ?? []), row.upstreamModel]);
  const candidates = new Set([...strings(provider.supported_models), ...allowed,
    ...(typeof provider.default_model === "string" ? [provider.default_model] : []), ...mappings.map(row => row.model)]);
  return [...candidates].filter(model => {
    if (!validModelId(model)) return false;
    const mapped = isRecord(provider.model_map_targets) && Object.hasOwn(provider.model_map_targets, model)
      || isRecord(provider.model_map) && Object.hasOwn(provider.model_map, model);
    const targets = (mapped ? byModel.get(model) ?? [] : [model]).filter(target => !allowed.size || allowed.has(model) || allowed.has(target));
    return targets.length > 0 && targets.every(target => implemented.has(target));
  });
}
