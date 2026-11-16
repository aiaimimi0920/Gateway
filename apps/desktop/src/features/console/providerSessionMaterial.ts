import { SESSION_PRODUCTS, type SessionProduct } from "./providerSessionProducts";
import { parsePerplexityWebMaterial } from "./perplexityWebMaterial";

export const SESSION_MATERIAL_MAX_BYTES = 128 * 1024;
export type ProviderSessionMaterial = {
  product: SessionProduct;
  token?: string;
  cookieHeader?: string;
  userAgent?: string;
  expiresAt?: string;
  realmId?: string;
  agentId?: string;
};
function text(value: unknown, field: string, limit: number): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "string" || !value.trim() || value.length > limit || /[\u0000-\u001f\u007f-\u009f]/.test(value)) {
    throw new Error(`会话材料字段 ${field} 无效。`);
  }
  return value.trim();
}
function cookies(header?: string): Map<string, string> {
  const entries = new Map<string, string>();
  for (const item of header?.split(";") ?? []) {
    if (!item.trim()) continue;
    const separator = item.indexOf("=");
    const name = item.slice(0, separator).trim();
    if (separator <= 0 || !/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(name) || entries.has(name)) throw new Error("会话材料 Cookie 格式无效或存在重复项。");
    entries.set(name, item.slice(separator + 1).trim());
  }
  return entries;
}
function expiry(value: unknown, token?: string): string | undefined {
  let milliseconds: number | undefined;
  if (token?.split(".").length === 3) {
    try {
      const claims = JSON.parse(atob(token.split(".")[1].replaceAll("-", "+").replaceAll("_", "/")));
      if (Number.isSafeInteger(claims?.exp)) milliseconds = claims.exp * 1000;
    } catch { /* Unverified claims only narrow expiry; they never authenticate a session. */ }
  }
  const supplied = text(value, "expiresAt", 64);
  if (supplied !== undefined) {
    const match = /^(\d{4})-(\d\d)-(\d\d)T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d(?:\.\d{1,9})?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)$/.exec(supplied);
    if (!match || Number(match[3]) < 1 || Number(match[2]) < 1 || Number(match[2]) > 12
      || Number(match[3]) > new Date(Date.UTC(Number(match[1]), Number(match[2]), 0)).getUTCDate() || !Number.isFinite(Date.parse(supplied))) {
      throw new Error("会话材料 expiresAt 必须是有效的 RFC3339 时间。");
    }
    milliseconds = Math.min(milliseconds ?? Infinity, Date.parse(supplied));
  }
  if (milliseconds === undefined) return undefined;
  if (!Number.isFinite(milliseconds) || milliseconds <= Date.now() || milliseconds > 8640000000000000) throw new Error("会话材料 token 已过期或到期时间无效。");
  return new Date(milliseconds).toISOString();
}

/** Strict projection: imported files cannot select headers, endpoints, refresh grants or other products. */
export function parseProviderSessionMaterial(source: string, product: SessionProduct): ProviderSessionMaterial {
  if (product === "perplexity-web") return parsePerplexityWebMaterial(source);
  if (new TextEncoder().encode(source).length > SESSION_MATERIAL_MAX_BYTES) throw new Error("会话材料不能超过 128 KiB。");
  let input: Record<string, unknown>;
  try { input = JSON.parse(source); } catch { throw new Error("会话材料不是有效的 JSON。"); }
  if (!input || typeof input !== "object" || Array.isArray(input) || input.product !== product || !Object.hasOwn(SESSION_PRODUCTS, product)) throw new Error("会话材料 product 与目标渠道不匹配。");
  const allowed = ["product", "token", "cookieHeader", "userAgent", "expiresAt",
    ...(product === "lumalabs-web" ? ["realmId"] : []), ...(product === "freebuff-web" ? ["agentId"] : [])];
  if (Object.keys(input).some((key) => !allowed.includes(key))) throw new Error("会话材料包含不支持的字段；请勿导入 API/OAuth、恢复 seed 或目的地配置。");
  const cookieHeader = text(input.cookieHeader, "cookieHeader", 65536);
  const cookieMap = cookies(cookieHeader);
  const definition = SESSION_PRODUCTS[product];
  let token = text(input.token, "token", 32768);
  if (token && (/[;\s]/.test(token) || definition.cookie && token.startsWith(`${definition.cookie}=`))) throw new Error("会话材料 token 只能填写值，完整 Cookie 请填 cookieHeader。");
  const cookieToken = definition.cookie ? cookieMap.get(definition.cookie) : undefined;
  if (token && cookieToken && token !== cookieToken) throw new Error("会话材料 token 与 Cookie 身份不一致。");
  token ??= cookieToken || undefined;
  const chunks = [...cookieMap.keys()].filter((key) => key.startsWith("sb-ssr-production-auth-token."));
  const splitUdio = product === "udio-web" && chunks.length > 0;
  if (splitUdio && (token || chunks.length > 32 || chunks.some((_, index) => !cookieMap.get(`sb-ssr-production-auth-token.${index}`)))) throw new Error("会话材料 Udio 分片 Cookie 不完整或与 token 冲突。");
  if (!token && !splitUdio) throw new Error("会话材料缺少此产品需要的 token 或认证 Cookie。");
  if (product === "suno-web" && !cookieToken) throw new Error("会话材料 Suno 必须包含带 __session 的完整 Cookie。");
  const realmId = text(input.realmId, "realmId", 256);
  const agentId = text(input.agentId, "agentId", 256);
  if ((product === "lumalabs-web" && !realmId) || (product === "freebuff-web" && !agentId)) throw new Error("会话材料缺少必需的 realmId 或 agentId。");
  if ([realmId, agentId].some((id) => id && !/^[A-Za-z0-9_:@.\/-]+$/.test(id))) throw new Error("会话材料 realmId 或 agentId 格式无效。");
  const userAgent = text(input.userAgent, "userAgent", 2048);
  const expiresAt = expiry(input.expiresAt, token);
  return { product, ...(token ? { token } : {}), ...(cookieHeader ? { cookieHeader } : {}), ...(userAgent ? { userAgent } : {}),
    ...(expiresAt ? { expiresAt } : {}), ...(realmId ? { realmId } : {}), ...(agentId ? { agentId } : {}) };
}
