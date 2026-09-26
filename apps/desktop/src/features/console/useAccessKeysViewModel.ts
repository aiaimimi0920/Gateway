import { useMemo } from "react";

import type { ConsoleAccessBundle, ConsoleAccessCatalog, ConsoleAccessKey } from "../../api/contracts";
import type { AccessKeyDraft, AccessPanelState } from "./accessKeysTypes";

export function useAccessKeysViewModel(
  catalog: AccessPanelState<ConsoleAccessCatalog>,
  keyDraft: AccessKeyDraft,
  bundleDraft: { slug: string; displayName: string },
  affinityDraft: { accessKeyId: string; model: string },
) {
  const bundles: ConsoleAccessBundle[] = catalog.data?.bundles ?? [];
  const sortedKeys = useMemo<ConsoleAccessKey[]>(() => {
    const rows = catalog.data?.accessKeys ?? [];
    return [...rows].sort((left, right) => {
      const byState = Number(Boolean(left.revokedAt)) - Number(Boolean(right.revokedAt));
      return byState !== 0 ? byState : right.createdAt.localeCompare(left.createdAt);
    });
  }, [catalog.data]);

  const bundleNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const bundle of bundles) {
      map.set(bundle.id, bundle.displayName || bundle.slug);
    }
    return map;
  }, [bundles]);

  const bundlesByKey = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const binding of catalog.data?.keyBundleBindings ?? []) {
      const label = bundleNameById.get(binding.bundleId) ?? binding.bundleId;
      const existing = map.get(binding.accessKeyId);
      if (existing) {
        existing.push(label);
      } else {
        map.set(binding.accessKeyId, [label]);
      }
    }
    return map;
  }, [bundleNameById, catalog.data]);

  const itemCountByBundle = useMemo(() => {
    const map = new Map<string, number>();
    for (const item of catalog.data?.bundleItems ?? []) {
      map.set(item.bundleId, (map.get(item.bundleId) ?? 0) + 1);
    }
    return map;
  }, [catalog.data]);

  return {
    affinityFormIncomplete:
      affinityDraft.accessKeyId.trim().length === 0 || affinityDraft.model.trim().length === 0,
    activeKeyCount: sortedKeys.filter((key) => !key.revokedAt).length,
    bundleFormIncomplete:
      bundleDraft.slug.trim().length === 0 || bundleDraft.displayName.trim().length === 0,
    bundles,
    bundlesByKey,
    catalogEmpty: catalog.data === null,
    itemCountByBundle,
    keyFormIncomplete:
      keyDraft.ownerId.trim().length === 0 ||
      keyDraft.resolvedProjectId.trim().length === 0 ||
      keyDraft.resolvedTenantId.trim().length === 0 ||
      keyDraft.displayName.trim().length === 0,
    sortedKeys,
  };
}
