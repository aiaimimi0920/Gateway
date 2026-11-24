// One explicitly selected SAP Chat deployment; the service key remains a normal protected secret.
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import type { CredentialDialogValue } from "./credentialDialogTypes";
export const SAP_TEMPLATE = {
  id: "sap-aicore-chat", providerId: "sap-aicore-chat-api", labelZh: "sap-aicore-chat-api", labelEn: "sap-aicore-chat-api",
  descriptionZh: "SAP AI Core 推理部署，使用自有 Service Key。",
  descriptionEn: "SAP AI Core inference deployment with your own service key.",
  category: "mainstream", compatibility: "openai", preset: "sap-aicore-chat",
  vendorKey: "sap", vendorName: "SAP AI Core", baseUrl: "", supportedModels: [],
} as const satisfies ProviderCatalogTemplate;
export const SAP_IMAGE_API_VERSION = "2024-10-21";
export const SAP_O3_MINI_API_VERSION = "2024-12-01-preview";
export type SapTarget = { resourceGroup: string; apiVersion: string; imageInputDeployment?: string; o3MiniDeployment?: string; nativeResponses?: boolean; nativeResponsesImageInput?: boolean; storedResponses?: boolean; orchestrationModel?: string; orchestrationImageInput?: boolean; orchestrationReasoning?: boolean; bedrockConverse?: boolean; bedrockImageInput?: boolean };
export function sapEndpoint(base: string): { origin: string; deployment: string } | null {
  const match = /^(https:\/\/api\.ai\.(?:prod\.[a-z0-9_-]{1,32}\.aws|prod-eu20\.westeurope\.azure)\.ml\.hana\.ondemand\.com)\/v2\/inference\/deployments\/([A-Za-z0-9_-]{1,128})$/.exec(base);
  return match ? { origin: match[1], deployment: match[2] } : null;
}
export function validSapTarget(target?: SapTarget): target is SapTarget {
  if (!target || !/^[A-Za-z0-9_-]{1,253}$/.test(target.resourceGroup)) return false;
  if (target.bedrockConverse) return target.orchestrationModel === undefined;
  if (target.orchestrationModel !== undefined) return /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(target.orchestrationModel);
  const date = target.apiVersion.replace(/-preview$/, "");
  return !!date && /^\d{4}-\d{2}-\d{2}$/.test(date) && Number.isFinite(Date.parse(date))
    && new Date(date).toISOString().slice(0, 10) === date;
}
export function setCatalogDeploymentUrl(draft: ProviderCatalogDraft, baseUrl: string): ProviderCatalogDraft {
  if (draft.baseUrl === baseUrl) return draft;
  return { ...draft, baseUrl,
    ...(draft.sapTarget ? { sapTarget: { ...draft.sapTarget, nativeResponses: undefined, nativeResponsesImageInput: undefined, storedResponses: undefined,
      bedrockImageInput: undefined, orchestrationImageInput: undefined, orchestrationReasoning: undefined, orchestrationModel: draft.sapTarget.orchestrationModel === undefined ? undefined : "" } } : {}) };
}
export function writeSapProvider(provider: Record<string, unknown>, draft: ProviderCatalogDraft) {
  if (draft.templateId !== SAP_TEMPLATE.id) return;
  const endpoint = sapEndpoint(draft.baseUrl);
  const target = draft.sapTarget;
  if (!endpoint || !validSapTarget(target)) throw new Error("请填写有效的 SAP 部署 URL、资源组与部署类型。");
  provider.auth_mode = "sap_aicore_service_key";
  provider.headers = { "AI-Resource-Group": target.resourceGroup };
  provider.extra_body = target.bedrockConverse ? { sap_bedrock_converse: true,
    ...(target.bedrockImageInput ? { sap_bedrock_image_input: true } : {}) } : target.orchestrationModel !== undefined ? {
      sap_orchestration_model: target.orchestrationModel,
      ...(target.orchestrationImageInput ? { sap_orchestration_image_input: true } : {}),
      ...(target.orchestrationReasoning ? { sap_orchestration_reasoning: true } : {}),
    } : {
    sap_api_version: target.apiVersion,
    ...(target.nativeResponses ? {
      sap_native_responses: true,
      ...(target.nativeResponsesImageInput ? { sap_native_responses_image_input: true } : {}),
      ...(target.storedResponses ? { sap_stored_responses: true } : {}),
    } : {}),
    ...(target.apiVersion === SAP_IMAGE_API_VERSION && target.imageInputDeployment === draft.baseUrl
      ? { sap_image_input: true } : {}),
    ...(target.apiVersion === SAP_O3_MINI_API_VERSION && target.o3MiniDeployment === draft.baseUrl
      ? { sap_o3_mini: true } : {}),
  };
  provider.supported_models = [endpoint.deployment];
}
export function normalizeSapCredential(provider: Record<string, unknown> | undefined, value: CredentialDialogValue): CredentialDialogValue {
  if (provider?.preset !== SAP_TEMPLATE.preset) return value;
  const base = String(provider.base_url ?? "");
  const endpoint = sapEndpoint(base);
  if (!endpoint || value.baseUrl && value.baseUrl !== base) throw new Error("SAP 账号必须使用服务商绑定的部署地址。");
  if (value.apiKeyOperation === "replace") {
    let key: SapServiceKey;
    try { key = JSON.parse(value.apiKeyValue); } catch { throw new Error("请粘贴完整 SAP Service Key JSON。"); }
    if (!key || typeof key.clientid !== "string" || !key.clientid || !hasSapAuth(key)
      || ![endpoint.origin, `${endpoint.origin}/`, `${endpoint.origin}/v2`, `${endpoint.origin}/v2/`].includes(String(key.serviceurls?.AI_API_URL))) {
      throw new Error("Service Key 必须包含 clientid、匹配的 AI_API_URL，以及一套完整的客户端密钥或 X.509 证书凭据。");
    }
  }
  return { ...value, baseUrl: base, supportedModelsText: endpoint.deployment, manualModelsText: endpoint.deployment,
    fetchedModelsText: "", discovery: undefined, modelCatalog: undefined };
}

type SapServiceKey = {
  clientid?: unknown; clientsecret?: unknown; url?: unknown; certurl?: unknown; certUrl?: unknown;
  certificate?: unknown; key?: unknown; serviceurls?: { AI_API_URL?: unknown };
};

function authSite(url: unknown, certificate: boolean): string | null {
  if (typeof url !== "string") return null;
  const match = (certificate
    ? /^https:\/\/([a-z0-9_-]{1,200})\.authentication\.cert\.([a-z0-9_-]{1,32})\.hana\.ondemand\.com\/*$/
    : /^https:\/\/([a-z0-9_-]{1,200})\.authentication\.([a-z0-9_-]{1,32})\.hana\.ondemand\.com\/*$/).exec(url);
  return match ? `${match[1]}.${match[2]}` : null;
}

function hasSapAuth(key: SapServiceKey): boolean {
  const cert = key.certurl ?? key.certUrl;
  if (cert == null && key.certificate == null && key.key == null) {
    return Boolean(authSite(key.url, false) && typeof key.clientsecret === "string" && key.clientsecret);
  }
  const site = authSite(cert, true);
  return Boolean(site && key.clientsecret == null && !("certurl" in key && "certUrl" in key)
    && (key.url == null || authSite(key.url, false) === site)
    && typeof key.certificate === "string" && key.certificate.trim().startsWith("-----BEGIN CERTIFICATE-----")
    && typeof key.key === "string" && /^-----BEGIN (?:RSA )?PRIVATE KEY-----/.test(key.key.trim()));
}
