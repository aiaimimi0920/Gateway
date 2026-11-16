// A management-only pool: never pass MCP credentials through generic inference defaults.
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import { LUMA_AGENTS_TEMPLATE } from "./lumaAgents";

const MCP_BASE = "https://agents.lumalabs.ai/mcp";
export const LUMA_CATALOG_TEMPLATES: readonly ProviderCatalogTemplate[] = [LUMA_AGENTS_TEMPLATE, {
  id: "lumalabs-mcp", providerId: "lumalabs-mcp-oauth", labelZh: "lumalabs-mcp-oauth", labelEn: "lumalabs-mcp-oauth",
  descriptionZh: "Luma OAuth / MCP 只读账号池。", descriptionEn: "Luma OAuth / MCP read-only account pool.",
  category: "mainstream", compatibility: "native", preset: null,
  adapter: "lumalabs_mcp_readonly", protocolProfile: "lumalabs_mcp_readonly",
  vendorKey: "lumalabs", vendorName: "Luma", baseUrl: MCP_BASE, supportedModels: [],
}];

export function lumaMcpCatalogProvider(draft: ProviderCatalogDraft): Record<string, unknown> {
  if (draft.baseUrl !== MCP_BASE) throw new Error("Luma MCP 仅支持固定官方地址。");
  const w = draft.workflow;
  // Reject actionable generic settings instead of silently discarding a requested operation.
  if (draft.supportedModels.length || draft.apiKey || Object.values(draft.secrets ?? {}).some(Boolean)
    || draft.siteHealthEnabled || draft.codexBasispointsEnabled || draft.discovery
    || (draft.requestClientProfile && draft.requestClientProfile !== "auto")
    || (w && (Object.values(w.mappings).some((targets) => targets.length) || w.callAdapter !== "auto"
      || w.specialAdapter !== "none" || w.autoRefill || w.autoPrune || w.driverId
      || w.testPolicy.automaticEnabled || w.testPolicy.models.length
      || w.health.enabled || w.storage || w.archive || w.permanentDelete))) {
    throw new Error("Luma MCP 只读账号池不支持推理、自动任务或通用认证配置。");
  }
  return { id: draft.providerId, label: draft.providerLabel, vendor_key: draft.vendorKey, vendor_name: draft.vendorName,
    base_url: MCP_BASE, adapter: "lumalabs_mcp_readonly", protocol_profile: "lumalabs_mcp_readonly",
    protocol_family: "lumalabs_mcp", auth_mode: "luma_mcp_oauth", explicit_credentials_only: true, credentials: [] };
}
