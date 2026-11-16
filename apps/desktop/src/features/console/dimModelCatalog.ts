// Typed declarations stay account-local and never imply successful generation or entitlement.
import { z } from "zod";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { editModelSources, modelTextIds } from "./credentialModelSelection";
import { dimSite } from "./dimCatalog";
import { isRecord } from "./routeDocument";
const text = (max: number) => z.string().min(1).refine((s) => s.trim().length > 0
  && new TextEncoder().encode(s).length <= max && !/\p{Cc}/u.test(s));
const strings = z.array(text(128)).max(32);
const limit = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).nullable();
export const dimModelCatalogSchema = z.object({
  site: z.enum(["china", "global"]), client_id: text(512), subject: text(255),
  models: z.array(z.object({ id: text(256), name: text(1024), protocol: z.enum(["chat", "responses", "messages", "gemini"]),
    context_window: limit, max_output_tokens: limit, tools: z.boolean().nullable(), reasoning: z.boolean().nullable(),
    reasoning_efforts: strings, input_modalities: strings, output_modalities: strings, service_tiers: strings,
    deprecated: z.boolean(), zdr: z.boolean() }).strict()).max(2048),
}).strict().refine((c) => new Set(c.models.map((m) => m.id)).size === c.models.length
  && new TextEncoder().encode(JSON.stringify(c)).length <= 2 * 1024 * 1024);
export type DimModelCatalog = z.infer<typeof dimModelCatalogSchema>;
export const dimCatalogResponseSchema = z.object({ revision: z.string().min(1), providerId: text(256), credentialId: text(256), catalog: dimModelCatalogSchema }).strict();
export type DimCatalogBinding = { providerId: string; credentialId: string; site: DimModelCatalog["site"]; clientId: string; subject: string; baseUrl: string };

export function savedDimBinding(provider: Record<string, unknown> | undefined, credentialId: string): DimCatalogBinding | null {
  const site = dimSite(provider);
  if (!site || !provider || provider.auth_mode !== "dim_user_oauth" || typeof provider.id !== "string") return null;
  const credential = Array.isArray(provider.credentials) ? provider.credentials.find((c) => isRecord(c) && c.id === credentialId) : null;
  if (!isRecord(credential) || !isRecord(credential.dim_oauth)) return null;
  const context = credential.dim_oauth;
  const baseUrl = site === "china" ? "https://dimagent.cn/v1" : "https://dimagent.com/v1";
  if (provider.base_url !== baseUrl || credential.base_url !== undefined && credential.base_url !== baseUrl || context.site !== site
    || !text(512).safeParse(context.client_id).success || !text(255).safeParse(context.subject).success) return null;
  return { providerId: provider.id, credentialId, site, clientId: String(context.client_id), subject: String(context.subject), baseUrl };
}
export function dimCatalogMatches(catalog: DimModelCatalog, binding: DimCatalogBinding): boolean {
  return catalog.site === binding.site && catalog.client_id === binding.clientId && catalog.subject === binding.subject;
}
export function stageDimCatalog(value: CredentialDialogValue, input: unknown, binding: DimCatalogBinding): CredentialDialogValue {
  const catalog = dimModelCatalogSchema.parse(input);
  if (value.providerId !== binding.providerId || value.credentialId !== binding.credentialId || value.apiKeyOperation !== "keep"
    || value.baseUrl && value.baseUrl !== binding.baseUrl || !dimCatalogMatches(catalog, binding)) throw new Error("DIM 目录与当前账号或目标不匹配。");
  const ids = catalog.models.map((m) => m.id); const fetched = ids.join("\n");
  // The shared editor accepts CSV/newlines: reject IDs it would split or trim instead of changing an opaque ID.
  if (JSON.stringify(modelTextIds(fetched)) !== JSON.stringify(ids)) throw new Error("DIM 目录含当前模型编辑器无法无损表示的模型 ID。");
  return editModelSources({ ...value, dimCatalog: catalog }, fetched, value.manualModelsText ?? "");
}
export function dimCatalogFields(provider: Record<string, unknown> | undefined, value: CredentialDialogValue): Record<string, unknown> {
  if (!value.dimCatalog) return {};
  const binding = savedDimBinding(provider, value.credentialId);
  if (!binding) throw new Error("DIM 目录账号已不存在或身份已变化。");
  return { dim_catalog: stageDimCatalog(value, value.dimCatalog, binding).dimCatalog };
}
