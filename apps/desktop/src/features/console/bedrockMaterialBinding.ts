import type { ConsoleRouteDocument, ConsoleSecretDescriptor, ConsoleSecretPatch } from "../../api/contracts";
import { awsMaterialField, bedrockEndpoint, bedrockMaterialDraft, isBedrockProvider, record } from "./bedrockMaterial";

export type BedrockMaterialBinding = { context: string };
function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([k,v]) => `${JSON.stringify(k)}:${stable(v)}`).join(",")}}`;
  return JSON.stringify(value) ?? "null";
}
/** Only non-secret destination fields enter the binding; names, models and enabled state do not. */
export function bedrockMaterialTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string) {
  const provider = document.providers.map(record).find((p) => p.id === providerId);
  const credential = (Array.isArray(provider?.credentials) ? provider.credentials : []).map(record).find((c) => c.id === credentialId);
  if (!provider || !credential || !isBedrockProvider(provider)) throw new Error("Bedrock 材料只能保存到对应产品的精确账号。");
  const draft = bedrockMaterialDraft(provider, credential);
  if (!draft.authMode || String(credential.base_url ?? provider.base_url).replace(/\/$/, "") !== bedrockEndpoint(draft.region, draft.fips)) throw new Error("Bedrock 材料的模式、区域或目标已改变，请重新填写材料。");
  const publicFields = ["preset", "adapter", "auth_mode", "authMode", "base_url", "protocol_profile", "protocol_family", "call_flow", "callFlow",
    "request_client_profile", "requestClientProfile", "codex_basispoints_enabled", "execution_mode", "endpoint_execution_modes",
    "runtime_state_object_key", "responses_path", "chat_completions_path", "messages_path", "completions_path", "embeddings_path",
    "audio_transcriptions_path", "audio_speech_path", "search_path", "fetch_path", "research_path", "balance_path", "refresh_endpoint", "refresh_client_id", "token_expires_in_secs"];
  const projection = (owner: Record<string, unknown>) => ({ fields: publicFields.map((key) => [key, owner[key]]),
    regions: Object.entries(record(owner.extra_body)).filter(([key]) => awsMaterialField(key) === "region"),
    session: owner.session_auth != null, keepalive: owner.keepalive != null, discovery: owner.discovery != null });
  for (const owner of [provider, credential]) {
    if (owner.session_auth != null || owner.keepalive != null || owner.discovery != null || owner.refresh_endpoint != null || owner.refresh_client_id != null
      || owner.adapter != null && owner.adapter !== "bedrock_converse_compatible" || owner.preset != null && owner.preset !== "bedrock-converse") {
      throw new Error("Bedrock 专用材料入口不接受其他认证、发现或 keepalive 配置，请使用独立空池。");
    }
    for (const flow of [record(owner.call_flow), record(owner.callFlow)]) {
      if (flow.call_adapter != null && flow.call_adapter !== "auto" || flow.special_adapter != null && !["auto", "none"].includes(String(flow.special_adapter))) throw new Error("Bedrock 材料必须保留原生调用适配器。");
    }
  }
  return { provider, credential, binding: { context: stable([projection(provider), projection(credential)]) } };
}
export function validateBedrockStaging(document: ConsoleRouteDocument, providerId: string, credentialId: string, binding: BedrockMaterialBinding,
  activeDocument: ConsoleRouteDocument, activeSecrets: readonly ConsoleSecretDescriptor[]) {
  if (bedrockMaterialTarget(document, providerId, credentialId).binding.context !== binding.context) throw new Error("暂存 Bedrock 材料后认证目标已变化，请重新填写并保存账号。");
  const index = activeDocument.providers.findIndex((p) => record(p).id === providerId);
  if (activeSecrets.some((s) => s.configured && new RegExp(`^/providers/${index}/(?:api_key$|auth_token$|refresh_token$|headers/|extra_body/|keepalive/)`).test(s.path))) {
    throw new Error("Bedrock 专用材料不能与服务商级共享秘密并用，请使用独立空凭据池。");
  }
}
/** Every historical exact-key alias needs its own clear; map merging is not case-insensitive. */
export function clearBedrockMaterialAliases(patches: Map<string, ConsoleSecretPatch>, path: string) {
  const marker = "/extra_body/";
  const offset = path.lastIndexOf(marker);
  if (offset < 0) return;
  const prefix = path.slice(0, offset + marker.length);
  const family = awsMaterialField(path.slice(prefix.length));
  if (!family || family === "region") return;
  for (const key of patches.keys()) {
    if (key !== path && key.startsWith(prefix) && awsMaterialField(key.slice(prefix.length)) === family) patches.set(key, { path: key, operation: "clear" });
  }
}
