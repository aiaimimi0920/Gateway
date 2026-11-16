// Account-local declarations remain distinct from model-call verification.
import { z } from "zod";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { editModelSources, modelTextIds } from "./credentialModelSelection";
import { isLobsterAiProvider, LOBSTERAI_TEMPLATE } from "./lobsteraiCatalog";
import { isRecord } from "./routeDocument";
const text = (max: number) => z.string().min(1).refine((s) => s.trim() === s && new TextEncoder().encode(s).length <= max && !/\p{Cc}/u.test(s));
const limit = z.number().int().positive().max(Number.MAX_SAFE_INTEGER).nullable();
export const lobsteraiModelCatalogSchema = z.object({ models: z.array(z.object({
  modelId: text(512), modelName: text(512).nullable(), apiFormat: text(512).nullable(), runtimeProfile: text(512).nullable(),
  supportsToolCalling: z.boolean().nullable(), supportsThinking: z.boolean().nullable(), supportsImage: z.boolean().nullable(), supportsVideo: z.boolean().nullable(),
  contextWindow: limit, maxTokens: limit, requestCapabilities: z.array(text(128)).max(64).nullable(), accessible: z.literal(true).nullable(),
}).strict()).max(2048) }).strict().refine((c) => new Set(c.models.map((m) => m.modelId)).size === c.models.length && new TextEncoder().encode(JSON.stringify(c)).length <= 2 * 1024 * 1024);
export type LobsterAiModelCatalog = z.infer<typeof lobsteraiModelCatalogSchema>;
export const lobsteraiCatalogResponseSchema = z.object({ revision: text(256), providerId: text(256), credentialId: text(256), catalog: lobsteraiModelCatalogSchema }).strict();
export type LobsterAiCatalogBinding = { providerId: string; credentialId: string; baseUrl: string; contextKey: string };
export function savedLobsterAiBinding(provider: Record<string, unknown> | undefined, credentialId: string): LobsterAiCatalogBinding | null {
  if (!provider || !isLobsterAiProvider(provider) || provider.base_url !== LOBSTERAI_TEMPLATE.baseUrl || typeof provider.id !== "string") return null;
  const credential = Array.isArray(provider.credentials) ? provider.credentials.find((c) => isRecord(c) && c.id === credentialId) : null;
  if (!isRecord(credential) || !isRecord(credential.lobsterai) || credential.base_url != null && credential.base_url !== provider.base_url) return null;
  return { providerId: provider.id, credentialId, baseUrl: LOBSTERAI_TEMPLATE.baseUrl, contextKey: JSON.stringify(credential.lobsterai) };
}
export function stageLobsterAiCatalog(value: CredentialDialogValue, input: unknown, binding: LobsterAiCatalogBinding): CredentialDialogValue {
  const catalog = lobsteraiModelCatalogSchema.parse(input);
  if (value.providerId !== binding.providerId || value.credentialId !== binding.credentialId || value.apiKeyOperation !== "keep" || value.baseUrl && value.baseUrl !== binding.baseUrl) throw new Error("目录与当前 LobsterAI 账号不匹配。");
  const ids = catalog.models.map((m) => m.modelId); const fetched = ids.join("\n");
  if (JSON.stringify(modelTextIds(fetched)) !== JSON.stringify(ids)) throw new Error("目录含当前编辑器无法无损表示的模型 ID。");
  return editModelSources({ ...value, lobsteraiCatalog: catalog }, fetched, value.manualModelsText ?? "");
}
export function lobsteraiCatalogFields(provider: Record<string, unknown> | undefined, value: CredentialDialogValue): Record<string, unknown> {
  if (!value.lobsteraiCatalog) return {};
  const binding = savedLobsterAiBinding(provider, value.credentialId);
  if (!binding) throw new Error("LobsterAI 账号已不存在或身份已变化。");
  return { lobsterai_catalog: stageLobsterAiCatalog(value, value.lobsteraiCatalog, binding).lobsteraiCatalog };
}
