// Official /models metadata is not account entitlement or successful generation evidence.
import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { applyModelSelection, modelSelectionSchema } from "./credentialModelSelection";

export const catalogSources = { gemini: "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000", anthropic: "https://api.anthropic.com/v1/models?limit=1000", openai: "https://api.openai.com/v1/models", nvidia: "https://integrate.api.nvidia.com/v1/models",
  openrouter: "https://openrouter.ai/api/v1/models", poe: "https://api.poe.com/v1/models",
  groq: "https://api.groq.com/openai/v1/models", cohere: "https://api.cohere.com/v1/models?endpoint=chat&page_size=1000" } as const;
export type CatalogProviderKind = keyof typeof catalogSources;
export const officialCatalogSchema = z.object({
  sourceUrl: z.enum([catalogSources.gemini, catalogSources.anthropic, catalogSources.openai, catalogSources.nvidia, catalogSources.openrouter, catalogSources.poe, catalogSources.groq, catalogSources.cohere]),
  checkedAt: z.string().datetime({ offset: true }),
  models: z.array(z.string().min(1).max(256).refine((id) => id.trim() === id
    && new TextEncoder().encode(id).length <= 256 && !/[\p{Cc}*?]/u.test(id))).min(1).max(2048)
    .refine((models) => new Set(models).size === models.length),
});
export const officialCatalogResponseSchema = z.object({ revision: z.string(), catalog: officialCatalogSchema });
export type OfficialModelCatalog = z.infer<typeof officialCatalogSchema>;
export type CatalogInput = { credentialId: string } | { providerId: string; apiKey: string };
export type DiscoverProviderModels = (input: CatalogInput, signal?: AbortSignal) => Promise<OfficialModelCatalog>;
export function officialCatalogBase(kind: CatalogProviderKind, base: string): boolean {
  return (kind === "nvidia" ? ["https://integrate.api.nvidia.com", "https://integrate.api.nvidia.com/v1"]
    : kind === "openai" ? ["https://api.openai.com/v1"]
    : kind === "anthropic" ? ["https://api.anthropic.com"]
    : kind === "gemini" ? ["https://generativelanguage.googleapis.com/v1beta"]
    : kind === "poe" ? ["https://api.poe.com", "https://api.poe.com/v1"]
      : kind === "groq" ? ["https://api.groq.com/openai/v1"]
        : kind === "cohere" ? ["https://api.cohere.com"]
      : ["https://openrouter.ai", "https://openrouter.ai/api", "https://openrouter.ai/api/v1"]).includes(base.replace(/\/+$/, ""));
}
export function catalogProviderKind(provider: Record<string, unknown>): CatalogProviderKind | undefined {
  if (provider.preset === "gemini-api") return "gemini";
  if (provider.preset === "anthropic") return "anthropic";
  if (provider.preset === "openai" && typeof provider.base_url === "string"
    && officialCatalogBase("openai", provider.base_url)) return "openai";
  if (provider.preset === "nvidia-openai") return "nvidia";
  if (provider.preset === "poe-openai") return "poe";
  if (provider.preset === "groq-openai") return "groq";
  if (provider.preset === "cohere-chat") return "cohere";
  if (provider.preset === "openrouter-openai" || (provider.preset === "openai"
    && typeof provider.base_url === "string" && officialCatalogBase("openrouter", provider.base_url))) return "openrouter";
}
const strings = (value: unknown): string[] => Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
function reconcileModels(existing: unknown, desired: string[]): string[] {
  const wanted = new Set(desired);
  return [...new Set([...strings(existing).filter((model) => wanted.has(model)), ...desired])];
}
export function catalogModels(credential: unknown): string[] {
  if (!isRecord(credential)) return [];
  const catalog = officialCatalogSchema.safeParse(credential.model_catalog);
  return catalog.success ? catalog.data.models : [];
}

/** Replace catalog-owned entries only; keep manual models, other accounts and route policy. */
export function applyOfficialCatalog(document: ConsoleRouteDocument, providerId: string, credentialId: string,
  input: OfficialModelCatalog): ConsoleRouteDocument {
  const catalog = officialCatalogSchema.parse(input);
  const next = structuredClone(document);
  const provider = next.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  const kind = isRecord(provider) ? catalogProviderKind(provider) : undefined;
  if (!isRecord(provider) || !kind || catalog.sourceUrl !== catalogSources[kind]) throw new Error("模型目录与服务商不匹配。");
  const credentials = Array.isArray(provider.credentials) ? provider.credentials.filter(isRecord) : [];
  const credential = credentials.find((entry) => entry.id === credentialId);
  if (!credential) throw new Error("模型目录账号已不存在。");
  const base = credential.base_url ?? provider.base_url;
  if (typeof base === "string" && !officialCatalogBase(kind, base)) throw new Error("模型目录仅支持官方 API 地址。");
  const selection = modelSelectionSchema.safeParse(credential.model_selection).data;
  if (selection) {
    delete credential.discovery_job;
    credential.model_catalog = catalog;
    return applyModelSelection(next, providerId, credentialId, { ...selection, fetched: catalog.models });
  }
  const previousManaged = new Set(credentials.flatMap(catalogModels));
  const previousAccount = new Set(catalogModels(credential));
  const manual = strings(credential.supported_models).filter((model) => !previousAccount.has(model));
  // Official directory refresh supersedes an old generative discovery intent.
  delete credential.discovery_job;
  credential.model_catalog = catalog;
  credential.supported_models = reconcileModels(credential.supported_models, [...manual, ...catalog.models]);
  const providerManual = strings(provider.supported_models).filter((model) => !previousManaged.has(model));
  provider.supported_models = reconcileModels(provider.supported_models,
    [...providerManual, ...credentials.flatMap((entry) => strings(entry.supported_models))]);
  const remaining = new Set(strings(provider.supported_models));
  next.model_routes = next.model_routes.flatMap((entry) => {
    if (!isRecord(entry) || !previousManaged.has(String(entry.pattern)) || remaining.has(String(entry.pattern))
      || !Array.isArray(entry.provider_ids) || entry.enabled === false) return [entry];
    const ids = entry.provider_ids.filter((id) => id !== providerId);
    entry.provider_ids = ids;
    return ids.length ? [entry] : [];
  });
  for (const model of catalog.models) {
    const route = next.model_routes.find((entry) => isRecord(entry) && entry.pattern === model);
    if (!route) next.model_routes.push({ pattern: model, provider_ids: [providerId], priority: 10, enabled: true });
    else if (isRecord(route) && Array.isArray(route.provider_ids) && !route.provider_ids.includes(providerId)) route.provider_ids.push(providerId);
  }
  return next;
}
