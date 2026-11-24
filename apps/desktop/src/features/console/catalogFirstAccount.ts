import type { ConsoleRouteDocument } from "../../api/contracts";
import { addExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";

// Typed OAuth/session and vendor material bindings remain owned by their existing dialogs.
export function supportsCatalogApiKey(template: ProviderCatalogTemplate): boolean {
  return Boolean(template.custom) || ["openai", "anthropic", "gemini-api", "groq-openai", "openrouter-openai", "nvidia-openai", "poe-openai", "cohere-chat", "vercel-ai-gateway"].includes(template.preset ?? "");
}
export function addCatalogFirstAccount(document: ConsoleRouteDocument, template: ProviderCatalogTemplate, draft: ProviderCatalogDraft): ConsoleRouteDocument {
  if (!draft.credentialId) return document;
  if (!supportsCatalogApiKey(template)) throw new Error("此服务商需要通过专用账号流程完成认证。");
  if (template.custom && draft.workflow?.specialAdapter && draft.workflow.specialAdapter !== "none") throw new Error("启用专用适配器时，请先添加服务商，再通过专用账号流程完成认证。");
  if (!draft.apiKey?.trim()) throw new Error("请填写首个账号的 API Key。");
  return addExplicitCredential(document, { providerId: draft.providerId, credential: {
    id: draft.credentialId.trim(), account_name: draft.accountName?.trim() || undefined,
    enabled: true, supported_models: draft.supportedModels.length ? draft.supportedModels : undefined,
  } });
}
export function catalogFirstAccountSecret(draft: ProviderCatalogDraft): CredentialSecretEdit | null {
  return draft.credentialId && draft.apiKey?.trim() ? {
    providerId: draft.providerId, credentialId: draft.credentialId.trim(), field: "api_key", operation: "replace", value: draft.apiKey.trim(),
  } : null;
}
