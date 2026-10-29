// 展示分类只读取公开协议元数据，不读取密钥，也不改写服务商或路由 ID。
export const PROVIDER_ACCESS_KINDS = ["oauth", "api", "web"] as const;
export type ProviderAccessKind = typeof PROVIDER_ACCESS_KINDS[number];
type ProviderAccessMetadata = {
  providerId?: string; providerLabel?: string; providerPreset?: string | null;
  adapter?: string | null; protocolProfile?: string | null;
  manualAddFamily?: string | null;
};
const WEB_PROFILES = new Set(["gemini-business", "gemini-canvas", "gemini-canvas-chat",
  "grok", "suno", "udio", "producer", "lumalabs", "chataibot"]);

export function providerAccessKind(provider: ProviderAccessMetadata): ProviderAccessKind {
  const metadata = [provider.providerPreset, provider.protocolProfile, provider.adapter, provider.manualAddFamily]
    .filter((value): value is string => Boolean(value)).map((value) => value.toLowerCase().replaceAll("_", "-").replace(/-compatible$/, ""));
  // 显式协议优先于用户自定义名称；OAuth 官方接口不因包含 api 而被归入 API Key。
  if (metadata.some((value) => value.includes("oauth") || value === "kiro" || value === "codex")) return "oauth";
  if (metadata.some((value) => WEB_PROFILES.has(value) || /(^|-)(web|browser|canvas)(-|$)/.test(value))) return "web";
  return "api";
}

export function providerAccessLabel(provider: ProviderAccessMetadata): string {
  const kind = providerAccessKind(provider);
  const original = (provider.providerLabel || provider.providerId || "provider").trim();
  const stem = original.replace(/-(oauth|api|web)$/i, "");
  // 旧内置 ChatGPT 标签的展示迁移，不影响历史 ID 和凭据归属。
  return `${kind === "oauth" && /^(chatgpt|codex)$/i.test(stem) ? "codex" : stem}-${kind}`;
}
