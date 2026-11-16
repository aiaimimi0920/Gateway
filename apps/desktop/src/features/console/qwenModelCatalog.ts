// Dialog-local receipt ownership only; never serialized, logged, or used as upstream entitlement.
import type { CredentialDialogValue } from "./CredentialDialog";
import { catalogMatchesProvider, officialCatalogSchema, type CatalogInput, type OfficialModelCatalog } from "./officialModelCatalog";

type Material = Pick<CredentialDialogValue, "providerId" | "credentialId" | "apiKeyValue" | "apiKeyOperation"> & { baseUrl: string };
// Weak ownership releases the comparison material when the dialog drops its catalog. No digest,
// secret alias or guessed timestamp is persisted as proof that a different Key fetched a directory.
const receipts = new WeakMap<OfficialModelCatalog, Material>();
export const isQwenReplacementCatalog = (catalog: OfficialModelCatalog | undefined): boolean => !!catalog && receipts.has(catalog);
export function qwenCatalogInput(value: CredentialDialogValue, baseUrl: string, previousBase?: string): CatalogInput {
  if (value.apiKeyOperation === "keep") {
    if (!value.credentialId || previousBase !== baseUrl) throw new Error("Qwen 目标已变化，不能使用保留的旧 Key 拉取目录。");
    return { credentialId: value.credentialId };
  }
  if (value.apiKeyOperation !== "replace" || !value.apiKeyValue.trim()) throw new Error("请填写匹配此目标的新 API Key。");
  return { providerId: value.providerId, apiKey: value.apiKeyValue.trim(), accountBaseUrl: baseUrl };
}
export function acceptQwenCatalog(provider: Record<string, unknown>, value: CredentialDialogValue,
  baseUrl: string, input: OfficialModelCatalog): OfficialModelCatalog {
  const catalog = officialCatalogSchema.parse(input);
  if (!catalogMatchesProvider(provider, baseUrl, catalog)) throw new Error("模型目录与 Qwen 账号目标不匹配。");
  if (value.apiKeyOperation === "replace") receipts.set(catalog, {
    providerId: value.providerId, credentialId: value.credentialId, baseUrl,
    apiKeyValue: value.apiKeyValue.trim(), apiKeyOperation: value.apiKeyOperation,
  });
  return catalog;
}
export function hasFreshQwenCatalog(value: CredentialDialogValue, baseUrl: string): boolean {
  const receipt = value.modelCatalog && receipts.get(value.modelCatalog);
  return !!receipt && value.apiKeyOperation === "replace" && receipt.apiKeyOperation === value.apiKeyOperation
    && receipt.providerId === value.providerId && receipt.credentialId === value.credentialId
    && receipt.baseUrl === baseUrl && receipt.apiKeyValue === value.apiKeyValue.trim();
}
