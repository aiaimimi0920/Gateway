// River's account-scoped directory is fetched explicitly; no stale model/price table.
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const RIVER_TEMPLATE = {
  id: "river-api", providerId: "river-api", labelZh: "river-api", labelEn: "river-api",
  descriptionZh: "River 官方 API Key；Chat / Responses 与账号专属模型目录，不含状态型 Responses 和余额查询。",
  descriptionEn: "River API Key; Chat / Responses and an account-specific model directory, without stateful Responses or balance queries.",
  category: "mainstream", compatibility: "openai", preset: "river-api",
  protocolProfile: "river", vendorKey: "river", vendorName: "River",
  baseUrl: "https://api.river.ai/v1", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
