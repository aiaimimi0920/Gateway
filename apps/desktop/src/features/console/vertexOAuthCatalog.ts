// The template contains no OAuth client, credentials, quota-project header or inferred model.
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import { vertexTargetBase } from "./vertexOAuth";

export const VERTEX_OAUTH_TEMPLATE: ProviderCatalogTemplate = {
  id: "vertex-oauth", providerId: "vertex-oauth", labelZh: "vertex-oauth", labelEn: "vertex-oauth",
  descriptionZh: "Full Vertex 用户 OAuth；显式 project/location，与 Express API Key 独立分池。",
  descriptionEn: "Full Vertex user OAuth with explicit project/location, separate from Express API Key.",
  category: "mainstream", compatibility: "native", preset: "google-agent-platform-official-api",
  vendorKey: "google-vertex", vendorName: "Google / Vertex", baseUrl: "", supportedModels: [],
};
export function vertexOAuthCatalogBase(draft: ProviderCatalogDraft): string {
  const target = draft.vertexTarget;
  const base = target && target.quota_project === undefined ? vertexTargetBase(target) : null;
  if (!base || base !== draft.baseUrl.replace(/\/$/, "")) throw new Error("请填写有效的 Full Vertex Project / Location；地址必须与显式目标一致。");
  return base;
}
