import type { ProviderCatalogTemplate } from "./providerCatalog";
export const NOUS_TEMPLATE: ProviderCatalogTemplate = {
  id: "nous-portal-oauth", providerId: "nous-portal-oauth", labelZh: "Nous Portal OAuth", labelEn: "Nous Portal OAuth",
  descriptionZh: "Nous 订阅账号设备授权。", descriptionEn: "Device authorization for Nous subscriptions.",
  category: "aggregator", compatibility: "native", preset: "nous-portal-oauth", vendorKey: "nous", vendorName: "Nous Research",
  baseUrl: "https://inference-api.nousresearch.com/v1", supportedModels: [],
};
export const isNousProvider = (provider: Record<string, unknown> | undefined) => provider?.preset === NOUS_TEMPLATE.id;
export function writeNousProvider(provider: Record<string, unknown>, template: ProviderCatalogTemplate) {
  if (template.id !== NOUS_TEMPLATE.id) return;
  if (provider.base_url !== NOUS_TEMPLATE.baseUrl) throw new Error("Nous 服务商必须使用固定服务地址。");
  provider.auth_mode = "nous_portal_oauth";
  provider.explicit_credentials_only = true;
}
