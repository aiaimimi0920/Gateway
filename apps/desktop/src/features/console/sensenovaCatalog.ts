// Token Plan Key 与控制台登录身份分开，目录不表示套餐权益。
import type { ProviderCatalogTemplate } from "./providerCatalog";
export const SENSENOVA_TEMPLATE = {
  id: "sensenova-token-plan", providerId: "sensenova-token-plan-api", labelZh: "sensenova-token-plan-api", labelEn: "sensenova-token-plan-api",
  descriptionZh: "SenseNova Token Plan 订阅 Key。", descriptionEn: "SenseNova Token Plan subscription Key.",
  category: "mainstream", compatibility: "openai", preset: "sensenova-token-plan", protocolProfile: "sensenova_token_plan",
  vendorKey: "sensenova", vendorName: "SenseNova", baseUrl: "https://token.sensenova.cn/v1", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
