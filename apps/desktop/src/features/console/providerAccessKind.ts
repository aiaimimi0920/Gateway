// 展示分类只读取公开协议元数据，不读取密钥，也不改写服务商或路由 ID。
import { isVertexOAuthProvider } from "./vertexOAuth";
export const PROVIDER_ACCESS_KINDS = ["oauth", "api", "web", "custom"] as const;
export type ProviderAccessKind = typeof PROVIDER_ACCESS_KINDS[number];
type ProviderAccessMetadata = {
  providerId?: string; providerLabel?: string; providerPreset?: string | null;
  adapter?: string | null; protocolProfile?: string | null;
  manualAddFamily?: string | null; authMode?: string | null;
};
const WEB_PROFILES = new Set(["gemini-business", "gemini-canvas", "gemini-canvas-chat",
  "grok", "suno", "udio", "producer", "lumalabs", "chataibot", "freebuff"]);

export function providerAccessKind(provider: ProviderAccessMetadata): ProviderAccessKind {
  if (provider.adapter === "lumalabs_mcp_readonly" && provider.protocolProfile === "lumalabs_mcp_readonly" && provider.authMode === "luma_mcp_oauth") return "oauth";
  if (provider.authMode === "lobsterai_session") return "oauth";
  // Full Vertex shares legacy presets with generic API configurations, so require its typed product identity.
  if (isVertexOAuthProvider({ preset: provider.providerPreset, auth_mode: provider.authMode })) return "oauth";
  // CLI Key and OAuth share an executor/profile, not an authentication identity.
  if (provider.providerPreset?.trim().toLowerCase() === "kiro-api-key") return "api";
  const metadata = [provider.providerPreset, provider.protocolProfile, provider.adapter, provider.manualAddFamily]
    .filter((value): value is string => Boolean(value)).map((value) => value.toLowerCase().replaceAll("_", "-").replace(/-compatible$/, ""));
  // 显式协议优先于用户自定义名称；OAuth 官方接口不因包含 api 而被归入 API Key。
  if (metadata.some((value) => value.includes("oauth") || ["kiro", "codex", "accio"].includes(value))) return "oauth";
  if (metadata.some((value) => WEB_PROFILES.has(value) || /(^|-)(web|browser|canvas)(-|$)/.test(value))) return "web";
  if (!provider.providerPreset && ["openai", "openai_compatible", "anthropic", "anthropic_compatible", "gemini", "gemini_compatible"].includes(provider.adapter ?? "")) return "custom";
  return "api";
}

export function providerAccessKindLabel(kind: ProviderAccessKind, t: (zh: string, en: string) => string): string {
  return { oauth: t("OAuth 授权", "OAuth"), api: "API Key", web: t("特殊", "Special"), custom: t("自定义", "Custom") }[kind];
}

export function providerAccessLabel(provider: ProviderAccessMetadata): string {
  const accessKind = providerAccessKind(provider);
  const kind = accessKind === "custom" ? "api" : accessKind;
  const original = (provider.providerLabel || provider.providerId || "provider").trim();
  const stem = original.replace(/-(oauth|api|web)$/i, "");
  // Canvas keeps its existing product name; custom pool names still get their access suffix.
  if (kind === "web" && stem === "gemini-canvas") return stem;
  // 旧内置 ChatGPT 标签的展示迁移，不影响历史 ID 和凭据归属。
  return `${kind === "oauth" && /^(chatgpt|codex)$/i.test(stem) ? "codex" : stem}-${kind}`;
}
