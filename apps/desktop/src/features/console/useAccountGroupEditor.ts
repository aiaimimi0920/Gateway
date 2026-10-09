import { useCallback, useState, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import { parseRouteDocument } from "./routeDocument";
import { type AccountGroupDraftRow, type AccountGroupCreateValue, accountGroupCreateValidation, createAccountGroupDraftRow, accountGroupDraftHasInput, accountGroupDraftNeedsId, parseAccountGroupBillingMultiplier } from "./accountGroupDraft";
import { ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH, ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN, ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH, ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN } from "./useConsoleRouteDraft";

const ACCOUNT_GROUP_EDITOR_JSON_ERROR =
  "Fix Route document JSON before using the structured account-group editor.";

type AccountGroupEditorOptions = {
  editorText: string;
  accountGroupDraftRows: AccountGroupDraftRow[];
  setAccountGroupDraftRows: Dispatch<SetStateAction<AccountGroupDraftRow[]>>;
  setError: Dispatch<SetStateAction<string | null>>;
  replaceEditorDocument: (document: ConsoleRouteDocument, syncStructuredEditors?: boolean) => void;
  setSelectedAccountGroupRowId: Dispatch<SetStateAction<string | null>>;
  setGroupMemberQuery: Dispatch<SetStateAction<string>>;
  setGroupMemberMode: Dispatch<SetStateAction<"all" | "members" | "ungrouped">>;
  t: (zh: string, en: string) => string;
};

export function useAccountGroupEditor({
  editorText,
  accountGroupDraftRows,
  setAccountGroupDraftRows,
  setError,
  replaceEditorDocument,
  setSelectedAccountGroupRowId,
  setGroupMemberQuery,
  setGroupMemberMode,
  t,
}: AccountGroupEditorOptions) {
  const [accountGroupDialogOpen, setAccountGroupDialogOpen] = useState(false);
  const applyAccountGroupDraftRows = useCallback(
    (nextRows: AccountGroupDraftRow[]) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(ACCOUNT_GROUP_EDITOR_JSON_ERROR);
        return false;
      }
      setError((current) =>
        current === ACCOUNT_GROUP_EDITOR_JSON_ERROR ||
        current === ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH ||
        current === ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN ||
        current === ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH ||
        current === ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN
          ? null
          : current,
      );

      const nextGroups = [];
      for (const row of nextRows) {
        if (!accountGroupDraftHasInput(row)) {
          continue;
        }
        if (accountGroupDraftNeedsId(row)) {
          setAccountGroupDraftRows(nextRows);
          setError(t(ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH, ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN));
          return false;
        }
        const billingMultiplier = parseAccountGroupBillingMultiplier(row.billingMultiplier);
        if (billingMultiplier === null) {
          setAccountGroupDraftRows(nextRows);
          setError(
            t(
              ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH,
              ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN,
            ),
          );
          return false;
        }

        nextGroups.push({
          id: row.groupId.trim(),
          name: row.name.trim(),
          billing_multiplier: billingMultiplier,
          enabled: row.enabled,
          ...(row.description.trim().length > 0 ? { description: row.description.trim() } : {}),
          ...(row.notes.trim().length > 0 ? { notes: row.notes.trim() } : {}),
          provider_credential_ids: [...new Set(row.providerCredentialIds)],
        });
      }

      if (nextGroups.length > 0) {
        (document as Record<string, unknown>).account_groups = nextGroups;
      } else {
        delete (document as Record<string, unknown>).account_groups;
      }
      setAccountGroupDraftRows(nextRows);
      replaceEditorDocument(document);
      return true;
    },
    [editorText, replaceEditorDocument, t],
  );

  const addAccountGroupRow = useCallback(() => setAccountGroupDialogOpen(true), []);

  // A cancelled form never enters the route document or starts autosave.
  const submitAccountGroup = useCallback((value: AccountGroupCreateValue) => {
    if (accountGroupCreateValidation(value, accountGroupDraftRows.map((row) => row.groupId))) {
      return false;
    }
    const nextRow = createAccountGroupDraftRow(value);
    if (!applyAccountGroupDraftRows([...accountGroupDraftRows, nextRow])) return false;
    setSelectedAccountGroupRowId(nextRow.id);
    setGroupMemberQuery("");
    setGroupMemberMode("all");
    return true;
  }, [accountGroupDraftRows, applyAccountGroupDraftRows]);

  const updateAccountGroupRow = useCallback(
    (
      rowId: string,
      field: "groupId" | "name" | "description" | "billingMultiplier" | "notes",
      value: string,
    ) => {
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => (row.id === rowId ? { ...row, [field]: value } : row)),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const updateAccountGroupEnabled = useCallback(
    (rowId: string, enabled: boolean) => {
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => (row.id === rowId ? { ...row, enabled } : row)),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const toggleAccountGroupMember = useCallback(
    (rowId: string, providerCredentialId: string) => {
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => {
          if (row.id !== rowId) {
            return row;
          }
          const exists = row.providerCredentialIds.includes(providerCredentialId);
          return {
            ...row,
            providerCredentialIds: exists
              ? row.providerCredentialIds.filter((value) => value !== providerCredentialId)
              : [...row.providerCredentialIds, providerCredentialId],
          };
        }),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const setAccountRoutingGroup = useCallback(
    (providerCredentialId: string, groupId: string) => {
      const normalizedGroupId = groupId.trim();
      if (
        normalizedGroupId.length > 0 &&
        !accountGroupDraftRows.some((row) => row.groupId.trim() === normalizedGroupId)
      ) {
        setError(t("所选分组池不存在。", "The selected routing pool does not exist."));
        return;
      }
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => {
          const withoutAccount = row.providerCredentialIds.filter(
            (value) => value !== providerCredentialId,
          );
          return row.groupId.trim() === normalizedGroupId
            ? { ...row, providerCredentialIds: [...withoutAccount, providerCredentialId] }
            : { ...row, providerCredentialIds: withoutAccount };
        }),
      );
      pushAppToast(
        "info",
        normalizedGroupId.length > 0
          ? t(
              `账号已绑定到分组池 ${normalizedGroupId}，保存路由配置后生效。`,
              `The account is assigned to ${normalizedGroupId}; save the route config to apply it.`,
            )
          : t(
              "账号已设为未分组，保存路由配置后生效。",
              "The account is now ungrouped; save the route config to apply it.",
            ),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows, t],
  );

  const removeAccountGroupRow = useCallback(
    (rowId: string) => {
      const nextRows = accountGroupDraftRows.filter((row) => row.id !== rowId);
      applyAccountGroupDraftRows(nextRows);
      setSelectedAccountGroupRowId((current) =>
        current === rowId ? nextRows[0]?.id ?? null : current,
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  return { accountGroupDialogOpen, setAccountGroupDialogOpen, submitAccountGroup, addAccountGroupRow, updateAccountGroupRow, updateAccountGroupEnabled, toggleAccountGroupMember, setAccountRoutingGroup, removeAccountGroupRow };
}
