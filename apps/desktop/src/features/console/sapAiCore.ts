// One explicitly selected SAP Chat deployment; the service key remains a normal protected secret.
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import type { CredentialDialogValue } from "./credentialDialogTypes";
export const SAP_TEMPLATE = {
  id: "sap-aicore-chat", providerId: "sap-aicore-chat-api", labelZh: "sap-aicore-chat-api", labelEn: "sap-aicore-chat-api",
  descriptionZh: "SAP AI Core Azure OpenAI Chat 部署，使用自有 Service Key。",
  descriptionEn: "SAP AI Core Azure OpenAI Chat deployment with your own service key.",
  category: "mainstream", compatibility: "openai", preset: "sap-aicore-chat",
  vendorKey: "sap", vendorName: "SAP AI Core", baseUrl: "", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
export type SapTarget = { resourceGroup: string; apiVersion: string };
export function sapEndpoint(base: string): { origin: string; deployment: string } | null {
  const match = /^https:\/\/api\.ai\.prod\.[a-z0-9_-]{1,32}\.aws\.ml\.hana\.ondemand\.com\/v2\/inference\/deployments\/([A-Za-z0-9_-]{1,128})$/.exec(base);
  return match ? { origin: new URL(base).origin, deployment: match[1] } : null;
}
export function writeSapProvider(provider: Record<string, unknown>, draft: ProviderCatalogDraft) {
  if (draft.templateId !== SAP_TEMPLATE.id) return;
  const endpoint = sapEndpoint(draft.baseUrl);
  const target = draft.sapTarget;
  const date = target?.apiVersion.replace(/-preview$/, "");
  if (!endpoint || !target || !/^[A-Za-z0-9_-]{1,253}$/.test(target.resourceGroup)
    || !date || !/^\d{4}-\d{2}-\d{2}$/.test(date) || !Number.isFinite(Date.parse(date))
    || new Date(date).toISOString().slice(0, 10) !== date) throw new Error("请填写有效的 SAP 部署 URL、资源组与 API 版本。");
  provider.auth_mode = "sap_aicore_service_key";
  provider.headers = { "AI-Resource-Group": target.resourceGroup };
  provider.extra_body = { sap_api_version: target.apiVersion };
  provider.supported_models = [endpoint.deployment];
}
export function normalizeSapCredential(provider: Record<string, unknown> | undefined, value: CredentialDialogValue): CredentialDialogValue {
  if (provider?.preset !== SAP_TEMPLATE.preset) return value;
  const base = String(provider.base_url ?? "");
  const endpoint = sapEndpoint(base);
  if (!endpoint || value.baseUrl && value.baseUrl !== base) throw new Error("SAP 账号必须使用服务商绑定的部署地址。");
  if (value.apiKeyOperation === "replace") {
    let key: { clientid?: unknown; clientsecret?: unknown; url?: unknown; serviceurls?: { AI_API_URL?: unknown } };
    try { key = JSON.parse(value.apiKeyValue); } catch { throw new Error("请粘贴完整 SAP Service Key JSON。"); }
    if (!key || typeof key.clientid !== "string" || !key.clientid || typeof key.clientsecret !== "string" || !key.clientsecret
      || typeof key.url !== "string" || !/^https:\/\/[a-z0-9_-]+\.authentication\.[a-z0-9_-]+\.hana\.ondemand\.com\/?$/.test(key.url)
      || ![endpoint.origin, `${endpoint.origin}/`, `${endpoint.origin}/v2`, `${endpoint.origin}/v2/`].includes(String(key.serviceurls?.AI_API_URL))) {
      throw new Error("Service Key 必须包含 clientid、clientsecret、认证 URL 和匹配的 AI_API_URL。");
    }
  }
  return { ...value, baseUrl: base, supportedModelsText: endpoint.deployment, manualModelsText: endpoint.deployment,
    fetchedModelsText: "", discovery: undefined, modelCatalog: undefined };
}
