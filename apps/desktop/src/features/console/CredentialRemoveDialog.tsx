import * as Dialog from "@radix-ui/react-dialog";

import type { PendingCredentialRemoval, TranslateFn } from "./accountCardTypes";

/** Keeps the destructive account action behind the same confirmation everywhere. */
export function CredentialRemoveDialog(props: {
  t: TranslateFn;
  pending: PendingCredentialRemoval | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { t, pending, onCancel, onConfirm } = props;
  return (
    <Dialog.Root
      open={pending !== null}
      onOpenChange={(open) => {
        if (!open) {
          onCancel();
        }
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-credential-remove-dialog"
          aria-describedby="credential-remove-description"
        >
          <Dialog.Title>{t("确认删除账号", "Confirm account deletion")}</Dialog.Title>
          <Dialog.Description id="credential-remove-description">
            {pending
              ? t(
                  `将从当前草稿中删除账号 ${pending.displayName}（${pending.accountId}）。保存草稿后该变更才会生效。`,
                  `This removes ${pending.displayName} (${pending.accountId}) from the current draft. The change takes effect after the draft is saved.`,
                )
              : ""}
          </Dialog.Description>
          <div className="nt-validation-list nt-validation-list--warning">
            <strong>{t("请再次确认", "Please confirm")}</strong>
            <ul>
              <li>{t("此操作不会删除其他账号。", "Other accounts are not affected.")}</li>
            </ul>
          </div>
          <div className="dialog-actions">
            <Dialog.Close asChild>
              <button type="button">{t("取消", "Cancel")}</button>
            </Dialog.Close>
            <button className="nt-btn nt-btn--danger" type="button" onClick={onConfirm}>
              {t("确认删除", "Delete account")}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
