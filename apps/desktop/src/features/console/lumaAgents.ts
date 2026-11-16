// Agents is a static Key audience, not the Web board; no discovery or generation happens here.
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import type { ProviderCatalogTemplate } from "./providerCatalog";
import { dialogModelSelection, modelTextIds, withModelEditors } from "./credentialModelSelection";
import { isRecord } from "./routeDocument";

export const LUMA_AGENTS_BASE = "https://agents.lumalabs.ai/v1";
export const LUMA_AGENTS_TEMPLATE: ProviderCatalogTemplate = {
  id: "lumalabs-api", providerId: "lumalabs-api", labelZh: "lumalabs-api", labelEn: "lumalabs-api",
  descriptionZh: "Luma Agents 官方 API Key；手工选择已实现的图像／视频模型，与 Web 独立分池。",
  descriptionEn: "Official Luma Agents API Key; explicit supported image/video models, separate from Web.",
  category: "mainstream", compatibility: "native", preset: "lumalabs-api",
  vendorKey: "lumalabs-agents", vendorName: "Luma Agents", baseUrl: LUMA_AGENTS_BASE, supportedModels: [],
};
export type LumaAgentsMaterialBinding = { product: "lumalabs-api"; baseUrl: string; authMode: "bearer" };
export const isLumaAgentsProvider = (provider: unknown): boolean => isRecord(provider) && provider.preset === "lumalabs-api";
const validModel = (id: unknown) => typeof id === "string" && ["uni-1", "uni-1-max", "ray-3.2"].includes(id);
const validKey = (key: string) => key.length > 9 && key.length <= 4096 && /^luma-api-[\x21-\x7e]+$/.test(key);
const emptyObject = (value: unknown) => value == null || isRecord(value) && Object.keys(value).length === 0;
const absentFields = ["session_auth", "auth_token", "refresh_token", "refresh_endpoint", "refresh_client_id",
  "token_expires_in_secs", "grok_oauth", "kiro_oauth", "vertex_oauth", "accio_sgcookie", "discovery", "discovery_job",
  "model_catalog", "keepalive", "runtime_state_object_key", "responses_path", "chat_completions_path", "messages_path",
  "completions_path", "embeddings_path", "audio_transcriptions_path", "audio_speech_path", "search_path", "fetch_path", "research_path", "balance_path"];

function validateSurface(source: Record<string, unknown>): void {
  const flow = source.call_flow ?? source.callFlow;
  if (source.base_url != null && (typeof source.base_url !== "string" || source.base_url.replace(/\/$/, "") !== LUMA_AGENTS_BASE)
    || source.auth_mode != null && source.auth_mode !== "bearer"
    || source.adapter != null && source.adapter !== "lumalabs_agents_compatible"
    || source.protocol_profile != null && !["lumalabs_agents_api", "lumalabs-agents-api"].includes(String(source.protocol_profile))
    || source.protocol_family != null && !["lumalabs_agents_media", "lumalabs-agents-media"].includes(String(source.protocol_family))
    || source.request_client_profile != null && source.request_client_profile !== "auto"
    || source.execution_mode != null && source.execution_mode !== "direct_http"
    || source.endpoint_execution_modes != null && (!isRecord(source.endpoint_execution_modes) || Object.values(source.endpoint_execution_modes).some((v) => v !== "direct_http"))
    || source.auth_header_name != null && String(source.auth_header_name).toLowerCase() !== "authorization"
    || source.codex_basispoints_enabled === true || !emptyObject(source.headers) || !emptyObject(source.extra_body)
    || absentFields.some((field) => source[field] != null)
    || flow != null && (!isRecord(flow) || ["call_adapter", "fingerprint_adapter", "special_adapter"].some((key) => flow[key] != null && flow[key] !== "auto"))) {
    throw new Error("Luma Agents 仅支持固定官方地址、静态 Bearer API Key 和全 Auto 原生调用流程。");
  }
  // Mapping sources are caller aliases; only actual upstream targets use the product model contract.
  const models: unknown[] = [];
  if (source.supported_models != null) {
    if (!Array.isArray(source.supported_models)) throw new Error("Luma Agents 模型列表无效。");
    models.push(...source.supported_models);
  }
  if (source.default_model != null) models.push(source.default_model);
  for (const field of ["model_map", "model_map_targets"] as const) {
    if (source[field] == null) continue;
    if (!isRecord(source[field])) throw new Error("Luma Agents 模型映射无效。");
    for (const target of Object.values(source[field])) {
      if (field === "model_map_targets" && !Array.isArray(target)) throw new Error("Luma Agents 模型映射无效。");
      models.push(...(Array.isArray(target) ? target : [target]));
    }
  }
  if (source.model_selection != null) {
    const selection = source.model_selection;
    if (!isRecord(selection) || !Array.isArray(selection.manual) || !Array.isArray(selection.fetched) || selection.fetched.length) {
      throw new Error("Luma Agents 模型仅支持手动选择。");
    }
    models.push(...selection.manual);
  }
  if (models.some((model) => !validModel(model))) throw new Error("Luma Agents 模型仅支持 uni-1、uni-1-max、ray-3.2。");
}

