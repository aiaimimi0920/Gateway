import { MODEL_COMPANY_RULES } from "./modelDisplayCatalog";
import { isRecord } from "./routeDocument";
import type { ConsoleTelemetrySnapshot } from "./telemetry";

export type ProviderModelMappingEntry = { model: string; upstreamModel: string };

export function readProviderModelMappings(provider: Record<string, unknown>): ProviderModelMappingEntry[] {
  const mappings = new Map<string, string[]>();
  if (isRecord(provider.model_map)) {
    for (const [source, target] of Object.entries(provider.model_map)) {
      if (typeof target === "string") mappings.set(source, [target]);
    }
  }
  if (isRecord(provider.model_map_targets)) {
    for (const [source, targets] of Object.entries(provider.model_map_targets)) {
      if (Array.isArray(targets)) mappings.set(source, targets.filter((target): target is string => typeof target === "string"));
    }
  }
  return [...mappings].flatMap(([model, targets]) => targets.map((upstreamModel) => ({ model, upstreamModel })));
}

/** Keep the legacy single-target format; only multiple targets use the additive field. */
export function writeProviderModelMappings(provider: Record<string, unknown>, entries: readonly ProviderModelMappingEntry[]) {
  const mappings = new Map<string, Set<string>>();
  for (const { model, upstreamModel } of entries) {
    const targets = mappings.get(model) ?? new Set<string>();
    targets.add(upstreamModel);
    mappings.set(model, targets);
  }
  delete provider.model_map;
  delete provider.model_map_targets;
  const single = [...mappings].filter(([, targets]) => targets.size === 1).map(([model, targets]) => [model, [...targets][0]]);
  const multiple = [...mappings].filter(([, targets]) => targets.size > 1).map(([model, targets]) => [model, [...targets]]);
  if (single.length) provider.model_map = Object.fromEntries(single);
  if (multiple.length) provider.model_map_targets = Object.fromEntries(multiple);
}

/** Local observed traffic is popularity evidence; catalogue order is only a fallback. */
export function rankedMappingModels(models: readonly string[], telemetry: ConsoleTelemetrySnapshot) {
  const catalogue = MODEL_COMPANY_RULES.flatMap((company) => company.models);
  const rank = new Map(catalogue.map((model, index) => [model, index]));
  const counts = new Map<string, number>();
  if (telemetry.retainedModelTotals) {
    for (const row of telemetry.retainedModelTotals) counts.set(row.model, (counts.get(row.model) ?? 0) + row.requestCount);
  } else {
    for (const provider of telemetry.providerAccounts.values()) {
      for (const row of provider.modelStats.values()) counts.set(row.model, (counts.get(row.model) ?? 0) + (row.requestCount ?? 0));
    }
  }
  return [...new Set([...models, ...catalogue, ...counts.keys()])].filter((model) => !model.includes("*"))
    .sort((a, b) => (counts.get(b) ?? 0) - (counts.get(a) ?? 0)
      || (rank.get(a) ?? Infinity) - (rank.get(b) ?? Infinity) || a.localeCompare(b));
}
