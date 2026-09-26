import type { CredentialGroupDirectoryItem } from "./accountManagementViewModel";
import { buildProviderAvailabilityCells } from "./providerCardMetrics";

type TranslateFn = (zh: string, en: string) => string;

/** Derived card data that only changes with the group directory or provider scope. */
export type CredentialGroupCardSnapshot = {
  group: CredentialGroupDirectoryItem;
  label: string;
  accountPanelId: string;
  cardMenuKey: string;
  deselectedProviderIds: string[];
  selectedProviderIds: ReadonlySet<string>;
  providerScopeNarrowed: boolean;
  successWindows: NonNullable<CredentialGroupDirectoryItem["metrics"]>["successWindows"];
  availabilityCells: ReturnType<typeof buildProviderAvailabilityCells>;
};

export function buildCredentialGroupCardSnapshot(
  group: CredentialGroupDirectoryItem,
  t: TranslateFn,
  deselectedProviderIds: string[] = [],
): CredentialGroupCardSnapshot {
  const deselectedProviderIdSet = new Set(deselectedProviderIds);
  const selectedProviderIds = new Set(
    group.providerScopes
      .map((scope) => scope.providerId)
      .filter((providerId) => !deselectedProviderIdSet.has(providerId)),
  );
  const successWindows = group.metrics?.successWindows ?? [];

  return {
    group,
    label: group.name || group.groupId || t("新分组", "New group"),
    accountPanelId: `entitlement-group-accounts-${group.rowId.replace(/[^a-zA-Z0-9_-]/g, "-")}`,
    cardMenuKey: `entitlement-group-card:${group.rowId}`,
    deselectedProviderIds,
    selectedProviderIds,
    providerScopeNarrowed: deselectedProviderIds.length > 0,
    successWindows,
    availabilityCells: buildProviderAvailabilityCells(successWindows),
  };
}
