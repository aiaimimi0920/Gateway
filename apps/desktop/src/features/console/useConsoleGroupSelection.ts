import { useEffect, useMemo, useState } from "react";
import { buildCredentialGroupDirectory, buildGroupMemberCandidates } from "./accountManagementViewModel";
import { accountGroupDraftNeedsId, parseAccountGroupBillingMultiplier } from "./accountGroupDraft";

type ConsoleGroupSelectionOptions = {
  displayedAccountCatalog: Parameters<typeof buildCredentialGroupDirectory>[0];
  accountGroupDraftRows: Parameters<typeof buildCredentialGroupDirectory>[1];
  accountCardMetricsById: Parameters<typeof buildCredentialGroupDirectory>[2];
  providerMetricsResolver: Parameters<typeof buildCredentialGroupDirectory>[3];
};

export function useConsoleGroupSelection({
  displayedAccountCatalog,
  accountGroupDraftRows,
  accountCardMetricsById,
  providerMetricsResolver,
}: ConsoleGroupSelectionOptions) {
  const [selectedAccountGroupRowId, setSelectedAccountGroupRowId] = useState<string | null>(null);

  const [groupMemberQuery, setGroupMemberQuery] = useState("");

  const [groupMemberMode, setGroupMemberMode] = useState<"all" | "members" | "ungrouped">("all");

  const groupDirectory = useMemo(
    () =>
      buildCredentialGroupDirectory(
        displayedAccountCatalog,
        accountGroupDraftRows,
        accountCardMetricsById,
        providerMetricsResolver,
      ),
    [
      accountCardMetricsById,
      accountGroupDraftRows,
      displayedAccountCatalog,
      providerMetricsResolver,
    ],
  );

  const effectiveSelectedAccountGroupRowId = selectedAccountGroupRowId ?? groupDirectory[0]?.rowId ?? null;

  const selectedAccountGroupDraft = useMemo(
    () =>
      accountGroupDraftRows.find((row) => row.id === effectiveSelectedAccountGroupRowId) ?? null,
    [accountGroupDraftRows, effectiveSelectedAccountGroupRowId],
  );

  const selectedGroupMemberCandidates = useMemo(
    () =>
      buildGroupMemberCandidates(displayedAccountCatalog.accounts, {
        selectedCredentialIds: selectedAccountGroupDraft?.providerCredentialIds ?? [],
        query: groupMemberQuery,
        mode: groupMemberMode,
      }),
    [displayedAccountCatalog.accounts, groupMemberMode, groupMemberQuery, selectedAccountGroupDraft],
  );

  const selectedGroupMembers = useMemo(
    () =>
      buildGroupMemberCandidates(displayedAccountCatalog.accounts, {
        selectedCredentialIds: selectedAccountGroupDraft?.providerCredentialIds ?? [],
        query: "",
        mode: "members",
      }),
    [displayedAccountCatalog.accounts, selectedAccountGroupDraft],
  );

  const selectedGroupIdInvalid = selectedAccountGroupDraft
    ? accountGroupDraftNeedsId(selectedAccountGroupDraft)
    : false;

  const selectedGroupBillingInvalid = selectedAccountGroupDraft
    ? parseAccountGroupBillingMultiplier(selectedAccountGroupDraft.billingMultiplier) === null
    : false;

  useEffect(() => {
    if (groupDirectory.length === 0) {
      if (selectedAccountGroupRowId !== null) {
        setSelectedAccountGroupRowId(null);
      }
      return;
    }
    if (
      selectedAccountGroupRowId === null ||
      !groupDirectory.some((group) => group.rowId === selectedAccountGroupRowId)
    ) {
      setSelectedAccountGroupRowId(groupDirectory[0]?.rowId ?? null);
    }
  }, [groupDirectory, selectedAccountGroupRowId]);

  return {
    groupDirectory,
    effectiveSelectedAccountGroupRowId,
    selectedAccountGroupDraft,
    selectedGroupMemberCandidates,
    selectedGroupMembers,
    selectedGroupIdInvalid,
    selectedGroupBillingInvalid,
    setSelectedAccountGroupRowId,
    groupMemberQuery,
    setGroupMemberQuery,
    groupMemberMode,
    setGroupMemberMode,
  };
}

