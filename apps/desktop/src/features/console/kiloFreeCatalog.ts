import type { ProviderCatalogTemplate } from "./providerCatalog";

export const KILO_FREE_MODEL = "kilo-auto/free";
export const KILO_FREE_BASE = "https://api.kilo.ai/api/gateway";
export const KILO_FREE_ID = "kilo-free";

export const KILO_FREE_TEMPLATE: ProviderCatalogTemplate = {
  id: KILO_FREE_ID, providerId: KILO_FREE_ID,
  labelZh: "Kilo AI Gateway Free", labelEn: "Kilo AI Gateway Free",
  descriptionZh: "匿名免费对话；按出口 IP 共享限额，不使用付费模型。",
  descriptionEn: "Anonymous free chat with a shared per-IP limit and no paid models.",
  category: "aggregator", compatibility: "openai", preset: KILO_FREE_ID,
  vendorKey: "kilo", vendorName: "Kilo",
  baseUrl: KILO_FREE_BASE, supportedModels: [KILO_FREE_MODEL],
};
