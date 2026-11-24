// Static inference Keys never share Agents credentials, OAuth context or model discovery.
import type { CredentialDialogValue } from "./credentialDialogTypes";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

export type LumaInferenceBinding = { product: "lumalabs-inference-api"; baseUrl: string };
export function lumaInferenceTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string): LumaInferenceBinding {
  const provider = document.providers.find(p => isRecord(p) && p.id === providerId);
  if (!isRecord(provider) || provider.preset !== "lumalabs-inference-api"
    || provider.base_url !== "https://inference.lumalabs.ai/v1" || !Array.isArray(provider.credentials)
    || !provider.credentials.some(c => isRecord(c) && c.id === credentialId
      && (c.base_url == null || c.base_url === provider.base_url))) {
    throw new Error("暂存 Luma Inference Key 后目标已变化，请重新录入。");
  }
  return { product: "lumalabs-inference-api", baseUrl: String(provider.base_url) };
}

export function normalizeLumaInferenceCredential(provider: Record<string, unknown> | undefined,
  value: CredentialDialogValue): CredentialDialogValue {
  if (provider?.preset !== "lumalabs-inference-api") return value;
  const base = "https://inference.lumalabs.ai/v1";
  if (provider.base_url !== base || value.baseUrl && value.baseUrl.replace(/\/+$/, "") !== base
    || value.discovery || value.modelCatalog || value.sessionMaterial || value.webSessionMaterial || value.bedrockMaterial) {
    throw new Error("Luma Inference 仅接受独立 API Key 和固定推理地址。");
  }
  const key = value.apiKeyValue.trim();
  if (value.apiKeyOperation === "replace"
    && (!key || key.length > 16384 || !/^[\x21-\x7e]+$/.test(key) || key.startsWith("luma-api-"))) {
    throw new Error("请填写 Luma Inference 独立 API Key；不能使用 Agents 媒体 Key。");
  }
  return { ...value, baseUrl: "", apiKeyValue: key };
}
