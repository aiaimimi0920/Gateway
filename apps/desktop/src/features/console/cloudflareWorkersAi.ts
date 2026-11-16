// A pool is bound to one Cloudflare account; API tokens are saved by the normal secret owner.
import type { ProviderCatalogTemplate } from "./providerCatalog";

export const CLOUDFLARE_TEMPLATE = {
  id: "cloudflare-workers-ai", providerId: "cloudflare-workers-ai", labelZh: "cloudflare-workers-ai", labelEn: "cloudflare-workers-ai",
  descriptionZh: "Cloudflare Account ID + Workers AI API Token；Chat/SSE、Embeddings，Responses 仅两个 GPT-OSS 模型的非流式请求。目录不代表额度。",
  descriptionEn: "Cloudflare Account ID + Workers AI API Token; Chat/SSE, Embeddings and non-streaming Responses for the two GPT-OSS models only. Catalog visibility is not quota.",
  category: "mainstream", compatibility: "openai", preset: "cloudflare-workers-ai",
  vendorKey: "cloudflare", vendorName: "Cloudflare Workers AI", baseUrl: "", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;

export function cloudflareBase(accountId: string): string | null {
  return /^[a-f0-9]{32}$/.test(accountId)
    ? `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1` : null;
}
export function cloudflareEndpoint(base: string): { accountId: string; baseUrl: string; sourceUrl: string } | null {
  const match = /^https:\/\/api\.cloudflare\.com\/client\/v4\/accounts\/([a-f0-9]{32})\/ai\/v1\/?$/.exec(base);
  if (!match) return null;
  const baseUrl = cloudflareBase(match[1])!;
  return { accountId: match[1], baseUrl,
    sourceUrl: baseUrl.replace(/\/v1$/, "/models/search?task=Text%20Generation&per_page=100&page=1") };
}
export function requireCloudflareBase(base: string): string {
  const endpoint = cloudflareEndpoint(base);
  if (!endpoint) throw new Error("请填写 32 位小写十六进制 Cloudflare Account ID；仅支持官方 Workers AI 地址。");
  return endpoint.baseUrl;
}
