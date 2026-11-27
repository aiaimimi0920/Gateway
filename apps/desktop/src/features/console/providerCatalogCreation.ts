// Provider-only writes are additive and preserve the legacy implicit-default contract.
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { addCatalogFirstAccount } from "./catalogFirstAccount";
import { KILO_FREE_ID } from "./kiloFreeCatalog";
import { providerDefinitionFromCatalogDraft, type ProviderCatalogDraft, type ProviderCatalogTemplate } from "./providerCatalog";

export function addCatalogProvider(document: ConsoleRouteDocument, template: ProviderCatalogTemplate, value: ProviderCatalogDraft): ConsoleRouteDocument {
  if (document.providers.some((provider) => isRecord(provider) && provider.id === value.providerId)) throw new Error(`Provider '${value.providerId}' already exists.`);
  const provider = providerDefinitionFromCatalogDraft(template, value);
  provider.explicit_credentials_only = template.id !== KILO_FREE_ID;
  provider.credentials = [];
  const next = structuredClone(document);
  next.providers.push(provider);
  if (template.id === "cursor-remote-oauth") return next;
  const patterns = [...new Set([...value.supportedModels, ...Object.entries(value.workflow?.mappings ?? {}).filter(([, targets]) => targets.length).map(([source]) => source)])];
  for (const pattern of patterns) {
    const route = next.model_routes.find((entry) => isRecord(entry) && entry.pattern === pattern);
    if (!route) next.model_routes.push({ pattern, provider_ids: [value.providerId] });
    else if (!isRecord(route) || !Array.isArray(route.provider_ids)) throw new Error(`Invalid model route '${pattern}'.`);
    else if (!route.provider_ids.includes(value.providerId)) route.provider_ids.push(value.providerId);
  }
  return addCatalogFirstAccount(next, template, value);
}
