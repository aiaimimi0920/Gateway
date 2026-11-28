// Official /models metadata is not account entitlement or successful generation evidence.
import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { applyModelSelection, modelSelectionSchema } from "./credentialModelSelection";
import { isQwenProvider, qwenCatalogEndpoint } from "./qwenEndpoint";
import { cloudflareEndpoint } from "./cloudflareWorkersAi";
import { zenmux } from "./zenmuxCatalog";

export const catalogSources = { gemini: "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000", anthropic: "https://api.anthropic.com/v1/models?limit=1000", openai: "https://api.openai.com/v1/models", nvidia: "https://integrate.api.nvidia.com/v1/models",
  openrouter: "https://openrouter.ai/api/v1/models", poe: "https://api.poe.com/v1/models", mistral: "https://api.mistral.ai/v1/models",
  zenmux: zenmux.modelsCatalogUrl!,
  groq: "https://api.groq.com/openai/v1/models", river: "https://api.river.ai/v1/models", agnes: "https://apihub.agnes-ai.com/v1/models", sensenova: "https://token.sensenova.cn/v1/models", commandcode: "https://api.commandcode.ai/provider/v1/models", cohere: "https://api.cohere.com/v1/models?endpoint=chat&page_size=1000" } as const;
export type CatalogProviderKind = keyof typeof catalogSources | "qwen" | "cloudflare-workers-ai";
export const officialCatalogSchema = z.object({
  sourceUrl: z.string().min(1).max(4096),
  accountBaseUrl: z.string().min(1).max(4096).optional(),
  checkedAt: z.string().datetime({ offset: true }),
  supportedEndpoints: z.record(z.string(), z.array(z.enum([
    "/chat/completions", "/responses", "/messages", "/systemone",
  ])).min(1).max(4).refine((routes) => new Set(routes).size === routes.length)).optional(),
  models: z.array(z.string().min(1).max(256).refine((id) => id.trim() === id
    && new TextEncoder().encode(id).length <= 256 && !/[\p{Cc}*?]/u.test(id))).min(1).max(2048)
    .refine((models) => new Set(models).size === models.length),
}).strict().refine((catalog) => {
  if (catalog.accountBaseUrl === undefined) return Object.values(catalogSources).some((source) => source === catalog.sourceUrl);
  const endpoint = qwenCatalogEndpoint(catalog.accountBaseUrl) ?? cloudflareEndpoint(catalog.accountBaseUrl);
  return !!endpoint && endpoint.baseUrl === catalog.accountBaseUrl && endpoint.sourceUrl === catalog.sourceUrl;
}, "模型目录来源或账号绑定无效。").refine((catalog) => {
  const endpoints = catalog.supportedEndpoints;
  if (!endpoints || Object.keys(endpoints).length === 0) return true;
  return catalog.sourceUrl === catalogSources.commandcode
    && Object.keys(endpoints).length === catalog.models.length
    && catalog.models.every((model) => Object.hasOwn(endpoints, model));
}, "模型协议声明与目录不匹配。");
export const officialCatalogResponseSchema = z.object({ revision: z.string(), catalog: officialCatalogSchema });
export type OfficialModelCatalog = z.infer<typeof officialCatalogSchema>;
export type CatalogInput = { credentialId: string } | { providerId: string; apiKey: string; accountBaseUrl?: string };
export type DiscoverProviderModels = (input: CatalogInput, signal?: AbortSignal) => Promise<OfficialModelCatalog>;
export function officialCatalogBase(kind: CatalogProviderKind, base: string): boolean {
  if (kind === "zenmux") return zenmux.isApprovedBaseUrl(base);
  if (kind === "qwen") return qwenCatalogEndpoint(base) !== null;
  if (kind === "cloudflare-workers-ai") return cloudflareEndpoint(base) !== null;
  if (kind === "mistral") return base.replace(/\/+$/, "") === "https://api.mistral.ai/v1";
  return (kind === "nvidia" ? ["https://integrate.api.nvidia.com", "https://integrate.api.nvidia.com/v1"]
    : kind === "openai" ? ["https://api.openai.com/v1"]
    : kind === "anthropic" ? ["https://api.anthropic.com"]
    : kind === "gemini" ? ["https://generativelanguage.googleapis.com/v1beta"]
    : kind === "poe" ? ["https://api.poe.com", "https://api.poe.com/v1"]
      : kind === "groq" ? ["https://api.groq.com/openai/v1"]
        : kind === "river" ? ["https://api.river.ai/v1"]
        : kind === "agnes" ? ["https://apihub.agnes-ai.com/v1"]
        : kind === "sensenova" ? ["https://token.sensenova.cn/v1"]
        : kind === "commandcode" ? ["https://api.commandcode.ai/provider/v1"]
        : kind === "cohere" ? ["https://api.cohere.com"]
      : ["https://openrouter.ai", "https://openrouter.ai/api", "https://openrouter.ai/api/v1"]).includes(base.replace(/\/+$/, ""));
}
export function catalogProviderKind(provider: Record<string, unknown>): CatalogProviderKind | undefined {
  if (provider.preset === zenmux.template.preset) return "zenmux";
  if (isQwenProvider(provider)) return "qwen";
  if (provider.preset === "cloudflare-workers-ai") return "cloudflare-workers-ai";
  if (provider.preset === "gemini-api") return "gemini";
  if (provider.preset === "anthropic") return "anthropic";
  if (provider.preset === "openai" && typeof provider.base_url === "string"
    && officialCatalogBase("openai", provider.base_url)) return "openai";
  if (provider.preset === "nvidia-openai") return "nvidia";
  if (provider.preset === "poe-openai") return "poe";
  if (provider.preset === "groq-openai") return "groq";
  if (provider.preset === "river-api") return "river";
  if (provider.preset === "agnes-api") return "agnes";
  if (provider.preset === "sensenova-token-plan") return "sensenova";
  if (provider.preset === "commandcode-api") return "commandcode";
  if (provider.preset === "cohere-chat") return "cohere";
  if (["mistral-api", "mistral-studio-api", "mistral-vibe-api"].some((preset) => preset === provider.preset)) return "mistral";
  if (provider.preset === "openrouter-openai" || (provider.preset === "openai"
    && typeof provider.base_url === "string" && officialCatalogBase("openrouter", provider.base_url))) return "openrouter";
}
export function catalogMatchesProvider(provider: Record<string, unknown>, base: unknown, catalog: OfficialModelCatalog): boolean {
  const kind = catalogProviderKind(provider);
  if (kind === "qwen" || kind === "cloudflare-workers-ai") {
    const endpoint = typeof base === "string" ? kind === "qwen" ? qwenCatalogEndpoint(base) : cloudflareEndpoint(base) : null;
    return !!endpoint && endpoint.baseUrl === catalog.accountBaseUrl && endpoint.sourceUrl === catalog.sourceUrl;
  }
  return !!kind && catalog.accountBaseUrl === undefined && catalog.sourceUrl === catalogSources[kind]
    && (base === undefined || typeof base === "string" && officialCatalogBase(kind, base));
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
  if (!isRecord(provider) || !catalogProviderKind(provider)) throw new Error("模型目录与服务商不匹配。");
  const credentials = Array.isArray(provider.credentials) ? provider.credentials.filter(isRecord) : [];
  const credential = credentials.find((entry) => entry.id === credentialId);
  if (!credential) throw new Error("模型目录账号已不存在。");
  const base = credential.base_url ?? provider.base_url;
  if (!catalogMatchesProvider(provider, base, catalog)) throw new Error("模型目录与服务商或账号目标不匹配。");
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
