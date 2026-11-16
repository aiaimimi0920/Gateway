// New static Key material is tied to a public fixed audience before autosave, never to an array slot.
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

export type KiroKeyMaterialBinding = { product: "kiro-api-key"; baseUrl: string; authMode: "kiro_api_key" };
export function kiroKeyMaterialTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string): KiroKeyMaterialBinding {
  const provider = document.providers.find((p) => isRecord(p) && p.id === providerId);
  const credential = isRecord(provider) && Array.isArray(provider.credentials)
    ? provider.credentials.find((c) => isRecord(c) && c.id === credentialId) : undefined;
  const baseUrl = isRecord(provider) && isRecord(credential) ? String(credential.base_url ?? provider.base_url).replace(/\/$/, "") : "";
  const authMode = isRecord(provider) && isRecord(credential) ? credential.auth_mode ?? provider.auth_mode ?? "kiro_api_key" : null;
  if (!isRecord(provider) || !isRecord(credential) || provider.preset !== "kiro-api-key"
    || baseUrl !== "https://runtime.us-east-1.kiro.dev" || authMode !== "kiro_api_key") {
    throw new Error("Kiro Key 必须绑定原生 Key 产品的固定 US East runtime 和精确账号。");
  }
  return { product: "kiro-api-key", baseUrl, authMode };
}
export function validateKiroKeyStaging(document: ConsoleRouteDocument, providerId: string, credentialId: string,
  binding?: KiroKeyMaterialBinding): void {
  const target = kiroKeyMaterialTarget(document, providerId, credentialId);
  if (!binding || target.product !== binding.product || target.baseUrl !== binding.baseUrl || target.authMode !== binding.authMode) {
    throw new Error("暂存 Kiro Key 后目标已变化，或材料未绑定目标；请重新录入。");
  }
}
