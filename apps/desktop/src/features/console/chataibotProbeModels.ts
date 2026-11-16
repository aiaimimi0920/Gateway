import { isRecord } from "./routeDocument";
import { validModelId } from "./providerModelId";
import { readProviderModelMappings } from "./providerModelMappingDocument";

export const isChataibotMediaProvider = (provider: unknown) => isRecord(provider) && provider.preset === "chataibot";
const generation = new Set(["gpt-image-1.5", "gpt-image-1.5-high", "ideogram", "google-nano-banana-pro", "google-nano-banana",
  "google-nano-banana-2", "midjourney-7", "qwen-lora", "bytedance-seedream"]);
const editing = new Set(["google-nano-banana", "google-nano-banana-2", "qwen-lora"]);
const strings = (value: unknown): string[] => Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];

/** Implementation capabilities, not a live catalogue or entitlement claim. Never inject a default. */
export function chataibotProbeModels(provider: Record<string, unknown>, credential: Record<string, unknown>, edit: boolean): string[] {
  const mappings = readProviderModelMappings(provider);
  const allowed = new Set(strings(credential.supported_models));
  const byModel = new Map<string, string[]>();
  for (const row of mappings) byModel.set(row.model, [...(byModel.get(row.model) ?? []), row.upstreamModel]);
  const candidates = new Set([...strings(provider.supported_models), ...allowed,
    ...(typeof provider.default_model === "string" ? [provider.default_model] : []), ...mappings.map(row => row.model)]);
  return [...candidates].filter(model => {
    if (!validModelId(model)) return false;
    // The backend randomly chooses an eligible multi-target mapping. All eligible targets must
    // support this operation; an unsupported target cannot be hidden by a supported neighbour.
    const hasMapping = isRecord(provider.model_map_targets) && Object.hasOwn(provider.model_map_targets, model)
      || isRecord(provider.model_map) && Object.hasOwn(provider.model_map, model);
    const targets = (hasMapping ? byModel.get(model) ?? [] : [model]).filter(target => !allowed.size || allowed.has(model) || allowed.has(target));
    return targets.length > 0 && targets.every(target => (edit ? editing : generation).has(target));
  });
}
