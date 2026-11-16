// M Plan 的区域和订阅 Key 独立绑定；平台目录不等于账号订阅权益。
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const MINIMAX_TEMPLATES = [
  { id: "minimax-m-plan", baseUrl: "https://api.minimax.io/anthropic" },
  { id: "minimax-m-plan-cn", baseUrl: "https://api.minimax.cn/anthropic" },
].map((region) => ({
  ...region, providerId: `${region.id}-api`, labelZh: `${region.id}-api`, labelEn: `${region.id}-api`, preset: region.id,
  descriptionZh: "M Plan 订阅 Key。", descriptionEn: "M Plan subscription Key.",
  category: "mainstream", compatibility: "anthropic", protocolProfile: "minimax_m_plan",
  vendorKey: "minimax", vendorName: "MiniMax", supportedModels: [],
} satisfies ProviderCatalogTemplate));
