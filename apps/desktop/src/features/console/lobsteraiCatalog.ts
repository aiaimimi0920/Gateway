import type { ProviderCatalogTemplate } from "./providerCatalog";
export const LOBSTERAI_TEMPLATE: ProviderCatalogTemplate = {
  id: "lobsterai-session", providerId: "lobsterai", labelZh: "LobsterAI", labelEn: "LobsterAI",
  descriptionZh: "本机个人账号授权与模型目录；推理待接入。", descriptionEn: "Local personal account authorization and catalog; inference pending.",
  category: "aggregator", compatibility: "native", preset: null, adapter: "openai_compatible", vendorKey: "lobsterai", vendorName: "LobsterAI",
  baseUrl: "https://lobsterai-server.youdao.com/api/proxy/v1", supportedModels: [],
};
export const isLobsterAiProvider = (provider: Record<string, unknown> | undefined) => provider?.auth_mode === "lobsterai_session";
export function writeLobsterAiProvider(provider: Record<string, unknown>, template: ProviderCatalogTemplate) {
  if (template.id !== LOBSTERAI_TEMPLATE.id) return;
  if (provider.base_url !== LOBSTERAI_TEMPLATE.baseUrl) throw new Error("LobsterAI 必须使用固定服务地址。");
  provider.auth_mode = "lobsterai_session"; provider.explicit_credentials_only = true;
}
