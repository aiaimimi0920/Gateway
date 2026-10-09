import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

/** Remove one pool atomically, retaining unrelated accounts, routes and files. */
export function removeProviderDocument(document: ConsoleRouteDocument, providerId: string): ConsoleRouteDocument {
  const next = structuredClone(document);
  const provider = next.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  if (!isRecord(provider)) throw new Error(`Provider '${providerId}' was not found.`);
  const ids = new Set([`${providerId}::default`]);
  if (Array.isArray(provider.credentials)) {
    for (const credential of provider.credentials) {
      if (isRecord(credential) && typeof credential.id === "string") ids.add(credential.id);
    }
  }
  next.providers = next.providers.filter((entry) => !isRecord(entry) || entry.id !== providerId);
  next.model_routes = next.model_routes.filter((route) => {
    if (!isRecord(route) || !Array.isArray(route.provider_ids)) return true;
    const remaining = route.provider_ids.filter((id) => id !== providerId);
    route.provider_ids = remaining;
    return remaining.length > 0;
  });
  if (Array.isArray(next.account_groups)) {
    for (const group of next.account_groups) {
      if (isRecord(group) && Array.isArray(group.provider_credential_ids)) {
        group.provider_credential_ids = group.provider_credential_ids.filter((id) => !ids.has(String(id)));
      }
    }
  }
  return next;
}
