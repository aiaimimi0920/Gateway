import { type FormEvent, useEffect, useId, useMemo, useRef, useState } from "react";

import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { accountGroupCreateValidation, type AccountGroupCreateValue } from "./accountGroupDraft";
import { buildGroupMemberCandidates } from "./accountManagementViewModel";
import { CredentialGroupEditDialog } from "./CredentialGroupEditDialog";

type AccountGroupDialogProps = {
  open: boolean;
  locked: boolean;
  existingGroupIds: readonly string[];
  accounts?: Parameters<typeof buildGroupMemberCandidates>[0];
  onOpenChange(open: boolean): void;
  onSubmit(value: AccountGroupCreateValue): boolean;
};

const EMPTY_VALUE: AccountGroupCreateValue = {
  groupId: "", name: "", description: "", billingMultiplier: "1", enabled: true, notes: "",
};
const EMPTY_ACCOUNTS: NonNullable<AccountGroupDialogProps["accounts"]> = [];

// Creation uses the same editor as editing, but keeps all changes local until confirmation.
export function AccountGroupDialog({
  open, locked, existingGroupIds, accounts = EMPTY_ACCOUNTS, onOpenChange, onSubmit,
}: AccountGroupDialogProps) {
  const { t } = useUiLocale();
  const [value, setValue] = useState(EMPTY_VALUE);
  const [memberIds, setMemberIds] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"all" | "members" | "ungrouped">("all");
  const [attempted, setAttempted] = useState(false);
  const [submissionFailed, setSubmissionFailed] = useState(false);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const draftId = useId();
  const candidates = useMemo(() => buildGroupMemberCandidates(accounts, {
    selectedCredentialIds: memberIds, query, mode,
  }), [accounts, memberIds, query, mode]);
  const validation = attempted ? accountGroupCreateValidation(value, existingGroupIds) : null;
  const idInvalid = validation === "groupId" || validation === "duplicate";
  const error = validation === "groupId"
    ? t("分组 ID 必须填写。", "Group ID is required.")
    : validation === "duplicate"
      ? t("分组 ID 已存在，请使用其他 ID。", "Group ID already exists; use a different ID.")
      : validation === "billingMultiplier"
        ? t("分组计费倍率必须是大于等于 0 的数字。", "Group billing multiplier must be a number greater than or equal to 0.")
        : submissionFailed ? t("未能创建分组，请检查当前配置后重试。", "Unable to create the group; check the current configuration and retry.") : null;

  useEffect(() => {
    if (open) {
      setValue(EMPTY_VALUE);
      setMemberIds([]);
      setQuery("");
      setMode("all");
      setAttempted(false);
      setSubmissionFailed(false);
    }
  }, [open]);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (locked) return;
    setAttempted(true);
    if (accountGroupCreateValidation(value, existingGroupIds)) return;
    // Drop candidates removed by a concurrent catalog refresh before committing the draft.
    const availableIds = new Set(accounts.map((account) => account.id));
    const providerCredentialIds = memberIds.filter((id) => availableIds.has(id));
    if (onSubmit({
      ...value, groupId: value.groupId.trim(), name: value.name.trim(),
      description: value.description.trim(), notes: value.notes.trim(),
      ...(providerCredentialIds.length ? { providerCredentialIds } : {}),
    })) onOpenChange(false);
    else setSubmissionFailed(true);
  };

  return <CredentialGroupEditDialog
    open={open} t={t} notice={null} error={error} editorLocked={locked}
    group={{ ...value, id: draftId, providerCredentialIds: memberIds }}
    selectedGroupIdInvalid={idInvalid} selectedGroupBillingInvalid={validation === "billingMultiplier"}
    groupIdError={idInvalid ? error ?? undefined : undefined}
    memberCandidates={candidates} memberQuery={query} memberMode={mode}
    onMemberQueryChange={setQuery} onMemberModeChange={setMode}
    onToggleMember={(_, id) => {
      if (!locked) setMemberIds((current) => current.includes(id)
        ? current.filter((memberId) => memberId !== id) : [...current, id]);
    }}
    onUpdateField={(_, field, next) => { if (!locked) setValue((current) => ({ ...current, [field]: next })); }}
    onToggleEnabled={(_, enabled) => { if (!locked) setValue((current) => ({ ...current, enabled })); }}
    onRemoveGroup={() => {}}
    onCaptureFocus={() => { returnFocusRef.current = document.activeElement as HTMLElement; }}
    onRestoreFocus={() => { if (returnFocusRef.current?.isConnected) returnFocusRef.current.focus(); }}
    onClose={() => onOpenChange(false)} onCreate={submit}
  />;
}