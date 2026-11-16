// Typed Full Vertex metadata only. Private client/token material belongs to the managed session owner.
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { isRecord } from "./routeDocument";
import type { VertexTarget } from "../../api/console/vertex-auth";

export type { VertexTarget } from "../../api/console/vertex-auth";
const presets = ["google-agent-platform", "google-agent-platform-official-api", "vertex-gemini", "vertex-official-api"];
export const isVertexOAuthProvider = (provider: unknown): boolean =>
  isRecord(provider) && provider.auth_mode === "vertex_user_oauth" && presets.includes(String(provider.preset));
const resource = (value: string) => /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$/.test(value);
export const validVertexTarget = (target: VertexTarget): boolean => resource(target.project)
  && /^[a-z0-9](?:[a-z0-9-]{0,50}[a-z0-9])?$/.test(target.location)
  && (target.quota_project === undefined || resource(target.quota_project));
export function vertexTargetBase(target: VertexTarget): string | null {
  if (!validVertexTarget(target)) return null;
  const host = target.location === "global" ? "aiplatform.googleapis.com" : ["us", "eu"].includes(target.location)
    ? `aiplatform.${target.location}.rep.googleapis.com` : `${target.location}-aiplatform.googleapis.com`;
  return `https://${host}/v1/projects/${target.project}/locations/${target.location}/publishers/google`;
}
function parseTarget(value: unknown): VertexTarget | null {
  if (!isRecord(value) || typeof value.project !== "string" || typeof value.location !== "string"
    || value.quota_project !== undefined && typeof value.quota_project !== "string") return null;
  const target = { project: value.project, location: value.location,
    ...(value.quota_project === undefined ? {} : { quota_project: value.quota_project as string }) };
  return validVertexTarget(target) ? target : null;
}
function targetFromBase(value: unknown): VertexTarget | null {
  if (typeof value !== "string") return null;
  const match = /^https:\/\/[^/]+\/v1\/projects\/([^/]+)\/locations\/([^/]+)\/publishers\/google\/?$/.exec(value);
  if (!match) return null;
  const target = parseTarget({ project: match[1], location: match[2] });
  return target && vertexTargetBase(target) === value.replace(/\/$/, "") ? target : null;
}
export function vertexAuthorizationTarget(provider: unknown, credentialId?: string): VertexTarget | null {
  if (!isRecord(provider) || !isVertexOAuthProvider(provider)) return null;
  if (!credentialId) return targetFromBase(provider.base_url);
  const credential = Array.isArray(provider.credentials) ? provider.credentials.find((entry) => isRecord(entry) && entry.id === credentialId) : undefined;
  if (!isRecord(credential) || !isRecord(credential.vertex_oauth)) return null;
  const target = parseTarget(credential.vertex_oauth.target);
  return target && vertexTargetBase(target) === String(credential.base_url ?? provider.base_url).replace(/\/$/, "") ? target : null;
}
export function normalizeVertexOAuthDraft(provider: unknown, value: CredentialDialogValue, editing: boolean): CredentialDialogValue {
  if (!isVertexOAuthProvider(provider)) return value;
  const target = vertexAuthorizationTarget(provider, value.credentialId);
  if (!editing || value.apiKeyOperation !== "keep" || !target || value.baseUrl && value.baseUrl.replace(/\/$/, "") !== vertexTargetBase(target)
    || value.apiKeyValue || value.discovery || value.modelCatalog || value.fetchedModelsText?.trim() || value.sessionMaterial
    || value.webSessionMaterial || value.bedrockMaterial || value.qwenEndpoint || value.chatgptAccountId) {
    throw new Error("Full Vertex 只允许保留材料的账号编辑；新增或替换请使用独立 OAuth 授权。");
  }
  return value;
}
