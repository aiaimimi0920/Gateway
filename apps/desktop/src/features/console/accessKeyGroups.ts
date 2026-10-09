import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
  ConsoleAccessKeyGroup,
  ConsoleAccessKeyInput,
} from "../../api/contracts";

export function keyGroupIds(key: ConsoleAccessKey): string[] | null {
  const ids = key.metadata?.accountGroupIds;
  if (ids === undefined) return null;
  return Array.isArray(ids)
    ? ids.filter((id): id is string => typeof id === "string")
    : [];
}

export function validKeyGroups(
  ids: string[],
  groups: ConsoleAccessKeyGroup[] | undefined,
): boolean {
  return (
    ids.length > 0 &&
    ids.length <= 128 &&
    ids.every((id) => groups?.some((group) => group.id === id && group.enabled))
  );
}

export function keyGroupUpdate(
  key: ConsoleAccessKey,
  ids: string[],
  catalog: ConsoleAccessCatalog,
): ConsoleAccessKeyInput {
  // A group edit must preserve existing endpoint/model/provider restrictions and server bundle bindings.
  return {
    ownerType: key.ownerType,
    ownerId: key.ownerId,
    resolvedProjectId: key.resolvedProjectId,
    resolvedTenantId: key.resolvedTenantId,
    keyKind: key.keyKind,
    publicKeyPrefix: key.publicKeyPrefix,
    displayName: key.displayName,
    expiresAt: key.expiresAt,
    metadata: { ...key.metadata, accountGroupIds: ids },
    bundleIds: catalog.keyBundleBindings
      .filter((binding) => binding.accessKeyId === key.id)
      .map((binding) => binding.bundleId),
  };
}
