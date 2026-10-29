import type { ConsoleRouteDocument } from "../../api/contracts";
import { updateExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import { modelSelectionSchema } from "./credentialModelSelection";

export const WEB_MATERIAL_MAX_BYTES = 128 * 1024;
export type ChatgptWebMaterial = {
  accessToken: string;
  cookieHeader: string;
  userAgent: string;
  sessionId?: string;
  expiresAt: string;
  extraBody: Record<string, unknown>;
  modelSlugs?: string[];
};

function record(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown> : {};
}

function text(value: unknown, name: string, limit: number, required = true): string | undefined {
  if (value == null && !required) return undefined;
  if (typeof value !== "string" || !value.trim() || value.length > limit || /[\u0000-\u001f\u007f]/.test(value)) {
    throw new Error(`网页会话字段 ${name} 无效。`);
  }
  return value.trim();
}

// JWT claims are expiry hints, not local proof of identity; upstream still authenticates the token.
function tokenExpiry(token: string): number | undefined {
  const part = token.split(".")[1];
  if (!part) return undefined;
  try {
    const claims = record(JSON.parse(atob(part.replaceAll("-", "+").replaceAll("_", "/"))));
    return typeof claims.exp === "number" && Number.isSafeInteger(claims.exp) ? claims.exp * 1000 : undefined;
  } catch { return undefined; }
}

export function parseChatgptWebMaterial(source: string): ChatgptWebMaterial {
  if (new TextEncoder().encode(source).length > WEB_MATERIAL_MAX_BYTES) throw new Error("网页会话文件不能超过 128 KiB。");
  let input: Record<string, unknown>;
  try { input = record(JSON.parse(source)); } catch { throw new Error("网页会话文件不是有效的 JSON。"); }
  if (input.refreshToken || input.refresh_token || input.oauthTokens) {
    throw new Error("此入口仅导入网页会话；OAuth 刷新凭据请使用独立 OAuth 流程。");
  }
  const headers = record(input.headers);
  const accessToken = text(input.authToken ?? input.accessToken ?? input.access_token, "accessToken", 32768)!;
  const cookieHeader = text(input.cookieHeader ?? headers.Cookie ?? headers.cookie, "Cookie", 65536)!;
  const userAgent = text(input.userAgent ?? headers["User-Agent"], "User-Agent", 2048)!;
  // Never use /api/auth/session's `expires`: it can outlive the access token.
  const rawExpiry = input.accessTokenExpiresAt ?? input.expiresAt;
  let expiry = tokenExpiry(accessToken);
  if (rawExpiry !== undefined) {
    const supplied = text(rawExpiry, "accessTokenExpiresAt", 64)!;
    if (!/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,9})?(?:Z|[+-]\d\d:\d\d)$/.test(supplied) || !Number.isFinite(Date.parse(supplied))) {
      throw new Error("网页会话的 token 到期时间无效。");
    }
    expiry = Math.min(expiry ?? Infinity, Date.parse(supplied));
  }
  if (!expiry || !Number.isFinite(expiry) || expiry <= Date.now() || expiry > 8640000000000000) {
    throw new Error("网页会话缺少有效的 token 到期时间，或 token 已过期。");
  }
  const extraBody: Record<string, unknown> = {};
  extraBody.deviceId = text(input.deviceId, "deviceId", 256);
  const sessionId = text(input.sessionId, "sessionId", 256, false);
  for (const key of ["clientVersion", "clientBuildNumber", "language", "languageCode", "timezone", "chatgptPowDataBuild"]) {
    const value = text(input[key], key, 4096, false);
    if (value !== undefined) extraBody[key] = value;
  }
  if (input.chatgptPowSources != null) {
    if (!Array.isArray(input.chatgptPowSources) || input.chatgptPowSources.length > 32) throw new Error("网页会话 PoW 元数据无效。");
    extraBody.chatgptPowSources = input.chatgptPowSources.map((value) => text(value, "chatgptPowSources", 4096));
  }
  const models = input.modelSlugs === undefined ? undefined
    : modelSelectionSchema.safeParse({ fetched: input.modelSlugs, manual: [] });
  if (models && !models.success) throw new Error("网页会话的模型列表无效。");
  return { accessToken, cookieHeader, userAgent, sessionId, expiresAt: new Date(expiry).toISOString(), extraBody,
    ...(models?.success ? { modelSlugs: [...new Set(models.data.fetched)] } : {}) };
}

/** Only sanitized metadata enters the editor document; all secrets use the existing grant/patch owner. */
export function applyChatgptWebMaterial(document: ConsoleRouteDocument, providerId: string, credentialId: string, material: ChatgptWebMaterial) {
  const provider = document.providers.map(record).find((entry) => entry.id === providerId);
  if (provider?.preset !== "chatgpt-web-reverse") throw new Error("网页会话只能导入 chatgpt-web 服务商。");
  const credential = (Array.isArray(provider.credentials) ? provider.credentials : []).map(record).find((entry) => entry.id === credentialId);
  if (!credential) throw new Error("找不到目标网页账号。");
  const headers = Object.fromEntries(Object.entries(record(credential.headers)).filter(([key]) => !["cookie", "user-agent"].includes(key.toLowerCase())));
  const extraBody = { ...record(credential.extra_body), ...material.extraBody };
  delete extraBody.sessionId;
  const nextDocument = updateExplicitCredential(document, { providerId, credentialId }, {
    headers: { ...headers, "User-Agent": material.userAgent },
    expires_at: material.expiresAt,
    session_auth: { ...record(credential.session_auth), transport: "bearer", header_name: "authorization", expires_at: material.expiresAt },
    extra_body: { ...extraBody, userAgent: material.userAgent, accessTokenExpiresAt: material.expiresAt },
  });
  const secretEdits: CredentialSecretEdit[] = [
    { providerId, credentialId, field: "api_key", operation: "replace", value: material.accessToken },
    { providerId, credentialId, field: "headers/Cookie", operation: "replace", value: material.cookieHeader },
    material.sessionId
      ? { providerId, credentialId, field: "extra_body/sessionId", operation: "replace", value: material.sessionId }
      : { providerId, credentialId, field: "extra_body/sessionId", operation: "clear" },
  ];
  return { document: nextDocument, secretEdits };
}
