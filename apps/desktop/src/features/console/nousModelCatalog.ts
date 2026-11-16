// Directory IDs are editable model selection, never pricing, entitlement or successful-call evidence.
import { z } from "zod";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { editModelSources, modelTextIds } from "./credentialModelSelection";
import { isRecord } from "./routeDocument";
import { isNousProvider } from "./nousCatalog";
import { validModelId } from "./providerModelId";
const id = z.string().min(1).max(512).regex(/^[\x21-\x7e]+$/);
export const nousModelCatalogSchema = z.object({ models: z.array(z.object({ id }).strict()).max(2048) }).strict()
  .refine((c) => new Set(c.models.map((m) => m.id)).size === c.models.length);
export type NousModelCatalog = z.infer<typeof nousModelCatalogSchema>;
export const nousCatalogResponseSchema = z.object({ revision: z.string().min(1), providerId: id, credentialId: id, catalog: nousModelCatalogSchema }).strict();
export type NousCatalogBinding = { providerId: string; credentialId: string; context: string; baseUrl: string };
export function savedNousBinding(provider: Record<string, unknown> | undefined, credentialId: string): NousCatalogBinding | null {
  if (!isNousProvider(provider) || !provider || provider.auth_mode !== "nous_portal_oauth" || typeof provider.id !== "string") return null;
  const c = Array.isArray(provider.credentials) ? provider.credentials.find((c) => isRecord(c) && c.id === credentialId) : null;
  if (!isRecord(c) || !isRecord(c.nous_portal)) return null;
  const context = c.nous_portal;
  const baseUrl = context.inference === "subscription" ? "https://inference-api.nousresearch.com/v1"
    : context.inference === "welcome" ? "https://welcome-api.nousresearch.com/v1" : null;
  if (!baseUrl || (c.base_url ?? provider.base_url) !== baseUrl || !id.safeParse(context.client_id).success
    || typeof context.access_digest !== "string" || !/^[a-f0-9]{64}$/.test(context.access_digest)) return null;
  return { providerId: provider.id, credentialId, baseUrl, context: JSON.stringify(context) };
}
export function stageNousCatalog(value: CredentialDialogValue, input: unknown, binding: NousCatalogBinding): CredentialDialogValue {
  const catalog = nousModelCatalogSchema.parse(input);
  if (value.providerId !== binding.providerId || value.credentialId !== binding.credentialId || value.apiKeyOperation !== "keep"
    || value.baseUrl && value.baseUrl !== binding.baseUrl) throw new Error("Nous 目录与当前账号不匹配。");
  const ids = catalog.models.map((m) => m.id); const fetched = ids.join("\n");
  if (ids.some((id) => !validModelId(id, "nous")) || JSON.stringify(modelTextIds(fetched)) !== JSON.stringify(ids)) {
    throw new Error("目录含模型编辑器无法无损表示的 ID。");
  }
  return editModelSources(value, fetched, value.manualModelsText ?? "");
}
