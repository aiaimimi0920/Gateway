import type { ConsoleRouteConfigResponse, ConsoleSecretPatch } from "../../api/contracts";
import type { CredentialSecretEdit } from "./credentialDocument";

export type SecretPatchDraft = {
  operation: ConsoleSecretPatch["operation"];
  value: string;
};

export function createSecretPatchDrafts(
  routeConfig: ConsoleRouteConfigResponse | null,
): Record<string, SecretPatchDraft> {
  return Object.fromEntries(
    (routeConfig?.routeConfig.secrets ?? [])
      .filter((secret) => secret.path.trim().length > 0)
      .map((secret) => [
        secret.path,
        {
          operation: "keep" as const,
          value: "",
        },
      ]),
  );
}

export function buildSecretPatches(
  routeConfig: ConsoleRouteConfigResponse | null,
  drafts: Record<string, SecretPatchDraft>,
): ConsoleSecretPatch[] {
  return (routeConfig?.routeConfig.secrets ?? [])
    .filter((secret) => secret.path.trim().length > 0)
    .map((secret) => {
      const draft = drafts[secret.path];
      if (draft?.operation === "replace") {
        return {
          path: secret.path,
          operation: "replace" as const,
          value: draft.value,
        };
      }
      if (draft?.operation === "clear") {
        return {
          path: secret.path,
          operation: "clear" as const,
        };
      }
      return {
        path: secret.path,
        operation: "keep" as const,
      };
    });
}

export function mergeCredentialSecretEditEntries(
  current: CredentialSecretEdit[],
  nextEntries: CredentialSecretEdit[],
): CredentialSecretEdit[] {
  const merged = [...current];
  const identityKey = (entry: CredentialSecretEdit) =>
    JSON.stringify([entry.providerId, entry.credentialId, entry.field]);
  const firstIndex = new Map<string, number>();
  // Preserve findIndex semantics when the existing draft contains duplicates.
  for (const [index, entry] of current.entries()) {
    const key = identityKey(entry);
    if (!firstIndex.has(key)) firstIndex.set(key, index);
  }
  for (const nextEntry of nextEntries) {
    const key = identityKey(nextEntry);
    const existingIndex = firstIndex.get(key);
    if (existingIndex !== undefined) {
      merged[existingIndex] = nextEntry;
      continue;
    }
    firstIndex.set(key, merged.length);
    merged.push(nextEntry);
  }
  return merged;
}
