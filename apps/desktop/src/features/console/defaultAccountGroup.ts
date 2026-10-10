import type { ConsoleRouteDocument } from "../../api/contracts";
import type { RouteAccountGroup } from "./routeAccountCatalog";
import { isRecord } from "./routeDocument";

/** Derive the same fallback membership as the server without modifying a saved revision. */
export function withDefaultAccountGroup(
  document: ConsoleRouteDocument, groups: RouteAccountGroup[],
): RouteAccountGroup[] {
  const assigned = new Set(groups.filter((group) => group.id !== "default")
    .flatMap((group) => group.providerCredentialIds));
  const members = new Set<string>();
  for (const provider of document.providers) {
    if (!isRecord(provider) || typeof provider.id !== "string" || !provider.id.trim()) continue;
    const providerId = provider.id.trim();
    const credentials = Array.isArray(provider.credentials) ? provider.credentials.filter(isRecord) : [];
    const ids = credentials.length ? credentials.map((credential, index) =>
      typeof credential.id === "string" && credential.id.trim()
        ? credential.id.trim() : `${providerId}-cred-${index}`) : [`${providerId}::default`];
    for (const id of ids) if (!assigned.has(id)) members.add(id);
  }
  const existing = groups.find((group) => group.id === "default");
  const fallback: RouteAccountGroup = {
    id: "default", name: "default", description: null, billingMultiplier: 1,
    enabled: true, notes: null, ...existing, providerCredentialIds: [...members],
  };
  return existing ? groups.map((group) => group.id === "default" ? fallback : group) : [...groups, fallback];
}
