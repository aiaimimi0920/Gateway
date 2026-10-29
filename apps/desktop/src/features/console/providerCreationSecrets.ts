// Bind pending creation secrets to provider identity and destination, never array position.
import type { ConsoleRouteDocument, ConsoleSecretPatch } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { healthSecretTarget, healthSecretAudience } from "./siteHealthSecretDraft";
import { siteHealthConfigSchema } from "./siteHealth";
import { readStorageConnection, storageConnectionIdentity, storageSecretFields } from "./providerStorageConnection";

export type ProviderCreationSecret = { providerId: string; field: string; value: string; audience: string };
function target(document: ConsoleRouteDocument, providerId: string, field: string) {
  const index = document.providers.findIndex((entry) => isRecord(entry) && entry.id === providerId);
  const provider = document.providers[index];
  if (!isRecord(provider)) return null;
  if (field === "credential_storage_password") return { path: `/providers/${index}/${field}`, audience: providerId };
  if (field === "site_health/auth/token") {
    const parsed = siteHealthConfigSchema.safeParse(provider.site_health);
    const health = healthSecretTarget(document, providerId, null);
    return parsed.success && parsed.data.auth.kind !== "none" && health
      ? { path: health.path, audience: healthSecretAudience(provider, parsed.data) } : null;
  }
  const match = /^(credential_(?:storage|archive)_connection)\/(password|access_key_id|secret_access_key|session_token)$/.exec(field);
  const connection = match ? readStorageConnection(provider[match[1]]) : undefined;
  if (!match || !connection || !storageSecretFields(connection.type).includes(match[2] as ReturnType<typeof storageSecretFields>[number])) return null;
  return { path: `/providers/${index}/${field}`, audience: storageConnectionIdentity(connection) };
}
export function captureProviderCreationSecrets(document: ConsoleRouteDocument, providerId: string, values: Record<string, string>): ProviderCreationSecret[] {
  return Object.entries(values).filter(([, value]) => value.length > 0).map(([field, value]) => {
    const destination = target(document, providerId, field);
    if (!destination) throw new Error("认证信息没有对应的有效配置。");
    return { providerId, field, value, audience: destination.audience };
  });
}
export function providerCreationSecretPatches(document: ConsoleRouteDocument, edits: ProviderCreationSecret[], strict = true): ConsoleSecretPatch[] {
  return edits.flatMap((edit) => {
    const provider = document.providers.find((entry) => isRecord(entry) && entry.id === edit.providerId);
    if (!provider) return []; // Deleted owners do not retain pending secrets.
    const destination = target(document, edit.providerId, edit.field);
    if (!destination || destination.audience !== edit.audience) {
      if (strict) throw new Error("新服务商认证目标已变化，请重新填写认证信息。");
      return [];
    }
    return [{ path: destination.path, operation: "replace" as const, value: edit.value }];
  });
}