export function validateLumaAgentsProvider(provider: unknown, credential: Record<string, unknown> = {}): LumaAgentsMaterialBinding {
  if (!isRecord(provider) || !isLumaAgentsProvider(provider)) throw new Error("Luma Agents Key 必须绑定原产品和精确账号。");
  validateSurface(provider); validateSurface(credential);
  const baseUrl = String(credential.base_url ?? provider.base_url).replace(/\/$/, "");
  if (baseUrl !== LUMA_AGENTS_BASE) throw new Error("Luma Agents 仅支持固定官方地址。");
  return { product: "lumalabs-api", baseUrl, authMode: "bearer" };
}
export function lumaAgentsMaterialTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string): LumaAgentsMaterialBinding {
  const provider = document.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  const credential = isRecord(provider) && Array.isArray(provider.credentials)
    ? provider.credentials.find((entry) => isRecord(entry) && entry.id === credentialId) : undefined;
  if (!isRecord(credential)) throw new Error("Luma Agents 账号已不存在。");
  return validateLumaAgentsProvider(provider, credential);
}
export function validateLumaAgentsStaging(document: ConsoleRouteDocument, providerId: string, credentialId: string, binding?: LumaAgentsMaterialBinding): void {
  const current = lumaAgentsMaterialTarget(document, providerId, credentialId);
  if (!binding || current.product !== binding.product || current.baseUrl !== binding.baseUrl || current.authMode !== binding.authMode) {
    throw new Error("暂存 Luma Agents Key 后目标已变化，或材料未绑定目标；请重新录入。");
  }
}
export function normalizeLumaAgentsDraft(provider: unknown, input: CredentialDialogValue, editing: boolean): CredentialDialogValue {
  if (!isLumaAgentsProvider(provider)) return input;
  validateLumaAgentsProvider(provider, input.baseUrl ? { base_url: input.baseUrl } : {});
  const value = withModelEditors(input);
  if (value.discovery || value.modelCatalog || value.fetchedModelsText?.trim() || value.sessionMaterial
    || value.webSessionMaterial || value.bedrockMaterial || value.qwenEndpoint || value.chatgptAccountId || value.codexBasispointsEnabled
    || !editing && value.apiKeyOperation !== "replace") {
    throw new Error("Luma Agents 使用静态 API Key 和手工模型，不能借用目录、OAuth 或 Web 会话材料。");
  }
  if (value.apiKeyOperation === "replace" && !validKey(value.apiKeyValue.trim())) {
    throw new Error("请填写有效的 Luma Agents luma-api-* Key（10–4096 个 ASCII 非空白字符）。");
  }
  const models = [...modelTextIds(value.manualModelsText ?? ""), ...modelTextIds(value.supportedModelsText)];
  if (models.some((model) => !validModel(model))) throw new Error("Luma Agents 模型仅支持 uni-1、uni-1-max、ray-3.2。");
  dialogModelSelection(value);
  return value;
}
