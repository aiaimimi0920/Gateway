// A staged replacement is authorized for a public product/audience, not for an editable array slot.
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { isQwenProvider, requireQwenEndpoint } from "./qwenEndpoint";

export type QwenMaterialBinding = { product: "qwen-official-api"; baseUrl: string; authMode: string };
export function qwenMaterialTarget(document: ConsoleRouteDocument, providerId: string, credentialId: string): QwenMaterialBinding {
  const provider = document.providers.find((p) => isRecord(p) && p.id === providerId);
  const credential = isRecord(provider) && Array.isArray(provider.credentials)
    ? provider.credentials.find((c) => isRecord(c) && c.id === credentialId) : undefined;
  if (!isRecord(provider) || !isRecord(credential) || !isQwenProvider(provider)) throw new Error("Qwen Key 必须绑定对应产品的精确账号。");
  const mode = credential.auth_mode ?? credential.authMode ?? provider.auth_mode ?? provider.authMode ?? "qwen_workspace_api";
  if (!["qwen_workspace_api", "bearer"].includes(String(mode))) throw new Error("Qwen Key 的认证模式不匹配。");
  return { product: "qwen-official-api", baseUrl: requireQwenEndpoint(String(credential.base_url ?? provider.base_url)), authMode: String(mode) };
}
export function validateQwenStaging(document: ConsoleRouteDocument, providerId: string, credentialId: string,
  binding?: QwenMaterialBinding): void {
  const target = qwenMaterialTarget(document, providerId, credentialId);
  if (!binding || target.product !== binding.product || target.baseUrl !== binding.baseUrl || target.authMode !== binding.authMode) {
    throw new Error("暂存 Qwen Key 后目标已变化，或材料未绑定目标；请重新录入匹配地域/工作空间的 Key。");
  }
}
