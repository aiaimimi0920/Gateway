import type { ProviderCatalogTemplate } from "./providerCatalog";

export const QUICK_OAUTH_BASE = "https://quick.aws.com";
export const QUICK_OAUTH_MODELS = ["smart", "fast", "balanced", "auto"] as const;

export const QUICK_OAUTH_TEMPLATE: ProviderCatalogTemplate = {
  id: "quick-oauth",
  providerId: "quick-oauth",
  labelZh: "quick-oauth",
  labelEn: "quick-oauth",
  descriptionZh: "Amazon Quick 社交登录 OAuth；支持本机自动回调或手动粘贴回调链接、流式对话与客户端工具桥接。",
  descriptionEn: "Amazon Quick social OAuth with automatic local callback or manual callback paste, streaming chat, and client tool bridging.",
  category: "mainstream",
  compatibility: "native",
  preset: "quick-oauth",
  vendorKey: "amazon-quick",
  vendorName: "Amazon Quick",
  baseUrl: QUICK_OAUTH_BASE,
  supportedModels: QUICK_OAUTH_MODELS,
};

export function isQuickOAuthProvider(provider: Record<string, unknown> | undefined): boolean {
  return provider?.preset === "quick-oauth" || provider?.auth_mode === "quick_oauth";
}

export function writeQuickProvider(provider: Record<string, unknown>, template: ProviderCatalogTemplate) {
  if (template.id !== QUICK_OAUTH_TEMPLATE.id) return;
  if (provider.base_url !== QUICK_OAUTH_BASE) {
    throw new Error("Amazon Quick 必须使用固定的 https://quick.aws.com 地址。");
  }
  provider.auth_mode = "quick_oauth";
  provider.explicit_credentials_only = true;
}
