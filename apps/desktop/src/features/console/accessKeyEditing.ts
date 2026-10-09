import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
} from "../../api/contracts";
import { keyGroupIds, keyGroupUpdate } from "./accessKeyGroups";
import { keyQuotaDraft, keyQuotaInput } from "./accessKeyQuota";
import type { AccessKeyDraft } from "./accessKeysTypes";

export function editAccessKeyDraft(
  key: ConsoleAccessKey,
  catalog: ConsoleAccessCatalog,
): AccessKeyDraft {
  return {
    ownerType: key.ownerType,
    ownerId: key.ownerId,
    resolvedProjectId: key.resolvedProjectId,
    resolvedTenantId: key.resolvedTenantId,
    keyKind: key.keyKind,
    publicKeyPrefix: key.publicKeyPrefix,
    displayName: key.displayName,
    expiresAt: key.expiresAt ?? "",
    accountGroupIds: keyGroupIds(key) ?? [],
    bundleIds: catalog.keyBundleBindings
      .filter((binding) => binding.accessKeyId === key.id)
      .map((binding) => binding.bundleId),
    ...keyQuotaDraft(
      catalog.balances.find((balance) => balance.accessKeyId === key.id),
    ),
  };
}

export function editedAccessKeyInput(
  key: ConsoleAccessKey,
  draft: AccessKeyDraft,
  catalog: ConsoleAccessCatalog,
) {
  const original = keyQuotaDraft(
    catalog.balances.find((balance) => balance.accessKeyId === key.id),
  );
  return {
    ...keyGroupUpdate(key, draft.accountGroupIds, catalog),
    displayName: draft.displayName.trim(),
    expiresAt: draft.expiresAt || null,
    ownerType: draft.ownerType.trim(),
    ownerId: draft.ownerId.trim(),
    bundleIds: draft.bundleIds,
    // Name/group edits must not reset consumed credit or a concurrent reservation.
    quota:
      original.quotaMode === draft.quotaMode &&
      original.quotaLimit === draft.quotaLimit
        ? undefined
        : keyQuotaInput(draft),
  };
}
