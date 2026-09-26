import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { routeAccountGroupsFromDocument } from "./routeAccountCatalog";

export type AccountGroupDraftRow = {
  id: string;
  groupId: string;
  name: string;
  description: string;
  billingMultiplier: string;
  enabled: boolean;
  notes: string;
  providerCredentialIds: string[];
};

export function createAccountGroupDraftRow(
  overrides: Partial<Omit<AccountGroupDraftRow, "id">> = {},
): AccountGroupDraftRow {
  return {
    id: `account-group-${Math.random().toString(36).slice(2, 10)}`,
    groupId: overrides.groupId ?? "",
    name: overrides.name ?? "",
    description: overrides.description ?? "",
    billingMultiplier: overrides.billingMultiplier ?? "1",
    enabled: overrides.enabled ?? true,
    notes: overrides.notes ?? "",
    providerCredentialIds: overrides.providerCredentialIds ?? [],
  };
}

export function accountGroupDraftRowsFromDocument(
  document: ConsoleRouteDocument,
): AccountGroupDraftRow[] {
  return routeAccountGroupsFromDocument(document).map((group) =>
    createAccountGroupDraftRow({
      groupId: group.id,
      name: group.name,
      description: group.description ?? "",
      billingMultiplier: String(group.billingMultiplier),
      enabled: group.enabled,
      notes: group.notes ?? "",
      providerCredentialIds: group.providerCredentialIds,
    }),
  );
}

export function parseAccountGroupBillingMultiplier(value: string): number | null {
  const normalized = value.trim();
  if (normalized.length === 0) {
    return 1;
  }
  if (!/^(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(normalized)) {
    return null;
  }
  const parsed = Number(normalized);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

export function accountGroupDraftHasInput(row: AccountGroupDraftRow): boolean {
  return (
    row.groupId.trim().length > 0 ||
    row.name.trim().length > 0 ||
    row.description.trim().length > 0 ||
    row.notes.trim().length > 0 ||
    row.providerCredentialIds.length > 0
  );
}

export function accountGroupDraftNeedsId(row: AccountGroupDraftRow): boolean {
  return accountGroupDraftHasInput(row) && row.groupId.trim().length === 0;
}

export function documentHasIncompleteAccountGroup(document: ConsoleRouteDocument): boolean {
  const groupsValue = (document as Record<string, unknown>).account_groups;
  if (!Array.isArray(groupsValue)) {
    return false;
  }
  return groupsValue.some((entry) => {
    if (!isRecord(entry)) {
      return false;
    }
    const groupId = typeof entry.id === "string" ? entry.id.trim() : "";
    const hasInput =
      groupId.length > 0 ||
      (typeof entry.name === "string" && entry.name.trim().length > 0) ||
      (typeof entry.description === "string" && entry.description.trim().length > 0) ||
      (typeof entry.notes === "string" && entry.notes.trim().length > 0) ||
      (Array.isArray(entry.provider_credential_ids) && entry.provider_credential_ids.length > 0);
    return hasInput && groupId.length === 0;
  });
}

export function documentHasInvalidAccountGroupBillingMultiplier(
  document: ConsoleRouteDocument,
): boolean {
  const groupsValue = (document as Record<string, unknown>).account_groups;
  if (!Array.isArray(groupsValue)) {
    return false;
  }
  return groupsValue.some(
    (entry) =>
      isRecord(entry) &&
      "billing_multiplier" in entry &&
      (typeof entry.billing_multiplier !== "number" ||
        !Number.isFinite(entry.billing_multiplier) ||
        entry.billing_multiplier < 0),
  );
}
