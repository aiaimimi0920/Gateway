// Compatibility runtime only: explicit profile, no inferred directory, scope or model entitlement.
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";

export const KIRO_OAUTH_TEMPLATE: ProviderCatalogTemplate = {
  id: "kiro-oauth", providerId: "kiro-oauth", labelZh: "kiro-oauth", labelEn: "kiro-oauth",
  descriptionZh: "Kiro Builder ID / IAM Identity Center；显式 profile 的 runtime 兼容通道，需另行确认订阅及模型权益。",
  descriptionEn: "Kiro Builder ID / IAM Identity Center with an explicit profile and compatibility runtime; subscription and model access require verification.",
  category: "mainstream", compatibility: "native", preset: "kiro", vendorKey: "kiro-platform", vendorName: "Kiro",
  baseUrl: "", supportedModels: [],
};

export function kiroProfileTarget(profile: string): string | null {
  const parts = profile.trim().split(":");
  if (profile.length > 2048 || parts.length !== 6 || parts[0] !== "arn" || parts[1] !== "aws"
    || parts[2] !== "codewhisperer" || !["us-east-1", "eu-central-1"].includes(parts[3])
    || !/^[0-9]{12}$/.test(parts[4]) || !/^profile\/[A-Za-z0-9_-]+$/.test(parts[5])) return null;
  return `https://runtime.${parts[3]}.kiro.dev`;
}

export function kiroOAuthCatalogBase(draft: ProviderCatalogDraft): string {
  const base = kiroProfileTarget(draft.kiroProfileArn ?? "");
  if (!base || base !== draft.baseUrl.replace(/\/$/, "")) throw new Error("请填写受支持的 Kiro Profile ARN；业务地址必须与 profile Region 一致。");
  return base;
}

export function writeKiroOAuthCatalog(provider: Record<string, unknown>, draft: ProviderCatalogDraft) {
  provider.base_url = kiroOAuthCatalogBase(draft);
  provider.auth_mode = "kiro_oauth_runtime";
  provider.responses_path = "/";
  provider.extra_body = { profileArn: draft.kiroProfileArn!.trim() };
}
