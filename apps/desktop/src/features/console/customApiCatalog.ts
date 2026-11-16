// 自定义 API 的产品入口；与内置厂商固定目标分开定义。
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const CUSTOM_API_TEMPLATE = {
  id: "custom-api-provider",
  providerId: "custom-api",
  labelZh: "custom-api",
  labelEn: "custom-api",
  descriptionZh: "自动识别模型与调用协议。",
  descriptionEn: "Discover models and API protocol automatically.",
  category: "third-party-compatible",
  compatibility: "auto",
  preset: null,
  adapter: "openai_compatible",
  protocolProfile: "openai_compatible_generic",
  vendorKey: "third-party",
  vendorName: "Custom API provider",
  baseUrl: "",
  supportedModels: [],
  custom: true,
} as const satisfies ProviderCatalogTemplate;
