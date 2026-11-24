import type { ProviderCatalogTemplate } from "./providerCatalog";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

export const VERCEL_PRESET = "vercel-ai-gateway";
export const VERCEL_CREDIT_AUTH_MODE = "vercel_monthly_credit_key";
const ZERO = [
  "nvidia/nemotron-3.5-lightning-free", "recursive/flint-v1.0",
];
const CREDIT = [
  "alibaba/qwen-3-14b", "alibaba/qwen-3-30b", "alibaba/qwen3-coder-30b-a3b", "alibaba/qwen3.5-flash",
  "deepseek/deepseek-v3.1", "inclusionai/ling-3.0-flash", "liquid/lfm2.5-24b-a2b", "meta/llama-3.3-70b",
  "openai/gpt-5-mini", "openai/gpt-4.1-mini", "google/gemini-2.5-flash-lite",
  "google/gemini-2.5-flash", "minimax/minimax-m2.7", "zai/glm-4.7-flash",
  "moonshotai/kimi-k2.7-code", "alibaba/qwen3-coder-next", "mistral/mistral-small",
];
export const VERCEL_FREE_TEMPLATE: ProviderCatalogTemplate = {
  id: VERCEL_PRESET, providerId: "vercel-ai-gateway-api",
  labelZh: "Vercel AI Gateway", labelEn: "Vercel AI Gateway",
  descriptionZh: "默认添加零价模型。可在账号内启用月度赠额模式，再添加可用赠额抵扣的模型。",
  descriptionEn: "Start with zero-price models. Opt in per account before adding models that use monthly credits.",
  category: "aggregator", compatibility: "openai", preset: VERCEL_PRESET,
  vendorKey: "vercel", vendorName: "Vercel",
  baseUrl: "https://ai-gateway.vercel.sh/v1",
  supportedModels: ZERO,
};

export function vercelModels(monthlyCredit: boolean): string[] {
  return monthlyCredit ? [...ZERO, ...CREDIT] : [...ZERO];
}

export function filterVercelAccountModels(preset: unknown, authMode: unknown, models: string[]): string[] {
  if (preset !== VERCEL_PRESET) return models;
  const allowed = new Set(vercelModels(authMode === VERCEL_CREDIT_AUTH_MODE));
  return models.filter((model) => allowed.has(model));
}

/** Only add exact routes selected on this account; preserve existing route options and order. */
export function registerVercelModels(document: ConsoleRouteDocument, providerId: string, models: string[]) {
  const provider = document.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  if (!isRecord(provider) || provider.preset !== VERCEL_PRESET) return document;
  const allowed = vercelModels(true);
  const selected = models.filter((model) => allowed.includes(model));
  const existing = Array.isArray(provider.supported_models) ? provider.supported_models : [];
  provider.supported_models = [...new Set([...existing, ...selected])];
  for (const pattern of selected) {
    const route = document.model_routes.find((entry) => isRecord(entry) && entry.pattern === pattern);
    if (!route) document.model_routes.push({ pattern, provider_ids: [providerId] });
    else if (isRecord(route) && Array.isArray(route.provider_ids) && !route.provider_ids.includes(providerId)) route.provider_ids.push(providerId);
  }
  return document;
}
