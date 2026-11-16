// Editing the pool default never authorizes existing material for a new audience.
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { buildQwenEndpoint, isQwenProvider, requireQwenEndpoint, type QwenEndpointDraft } from "./qwenEndpoint";

export function editQwenProviderEndpoint(document: ConsoleRouteDocument, providerId: string,
  expectedProvider: string, target: QwenEndpointDraft, preserveInherited: boolean): ConsoleRouteDocument {
  const matches = document.providers.filter((entry) => isRecord(entry) && entry.id === providerId);
  const provider = matches[0];
  if (matches.length !== 1 || !isRecord(provider) || !isQwenProvider(provider)) throw new Error("找不到唯一的普通 Qwen 服务商。");
  if (JSON.stringify(provider) !== expectedProvider) throw new Error("Qwen 服务商草稿已变化，请关闭并重新打开目标编辑器。");
  if (provider.explicit_credentials_only !== true) throw new Error("请先使用独立显式账号池；此编辑器不迁移服务商默认材料。");
  if (["api_key", "auth_token", "refresh_token"].some((field) => provider[field])) throw new Error("此编辑器不迁移服务商默认材料。");
  const after = buildQwenEndpoint(target);
  if (!after) throw new Error("请选择 Qwen 地域并填写有效工作空间。");
  const before = provider.base_url == null || provider.base_url === "" ? null : requireQwenEndpoint(String(provider.base_url));
  if (provider.credentials !== undefined && !Array.isArray(provider.credentials)) throw new Error("Qwen 账号列表无效。");
  const credentials = (provider.credentials ?? []) as unknown[];
  const seen = new Set<string>();
  for (const credential of credentials) {
    if (!isRecord(credential) || typeof credential.id !== "string" || !credential.id || seen.has(credential.id)) throw new Error("Qwen 账号身份无效。");
    seen.add(credential.id);
    requireQwenEndpoint(String(credential.base_url ?? before));
  }
  const inherited = credentials.filter((credential) => isRecord(credential) && credential.base_url == null);
  if (after !== before && inherited.length && !preserveInherited) throw new Error("请明确固定原目标，不能把继承账号的旧材料移到新目标。");
  const next = structuredClone(document);
  const nextProvider = next.providers[document.providers.indexOf(provider)] as Record<string, unknown>;
  nextProvider.base_url = after;
  if (after !== before && Array.isArray(nextProvider.credentials)) {
    // Pin only inheritance. Existing overrides, model evidence, routes and staged secret bindings stay intact.
    for (const credential of nextProvider.credentials) if (isRecord(credential) && credential.base_url == null) credential.base_url = before;
  }
  return next;
}
