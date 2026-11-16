import type { ConsoleRouteDocument } from "../../api/contracts";
import { sessionProductForPreset, SESSION_PRODUCTS, type SessionProduct } from "./providerSessionProducts";

export type SessionMaterialBinding = { product: SessionProduct; realmId?: string; agentId?: string; expiresAt?: string; userAgent?: string };
const record = (value: unknown): Record<string, unknown> => value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
function assertDestination(target: unknown, baseUrl: string) {
  try {
    if (typeof target !== "string") throw new Error();
    const actual = new URL(target);
    const expected = new URL(baseUrl);
    if (actual.username || actual.password || actual.search || actual.hash || actual.origin !== expected.origin
      || actual.pathname.replace(/\/$/, "") !== expected.pathname.replace(/\/$/, "")) throw new Error();
  } catch { throw new Error("会话材料目标地址不是此产品的固定入口；请先检查服务商及账号配置。"); }
}

export function sessionAuthForProduct(product: SessionProduct, expiresAt?: string): Record<string, unknown> {
  const cookie = SESSION_PRODUCTS[product].cookie;
  return cookie && product !== "suno-web" ? { transport: "cookie", primary_cookie_name: cookie,
    secondary_cookie_name: product === "grok-web" ? "sso-rw" : undefined, expires_at: expiresAt }
    : { transport: "bearer", header_name: "authorization", expires_at: expiresAt };
}

/** Reused at staging and commit, so later JSON edits cannot redirect a pending product secret. */
export function sessionMaterialTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string, binding: SessionMaterialBinding, committing = false) {
  const provider = document.providers.map(record).find((entry) => entry.id === providerId);
  if (!provider || sessionProductForPreset(provider.preset) !== binding.product) throw new Error("会话材料只能导入相同产品的服务商，不能跨 API/OAuth/Web 凭据池。");
  const credential = (Array.isArray(provider.credentials) ? provider.credentials : []).map(record).find((entry) => entry.id === credentialId);
  if (!credential) throw new Error("找不到目标会话账号。");
  const definition = SESSION_PRODUCTS[binding.product];
  for (const owner of [provider, credential]) {
    // The management wire serializes absent optional addresses as null; they inherit below.
    if (owner.base_url != null) assertDestination(owner.base_url, definition.baseUrl);
    if (["adapter", "auth_mode", "authMode", "auth_header_name", "protocol_profile", "protocol_family", "responses_path", "chat_completions_path", "messages_path",
      "refresh_endpoint", "refresh_client_id", "token_expires_in_secs"].some((field) => owner[field] != null)) {
      throw new Error("会话材料不接受覆盖产品协议、认证或请求地址的配置。");
    }
    for (const flow of [record(owner.call_flow), record(owner.callFlow)]) {
      if (flow.special_adapter !== undefined && flow.special_adapter !== "auto" && flow.special_adapter !== definition.special
        || flow.fingerprint_adapter !== undefined && !["auto", "session"].includes(String(flow.fingerprint_adapter))
        || flow.call_adapter !== undefined && flow.call_adapter !== "auto") throw new Error("会话材料与已配置的产品执行器不一致。");
    }
    if (binding.product === "freebuff-web" && record(owner.extra_body).freebuffModelAgentMap !== undefined) throw new Error("会话材料不支持已有 FreeBuff model-agent 映射，请使用独立账号及空池。");
    if (binding.product === "qwen-web" && ["authSeed", "auth_seed", "signinSeed", "signin_seed"].some((key) => Object.hasOwn(record(owner.extra_body), key))) {
      throw new Error("此账号存在 Qwen 恢复 seed；请使用独立新账号导入会话，避免混用恢复身份。");
    }
  }
  assertDestination(credential.base_url ?? provider.base_url, definition.baseUrl);
  if (provider.expires_at != null || record(provider.session_auth).expires_at != null) throw new Error("会话材料到期时间必须归属账号，不能继承服务商级会话时间。");
  if (committing && (binding.realmId !== undefined && record(credential.extra_body).realmId !== binding.realmId
    || binding.agentId !== undefined && record(credential.extra_body).freebuffAgentId !== binding.agentId)) throw new Error("会话材料绑定的 realmId 或 agentId 已变化，请重新导入。");
  if (committing) {
    const expected = sessionAuthForProduct(binding.product, binding.expiresAt);
    const actual = record(credential.session_auth);
    if (Object.keys(expected).some((key) => (actual[key] ?? undefined) !== expected[key])
      || Object.keys(actual).some((key) => actual[key] != null && !Object.hasOwn(expected, key))
      || (credential.expires_at ?? undefined) !== binding.expiresAt || binding.expiresAt && Date.parse(binding.expiresAt) <= Date.now()) throw new Error("会话材料认证或有效期已变化，请重新导入。");
    if (["perplexity-web", "qwen-web"].includes(binding.product)) {
      // UA belongs to this imported session, not a provider default or a later JSON edit.
      const agents = Object.entries(record(credential.headers)).filter(([key]) => key.toLowerCase() === "user-agent");
      if (binding.product === "perplexity-web" && !binding.userAgent
        || (binding.userAgent ? agents.length !== 1 || agents[0][1] !== binding.userAgent : agents.length !== 0)
        || Object.keys(record(provider.headers)).length > 0) throw new Error("会话材料原始 User-Agent 或账号归属已变化，请重新导入。");
    }
  }
  return { provider, credential };
}
