// Catalog visibility is upstream metadata, not proof of successful model generation.
import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { registerChatgptCredentialModels } from "./chatgptCredentialModels";
import { applyModelSelection, modelSelectionSchema } from "./credentialModelSelection";

export const chatgptCatalogSchema = z.object({ clientVersion: z.string().optional(), checkedAt: z.string(),
  models: z.array(z.object({ slug: z.string().min(1).max(200), visibility: z.string().nullable(),
    supported_in_api: z.boolean().nullable() })).min(1).max(2048) });
export const chatgptCatalogResponseSchema = z.object({ revision: z.string(), catalog: chatgptCatalogSchema });
export type ChatgptCatalog = z.infer<typeof chatgptCatalogSchema>;
export type ChatgptCatalogInput = { credentialId: string } | { accessToken: string; accountId: string };
export type DiscoverChatgptModels = (input: ChatgptCatalogInput, signal?: AbortSignal) => Promise<ChatgptCatalog>;
export const isChatgptCatalogProvider = (provider: Record<string, unknown>) =>
  provider.preset === "chatgpt-codex-oauth-official-api" || provider.preset === "chatgpt-web-reverse";

export function visibleChatgptModels(catalog: ChatgptCatalog): string[] {
  return catalog.models.filter((model) => model.visibility !== "hide" && model.visibility !== "hidden"
    && model.supported_in_api !== false).map((model) => model.slug);
}

export function applyChatgptCatalog(document: ConsoleRouteDocument, providerId: string, credentialId: string,
  catalog: ChatgptCatalog): ConsoleRouteDocument {
  const next = structuredClone(document);
  const provider = next.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  if (!isRecord(provider) || !isChatgptCatalogProvider(provider)) throw new Error("ChatGPT 服务商已不存在。");
  const credential = Array.isArray(provider.credentials) && provider.credentials.find((entry) => isRecord(entry) && entry.id === credentialId);
  if (!isRecord(credential)) throw new Error("ChatGPT 账号已不存在。");
  const selection = modelSelectionSchema.safeParse(credential.model_selection).data;
  if (selection) return applyModelSelection(next, providerId, credentialId, { ...selection, fetched: visibleChatgptModels(catalog) });
  const declared = Array.isArray(credential.supported_models) ? credential.supported_models as string[] : [];
  if (provider.preset === "chatgpt-web-reverse") return applyModelSelection(next, providerId, credentialId,
    { fetched: visibleChatgptModels(catalog), manual: declared });
  // Keep explicit operator declarations (including models absent from this version's catalog).
  credential.supported_models = [...new Set([...declared, ...visibleChatgptModels(catalog)])];
  return registerChatgptCredentialModels(next, providerId, credential.supported_models as string[]);
}
