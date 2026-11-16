import type { DimSite } from "../../api/console/dim-auth";
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const DIM_TEMPLATES: readonly ProviderCatalogTemplate[] = (["china", "global"] as const).map((site) => ({
  id: site === "china" ? "dim-cn-oauth" : "dim-global-oauth", providerId: site === "china" ? "dim-cn-oauth" : "dim-global-oauth",
  labelZh: site === "china" ? "DIM 国内 OAuth" : "DIM 海外 OAuth", labelEn: site === "china" ? "DIM China OAuth" : "DIM Global OAuth",
  descriptionZh: "独立订阅账号；需要有权供 Gateway 使用的 public client 与精确登记回调，目录声明不代表模型已验收。",
  descriptionEn: "Separate subscription account; requires an authorized public client and exact registered redirect. Catalog declarations are not verified calls.",
  category: "aggregator", compatibility: "native", preset: site === "china" ? "dim-cn-oauth" : "dim-global-oauth",
  vendorKey: site === "china" ? "dim-cn" : "dim-global", vendorName: site === "china" ? "DIM China" : "DIM Global",
  baseUrl: site === "china" ? "https://dimagent.cn/v1" : "https://dimagent.com/v1", supportedModels: [],
}));
export function dimSite(provider: Record<string, unknown> | undefined): DimSite | null {
  return provider?.preset === "dim-cn-oauth" ? "china" : provider?.preset === "dim-global-oauth" ? "global" : null;
}
export function writeDimProvider(provider: Record<string, unknown>, template: ProviderCatalogTemplate) {
  if (!DIM_TEMPLATES.some((t) => t.id === template.id)) return;
  if (provider.base_url !== template.baseUrl) throw new Error("DIM 仅允许与当前站点一致的固定服务地址。");
  provider.auth_mode = "dim_user_oauth";
  provider.explicit_credentials_only = true;
}
