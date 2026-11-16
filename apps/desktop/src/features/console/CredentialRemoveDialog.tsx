import { ConfirmDialog, Notice } from "@neuro/ui";

import type { PendingCredentialRemoval, TranslateFn } from "./accountCardTypes";

/** Keeps the destructive account action behind the same confirmation everywhere. */
export function CredentialRemoveDialog(props: {
  t: TranslateFn;
  pending: PendingCredentialRemoval | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { t, pending, onCancel, onConfirm } = props;
  return <ConfirmDialog open={pending !== null} onOpenChange={open => { if (!open) onCancel(); }}
    title={t("确认删除账号", "Confirm account deletion")}
    description={pending ? t(
      `将从当前草稿中删除账号 ${pending.displayName}（${pending.accountId}）。保存草稿后该变更才会生效。`,
      `This removes ${pending.displayName} (${pending.accountId}) from the current draft. The change takes effect after the draft is saved.`,
    ) : ""}
    confirmLabel={t("确认删除", "Delete account")} cancelLabel={t("取消", "Cancel")}
    dismissOnOutside onConfirm={onConfirm}>
    <Notice tone="warning">{t("此操作不会删除其他账号。", "Other accounts are not affected.")}</Notice>
  </ConfirmDialog>;
}
