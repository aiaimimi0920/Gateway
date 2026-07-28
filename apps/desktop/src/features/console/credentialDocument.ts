import type {
  ConsoleRouteDocument,
  ConsoleSecretDescriptor,
  ConsoleSecretPatch,
} from "../../api/contracts";

export type AddExplicitCredentialInput = {
  providerId: string;
  credential: Record<string, unknown>;
};

export type CredentialIdentity = {
  providerId: string;
  credentialId: string;
};

export type CredentialSecretEdit = CredentialIdentity &
  (
    | {
        field: "api_key";
        operation: "replace";
        value: string;
      }
    | {
        field: "api_key";
        operation: "clear";
      }
  );

export type BuildCredentialSecretPatchesInput = {
  activeDocument: ConsoleRouteDocument;
  draftDocument: ConsoleRouteDocument;
  activeSecrets: readonly ConsoleSecretDescriptor[];
  activeSecretPatches?: readonly ConsoleSecretPatch[];
  credentialSecretEdits?: readonly CredentialSecretEdit[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cloneJsonValue<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((entry) => cloneJsonValue(entry)) as T;
  }
  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, cloneJsonValue(entry)]),
    ) as T;
  }
  return value;
}

function requiredId(value: unknown, label: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${label} must be a non-empty string.`);
  }
  return value.trim();
}

function providerIndexById(document: ConsoleRouteDocument, providerId: string): number {
  return document.providers.findIndex(
    (provider) => isRecord(provider) && provider.id === providerId,
  );
}

function credentialIndexById(provider: Record<string, unknown>, credentialId: string): number {
  if (!Array.isArray(provider.credentials)) {
    return -1;
  }
  return provider.credentials.findIndex(
    (credential) => isRecord(credential) && credential.id === credentialId,
  );
}

function documentHasCredentialId(document: ConsoleRouteDocument, credentialId: string): boolean {
  return document.providers.some(
    (provider) =>
      isRecord(provider) && credentialIndexById(provider, credentialId) >= 0,
  );
}

function patchAtPath(patch: ConsoleSecretPatch | undefined, path: string): ConsoleSecretPatch {
  if (!patch || patch.operation === "keep") {
    return { path, operation: "keep" };
  }
  if (patch.operation === "clear") {
    return { path, operation: "clear" };
  }
  return { path, operation: "replace", value: patch.value };
}

function rewriteAccountGroupMember(
  document: ConsoleRouteDocument,
  previousId: string,
  nextId?: string,
): void {
  const groups = document.account_groups;
  if (!Array.isArray(groups)) {
    return;
  }

  for (const group of groups) {
    if (!isRecord(group) || !Array.isArray(group.provider_credential_ids)) {
      continue;
    }
    const seen = new Set<string>();
    group.provider_credential_ids = group.provider_credential_ids.flatMap((entry) => {
      if (typeof entry !== "string") {
        return [entry];
      }
      if (entry === previousId && nextId === undefined) {
        return [];
      }
      const memberId = entry === previousId ? nextId : entry;
      if (memberId === undefined) {
        return [];
      }
      if (seen.has(memberId)) {
        return [];
      }
      seen.add(memberId);
      return [memberId];
    });
  }
}

export function addExplicitCredential(
  document: ConsoleRouteDocument,
  input: AddExplicitCredentialInput,
): ConsoleRouteDocument {
  const providerId = requiredId(input.providerId, "providerId");
  const credentialId = requiredId(input.credential.id, "credential.id");
  if (documentHasCredentialId(document, credentialId)) {
    throw new Error(`Credential '${credentialId}' already exists.`);
  }
  const nextDocument = cloneJsonValue(document);
  const provider = nextDocument.providers.find(
    (entry): entry is Record<string, unknown> => isRecord(entry) && entry.id === providerId,
  );
  if (!provider) {
    throw new Error(`Provider '${providerId}' was not found.`);
  }

  const credentialsValue = provider.credentials;
  if (credentialsValue !== undefined && !Array.isArray(credentialsValue)) {
    throw new Error(`Provider '${providerId}' credentials must be an array.`);
  }
  const credentials = Array.isArray(credentialsValue) ? credentialsValue : [];
  const addingFirstExplicitCredential = credentials.length === 0;
  provider.credentials = [...credentials, { ...cloneJsonValue(input.credential), id: credentialId }];

  if (addingFirstExplicitCredential) {
    rewriteAccountGroupMember(
      nextDocument,
      `${providerId}::default`,
      credentialId,
    );
  }

  return nextDocument;
}

export function deleteExplicitCredential(
  document: ConsoleRouteDocument,
  identity: CredentialIdentity,
): ConsoleRouteDocument {
  const providerId = requiredId(identity.providerId, "providerId");
  const credentialId = requiredId(identity.credentialId, "credentialId");
  const nextDocument = cloneJsonValue(document);
  const provider = nextDocument.providers.find(
    (entry): entry is Record<string, unknown> => isRecord(entry) && entry.id === providerId,
  );
  if (!provider) {
    throw new Error(`Provider '${providerId}' was not found.`);
  }
  if (!Array.isArray(provider.credentials)) {
    throw new Error(`Provider '${providerId}' credentials must be an array.`);
  }

  const credentialIndex = provider.credentials.findIndex(
    (entry) => isRecord(entry) && entry.id === credentialId,
  );
  if (credentialIndex < 0) {
    throw new Error(
      `Credential '${credentialId}' was not found under provider '${providerId}'.`,
    );
  }
  provider.credentials.splice(credentialIndex, 1);
  rewriteAccountGroupMember(
    nextDocument,
    credentialId,
    provider.credentials.length === 0 ? `${providerId}::default` : undefined,
  );

  return nextDocument;
}

export function updateExplicitCredential(
  document: ConsoleRouteDocument,
  identity: CredentialIdentity,
  updates: Record<string, unknown>,
): ConsoleRouteDocument {
  const providerId = requiredId(identity.providerId, "providerId");
  const credentialId = requiredId(identity.credentialId, "credentialId");
  const nextDocument = cloneJsonValue(document);
  const provider = nextDocument.providers.find(
    (entry): entry is Record<string, unknown> => isRecord(entry) && entry.id === providerId,
  );
  if (!provider) {
    throw new Error(`Provider '${providerId}' was not found.`);
  }
  if (!Array.isArray(provider.credentials)) {
    throw new Error(`Provider '${providerId}' credentials must be an array.`);
  }

  const credentialIndex = provider.credentials.findIndex(
    (entry) => isRecord(entry) && entry.id === credentialId,
  );
  if (credentialIndex < 0) {
    throw new Error(
      `Credential '${credentialId}' was not found under provider '${providerId}'.`,
    );
  }
  const credential = provider.credentials[credentialIndex];
  if (!isRecord(credential)) {
    throw new Error(`Credential '${credentialId}' must be an object.`);
  }
  provider.credentials[credentialIndex] = {
    ...credential,
    ...cloneJsonValue(updates),
    id: credentialId,
  };

  return nextDocument;
}

export function buildCredentialSecretPatches({
  activeDocument,
  draftDocument,
  activeSecrets,
  activeSecretPatches = [],
  credentialSecretEdits = [],
}: BuildCredentialSecretPatchesInput): ConsoleSecretPatch[] {
  const patchesByPath = new Map<string, ConsoleSecretPatch>();
  const activePatchesByPath = new Map(
    activeSecretPatches.map((patch) => [patch.path, patch] as const),
  );

  for (const secret of activeSecrets) {
    const activePatch = activePatchesByPath.get(secret.path);
    const credentialMatch = /^\/providers\/(\d+)\/credentials\/(\d+)(\/.*)$/.exec(secret.path);
    if (credentialMatch) {
      const activeProvider = activeDocument.providers[Number(credentialMatch[1])];
      if (!isRecord(activeProvider)) {
        continue;
      }
      const providerId = requiredId(activeProvider.id, "provider.id");
      const activeCredentials = activeProvider.credentials;
      if (!Array.isArray(activeCredentials)) {
        continue;
      }
      const activeCredential = activeCredentials[Number(credentialMatch[2])];
      if (!isRecord(activeCredential)) {
        continue;
      }
      const credentialId = requiredId(activeCredential.id, "credential.id");
      const draftProviderIndex = providerIndexById(draftDocument, providerId);
      if (draftProviderIndex < 0) {
        continue;
      }
      const draftProvider = draftDocument.providers[draftProviderIndex];
      if (!isRecord(draftProvider)) {
        continue;
      }
      const draftCredentialIndex = credentialIndexById(draftProvider, credentialId);
      if (draftCredentialIndex < 0) {
        continue;
      }
      const path = `/providers/${draftProviderIndex}/credentials/${draftCredentialIndex}${credentialMatch[3]}`;
      patchesByPath.set(path, patchAtPath(activePatch, path));
      continue;
    }

    const providerMatch = /^\/providers\/(\d+)(\/.*)$/.exec(secret.path);
    if (!providerMatch) {
      patchesByPath.set(secret.path, patchAtPath(activePatch, secret.path));
      continue;
    }
    const activeProvider = activeDocument.providers[Number(providerMatch[1])];
    if (!isRecord(activeProvider)) {
      continue;
    }
    const providerId = requiredId(activeProvider.id, "provider.id");
    const draftProviderIndex = providerIndexById(draftDocument, providerId);
    if (draftProviderIndex < 0) {
      continue;
    }
    const path = `/providers/${draftProviderIndex}${providerMatch[2]}`;
    patchesByPath.set(path, patchAtPath(activePatch, path));
  }

  for (const edit of credentialSecretEdits) {
    const providerId = requiredId(edit.providerId, "providerId");
    const credentialId = requiredId(edit.credentialId, "credentialId");
    const draftProviderIndex = providerIndexById(draftDocument, providerId);
    if (draftProviderIndex < 0) {
      throw new Error(`Provider '${providerId}' was not found.`);
    }
    const draftProvider = draftDocument.providers[draftProviderIndex];
    if (!isRecord(draftProvider)) {
      throw new Error(`Provider '${providerId}' must be an object.`);
    }
    const draftCredentialIndex = credentialIndexById(draftProvider, credentialId);
    if (draftCredentialIndex < 0) {
      throw new Error(
        `Credential '${credentialId}' was not found under provider '${providerId}'.`,
      );
    }
    const path = `/providers/${draftProviderIndex}/credentials/${draftCredentialIndex}/${edit.field}`;
    if (edit.operation === "replace") {
      patchesByPath.set(path, { path, operation: "replace", value: edit.value });
    } else {
      patchesByPath.set(path, { path, operation: "clear" });
    }
  }

  return [...patchesByPath.values()].sort((left, right) => left.path.localeCompare(right.path));
}
