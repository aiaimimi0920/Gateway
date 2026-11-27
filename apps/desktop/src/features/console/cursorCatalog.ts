import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
export const CURSOR_TEMPLATE: ProviderCatalogTemplate = {
  id: "cursor-remote-oauth", providerId: "cursor-remote", labelZh: "Cursor 账号导入", labelEn: "Cursor account import",
  descriptionZh: "添加后，在账号列表导入本人 Cursor 账号。此入口不提供官方独立模型 API，也不会创建 Cloud Agent。",
  descriptionEn: "After adding this provider, import your Cursor account from the account list. This does not provide an official standalone model API or create a Cloud Agent.",
  category: "mainstream", compatibility: "native", preset: null,
  vendorKey: "cursor", vendorName: "Cursor", baseUrl: "", supportedModels: [],
};
export function isCursorProvider(provider: Record<string, unknown> | undefined): boolean {
  return provider?.auth_mode === "cursor_remote";
}
export function cursorProvider(draft: ProviderCatalogDraft): Record<string, unknown> {
  if (draft.baseUrl || draft.supportedModels.length || draft.credentialId || draft.apiKey ||
      Object.values(draft.secrets ?? {}).some(Boolean)) throw new Error("Cursor 仅通过专用账号授权流程导入。");
  return { id: draft.providerId, label: draft.providerLabel, vendor_key: "cursor", vendor_name: "Cursor",
    base_url: "", auth_mode: "cursor_remote", explicit_credentials_only: true, credentials: [] };
}
