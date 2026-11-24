import type { ConsoleRouteDocument } from "../../api/contracts";
import { providerAccessKind } from "./providerAccessKind";
import { isRecord } from "./routeDocument";

export type ProviderConnectionDraft = { label: string; baseUrl: string };
// These products bind authentication to an official endpoint or a typed account target.
const MANAGED_TARGETS = new Set([
  "qwen-official-api", "bedrock-converse", "agnes-api", "sensenova-token-plan",
  "commandcode-api", "minimax-m-plan", "minimax-m-plan-cn", "kiro-api-key",
  "vertex-express-api", "cloudflare-workers-ai", "sap-aicore-chat", "lumalabs-api", "lumalabs-inference-api",
]);

export function providerConnectionEditable(provider: Record<string, unknown>): boolean {
  const kind = providerAccessKind({
    providerPreset: typeof provider.preset === "string" ? provider.preset : undefined,
    adapter: typeof provider.adapter === "string" ? provider.adapter : undefined,
    protocolProfile: typeof provider.protocol_profile === "string" ? provider.protocol_profile : undefined,
    authMode: typeof provider.auth_mode === "string" ? provider.auth_mode : undefined,
  });
  return kind === "api" || kind === "custom";
}

export function providerTargetEditable(provider: Record<string, unknown>): boolean {
  return providerConnectionEditable(provider) && !MANAGED_TARGETS.has(String(provider.preset))
    && !["luma_agents", "vertex_express", "kiro_api_key", "sap_aicore"].includes(String(provider.auth_mode));
}

export function providerConnectionValue(provider: Record<string, unknown>): ProviderConnectionDraft {
  return {
    label: typeof provider.label === "string" ? provider.label : String(provider.id),
    baseUrl: typeof provider.base_url === "string" ? provider.base_url : "",
  };
}

export function normalizeProviderTarget(value: string): string {
  const target = value.trim().replace(/\/+$/, "");
  let url: URL;
  try { url = new URL(target); } catch { throw new Error("请输入有效的 HTTP(S) 目标网址。"); }
  if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.username || url.password || url.search || url.hash || /[\u0000-\u001f\u007f]/.test(target)) {
    throw new Error("目标网址须使用 HTTP(S)，不能包含账号密码、查询参数或片段。");
  }
  return target;
}

/** Edit public metadata by stable ID; secret patches and revision/CAS keep their existing owners. */
export function editProviderConnection(document: ConsoleRouteDocument, providerId: string,
  expected: string, value: ProviderConnectionDraft): ConsoleRouteDocument {
  const matches = document.providers.filter(p => isRecord(p) && p.id === providerId);
  const provider = matches[0];
  if (matches.length !== 1 || !isRecord(provider) || !providerConnectionEditable(provider)) throw new Error("服务商已变化，请重新打开设置。");
  if (JSON.stringify(provider) !== expected) throw new Error("服务商配置已变化，请取消编辑后重试。");
  const label = value.label.trim();
  if (!label || label.length > 256 || /[\u0000-\u001f\u007f]/.test(label)) throw new Error("服务商名称不能为空，且不能超过 256 个字符。");
  const previous = providerConnectionValue(provider).baseUrl.trim().replace(/\/+$/, "");
  const changed = value.baseUrl.trim().replace(/\/+$/, "") !== previous;
  if (changed && !providerTargetEditable(provider)) throw new Error("此服务商使用专用目标配置，不能在此修改网址。");
  const target = changed ? normalizeProviderTarget(value.baseUrl) : provider.base_url;
  const next = structuredClone(document);
  const updated = next.providers[document.providers.indexOf(provider)] as Record<string, unknown>;
  updated.label = label;
  if (!changed) return next;
  updated.base_url = target;
  for (const account of Array.isArray(updated.credentials) ? updated.credentials : []) {
    if (!isRecord(account)) continue;
    const override = typeof account.base_url === "string" ? account.base_url.trim().replace(/\/+$/, "") : null;
    if (override !== null && override !== previous) continue;
    if (override !== null) account.base_url = target;
    // Old evidence belongs to the old address. Keep configured model choices and route order.
    delete account.discovery;
    delete account.discovery_job;
    delete account.model_catalog;
  }
  return next;
}
