// Express is a fixed Key audience; neither AI Studio catalogues nor full Vertex OAuth belong here.
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { isRecord } from "./routeDocument";

export const VERTEX_EXPRESS_BASE = "https://aiplatform.googleapis.com/v1/publishers/google";
export const isVertexExpressProvider = (provider: unknown): boolean =>
  isRecord(provider) && provider.preset === "vertex-express-api";
export const validVertexExpressModel = (id: string) => id.length <= 2048 && /^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(id);
type Binding = { product: "vertex-express-api"; baseUrl: string; authMode: "vertex_express_api_key" };
export type VertexExpressMaterialBinding = Binding;

function target(provider: unknown, credential: Record<string, unknown>): Binding {
  if (!isRecord(provider) || !isVertexExpressProvider(provider)) throw new Error("Vertex Express Key 必须绑定原产品和精确账号。");
  const baseUrl = String(credential.base_url ?? provider.base_url).replace(/\/$/, "");
  const authMode = credential.auth_mode ?? provider.auth_mode ?? "vertex_express_api_key";
  const flow = provider.call_flow ?? provider.callFlow;
  if (baseUrl !== VERTEX_EXPRESS_BASE || authMode !== "vertex_express_api_key"
    // Management JSON uses null for unset optional fields; non-null overrides remain constrained.
    || provider.adapter != null && provider.adapter !== "gemini_api_compatible"
    || provider.protocol_profile != null && !["vertex_express_api", "vertex-express-api"].includes(String(provider.protocol_profile))
    || provider.request_client_profile != null && provider.request_client_profile !== "auto"
    || flow != null && (!isRecord(flow) || flow.call_adapter && flow.call_adapter !== "auto"
      || flow.fingerprint_adapter && flow.fingerprint_adapter !== "auto" || flow.special_adapter && !["auto", "none"].includes(String(flow.special_adapter)))) {
    throw new Error("Vertex Express API Key 仅支持固定的 global publisher-model 地址和原生调用流程。");
  }
  return { product: "vertex-express-api", baseUrl, authMode };
}
export function vertexExpressMaterialTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string): Binding {
  const provider = document.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  const credential = isRecord(provider) && Array.isArray(provider.credentials)
    ? provider.credentials.find((entry) => isRecord(entry) && entry.id === credentialId) : undefined;
  if (!isRecord(credential)) throw new Error("Vertex Express 账号已不存在。");
  return target(provider, credential);
}
export function validateVertexExpressStaging(document: ConsoleRouteDocument, providerId: string, credentialId: string, binding?: Binding): void {
  const current = vertexExpressMaterialTarget(document, providerId, credentialId);
  if (!binding || current.product !== binding.product || current.baseUrl !== binding.baseUrl || current.authMode !== binding.authMode) {
    throw new Error("暂存 Vertex Express Key 后目标已变化，或材料未绑定目标；请重新录入。");
  }
}
export function normalizeVertexExpressDraft(provider: unknown, value: CredentialDialogValue, editing: boolean): CredentialDialogValue {
  if (!isVertexExpressProvider(provider)) return value;
  target(provider, value.baseUrl ? { base_url: value.baseUrl } : {});
  if (value.discovery || value.modelCatalog || value.fetchedModelsText?.trim() || value.sessionMaterial
    || value.webSessionMaterial || value.bedrockMaterial || value.qwenEndpoint || value.chatgptAccountId
    || !editing && value.apiKeyOperation !== "replace") {
    throw new Error("Vertex Express 使用原生 API Key 和手工精确模型，不能借用目录、OAuth 或会话材料。");
  }
  if (value.apiKeyOperation === "replace" && !/^[\x21-\x7e]{1,32768}$/.test(value.apiKeyValue.trim())) {
    throw new Error("请填写有效的 Vertex Express API Key，不能包含空白或控制字符。");
  }
  return value;
}
