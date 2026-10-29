// Model ownership is separate from immutable directory/protocol evidence.
import { z } from "zod";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./CredentialDialog";
import { isRecord } from "./routeDocument";

const modelId = z.string().min(1).refine((id) => id.trim() === id
  && new TextEncoder().encode(id).length <= 256 && !/[\p{Cc}*?]/u.test(id), "模型 ID 无效。");
export const modelSelectionSchema = z.object({
  fetched: z.array(modelId).max(2048), manual: z.array(modelId).max(2048),
});
export type ModelSelection = z.infer<typeof modelSelectionSchema>;
export const modelTextIds = (text: string): string[] => [...new Set(text.split(/\r?\n|,/).map((id) => id.trim()).filter(Boolean))];
export const selectedModels = (selection: ModelSelection): string[] => [...new Set([...selection.manual, ...selection.fetched])];
export function dialogModelSelection(value: CredentialDialogValue): ModelSelection | undefined {
  if (value.fetchedModelsText === undefined && value.manualModelsText === undefined) return undefined;
  return modelSelectionSchema.parse({ fetched: modelTextIds(value.fetchedModelsText ?? ""), manual: modelTextIds(value.manualModelsText ?? "") });
}
export function withModelEditors(value: CredentialDialogValue): CredentialDialogValue {
  if (value.fetchedModelsText !== undefined || value.manualModelsText !== undefined) return value;
  const models = modelTextIds(value.supportedModelsText);
  const discovered = new Set(value.modelCatalog?.models ?? value.discovery?.models ?? []);
  return editModelSources(value, models.filter((id) => discovered.has(id)).join("\n"), models.filter((id) => !discovered.has(id)).join("\n"));
}
export function editModelSources(value: CredentialDialogValue, fetched: string, manual: string): CredentialDialogValue {
  return { ...value, fetchedModelsText: fetched, manualModelsText: manual,
    supportedModelsText: selectedModels({ fetched: modelTextIds(fetched), manual: modelTextIds(manual) }).join("\n") };
}
const strings = (value: unknown): string[] => Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];

/** Reconcile only this account's ownership; never enable an existing disabled route. */
export function applyModelSelection(document: ConsoleRouteDocument, providerId: string, credentialId: string,
  input: ModelSelection): ConsoleRouteDocument {
  const selection = modelSelectionSchema.parse(input);
  const models = selectedModels(selection);
  const next = structuredClone(document);
  const provider = next.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  if (!isRecord(provider)) throw new Error("服务商已不存在。");
  const credentials = Array.isArray(provider.credentials) ? provider.credentials.filter(isRecord) : [];
  const credential = credentials.find((entry) => entry.id === credentialId);
  if (!credential) throw new Error("账号已不存在。");
  const previous = new Set(strings(credential.supported_models));
  credential.model_selection = selection;
  credential.supported_models = models;
  const inherited = credentials.some((entry) => entry !== credential && !entry.model_selection && !strings(entry.supported_models).length);
  provider.supported_models = [...new Set([
    ...strings(provider.supported_models).filter((id) => inherited || !previous.has(id)),
    ...credentials.flatMap((entry) => strings(entry.supported_models)),
  ])];
  const remaining = new Set(strings(provider.supported_models));
  next.model_routes = next.model_routes.flatMap((entry) => {
    if (!isRecord(entry) || !previous.has(String(entry.pattern)) || remaining.has(String(entry.pattern))
      || entry.enabled === false || !Array.isArray(entry.provider_ids)) return [entry];
    const ids = entry.provider_ids.filter((id) => id !== providerId);
    entry.provider_ids = ids;
    return ids.length ? [entry] : [];
  });
  const routes = new Map<string, Record<string, unknown>>();
  for (const entry of next.model_routes) {
    if (isRecord(entry) && typeof entry.pattern === "string" && !routes.has(entry.pattern)) routes.set(entry.pattern, entry);
  }
  for (const model of models) {
    const route = routes.get(model);
    if (!route) next.model_routes.push({ pattern: model, provider_ids: [providerId], priority: 10, enabled: true });
    else if (isRecord(route) && Array.isArray(route.provider_ids) && !route.provider_ids.includes(providerId)) route.provider_ids.push(providerId);
  }
  return next;
}
