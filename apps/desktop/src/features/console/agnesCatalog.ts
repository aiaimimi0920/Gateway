// 国际站订阅及普通 Key 共用推理目标；不猜模型、余额或账号权益。
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const AGNES_TEMPLATE = {
  id: "agnes-api", providerId: "agnes-api", labelZh: "agnes-api", labelEn: "agnes-api",
  descriptionZh: "Agnes AI 国际站 API Key / 订阅 Key。",
  descriptionEn: "Agnes AI international API / subscription Key.",
  category: "mainstream", compatibility: "openai", preset: "agnes-api",
  protocolProfile: "agnes_api", vendorKey: "agnes-ai", vendorName: "Agnes AI",
  baseUrl: "https://apihub.agnes-ai.com/v1", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
