// Match OAuth's additive model registration without claiming live model discovery.
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

export function registerChatgptCredentialModels(document: ConsoleRouteDocument, providerId: string, models: string[]): ConsoleRouteDocument {
  const next = structuredClone(document);
  const provider = next.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  if (!isRecord(provider) || provider.preset !== "chatgpt-codex-oauth-official-api") return document;
  const existing = Array.isArray(provider.supported_models) ? provider.supported_models.filter((model): model is string => typeof model === "string") : [];
  provider.supported_models = [...new Set([...existing, ...models])];
  if (!provider.default_model && models.length) provider.default_model = models[0];
  for (const pattern of models) {
    const route = next.model_routes.find((entry) => isRecord(entry) && entry.pattern === pattern);
    if (!route) next.model_routes.push({ pattern, provider_ids: [providerId] });
    else if (!isRecord(route) || !Array.isArray(route.provider_ids)) throw new Error(`Invalid model route '${pattern}'.`);
    else if (!route.provider_ids.includes(providerId)) route.provider_ids.push(providerId);
  }
  return next;
}
