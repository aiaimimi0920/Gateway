// Provider API Key；公开目录不证明账号权益或推理成功。
import type { ProviderCatalogTemplate } from "./providerCatalog";
export const COMMANDCODE_TEMPLATE = {
  id: "commandcode-api", providerId: "commandcode-api", labelZh: "commandcode-api", labelEn: "commandcode-api",
  descriptionZh: "Command Code Provider API Key。", descriptionEn: "Command Code Provider API Key.",
  category: "mainstream", compatibility: "openai", preset: "commandcode-api", protocolProfile: "commandcode_chat",
  vendorKey: "commandcode", vendorName: "Command Code", baseUrl: "https://api.commandcode.ai/provider/v1", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
