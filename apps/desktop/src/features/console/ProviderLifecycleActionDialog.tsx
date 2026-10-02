import { useRef } from "react";
import * as Dialog from "@radix-ui/react-dialog";

export type PendingProviderLifecycleAction = {
  kind: "prune" | "enable-permanent-delete" | "purge-archive";
  providerId: string;
  providerLabel: string;
  permanentDeleteEnabled: boolean;
  archivedCredentialCount: number;
};

type ProviderLifecycleActionDialogProps = {
  pending: PendingProviderLifecycleAction | null;
  onCancel: () => void;
  onConfirm: () => void;
  t: (zh: string, en: string) => string;
};

export function ProviderLifecycleActionDialog({
  pending: pendingProviderLifecycleAction,
  onCancel,
  onConfirm: confirmProviderLifecycleAction,
  t,
}: ProviderLifecycleActionDialogProps) {
  const returnFocus = useRef<HTMLElement | null>(null);
  return (
    <Dialog.Root
      open={pendingProviderLifecycleAction !== null}
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
          aria-describedby="provider-lifecycle-action-description"
          onOpenAutoFocus={() => {
            returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            const trigger = returnFocus.current;
            if (!trigger?.isConnected || trigger.closest("[inert]")) return;
            if (trigger instanceof HTMLButtonElement && trigger.disabled) {
              trigger.closest("article")?.querySelector<HTMLButtonElement>(".nt-provider-card__back .nt-provider-card__flip")?.focus();
            } else trigger.focus();
          }}
        >
          <Dialog.Title>
            {pendingProviderLifecycleAction?.kind === "enable-permanent-delete"
              ? t("开启彻底删除模式", "Enable permanent deletion")
              : pendingProviderLifecycleAction?.kind === "purge-archive"
                ? t("确认清空账号归档", "Confirm archive purge")
                : pendingProviderLifecycleAction?.permanentDeleteEnabled
                  ? t("确认彻底删除失效号", "Confirm permanent invalid credential deletion")
                  : t("确认归档失效号", "Confirm invalid credential archival")}
          </Dialog.Title>
          <Dialog.Description id="provider-lifecycle-action-description">
            {pendingProviderLifecycleAction?.kind === "enable-permanent-delete"
              ? t(
                  `开启后，${pendingProviderLifecycleAction.providerLabel} 的自动和手动失效号删除将跳过归档。保存当前草稿后生效。`,
                  `Once enabled, automatic and manual invalid credential deletion for ${pendingProviderLifecycleAction.providerLabel} will skip archival. Save the current draft to apply it.`,
                )
              : pendingProviderLifecycleAction?.kind === "purge-archive"
                ? t(
                    `将彻底删除 ${pendingProviderLifecycleAction.providerLabel} 归档目录内的 ${pendingProviderLifecycleAction.archivedCredentialCount} 个账号。`,
                    `This permanently deletes ${pendingProviderLifecycleAction.archivedCredentialCount} archived credentials for ${pendingProviderLifecycleAction.providerLabel}.`,
                  )
                : pendingProviderLifecycleAction?.permanentDeleteEnabled
                  ? t(
                      `将调用受信任驱动识别 ${pendingProviderLifecycleAction?.providerLabel} 的失效号，并直接彻底删除。`,
                      `The trusted driver will identify invalid credentials for ${pendingProviderLifecycleAction?.providerLabel} and permanently delete them.`,
                    )
                  : t(
                      `将调用受信任驱动识别 ${pendingProviderLifecycleAction?.providerLabel} 的失效号，并将其移入账号归档目录。`,
                      `The trusted driver will identify invalid credentials for ${pendingProviderLifecycleAction?.providerLabel} and move them into the account archive.`,
                    )}
          </Dialog.Description>
          <div className="nt-validation-list nt-validation-list--warning">
            <strong>{t("请再次确认", "Please confirm")}</strong>
            <ul>
              <li>
                {pendingProviderLifecycleAction?.kind === "prune" &&
                !pendingProviderLifecycleAction.permanentDeleteEnabled
                  ? t("默认删除会保留独立归档记录。", "Default deletion retains an independent archive record.")
                  : t("此操作涉及不可恢复的数据删除。", "This action involves irreversible data deletion.")}
              </li>
            </ul>
          </div>
          <div className="dialog-actions">
            <Dialog.Close asChild>
              <button type="button">{t("取消", "Cancel")}</button>
            </Dialog.Close>
            <button
              className={
                pendingProviderLifecycleAction?.kind === "prune" &&
                !pendingProviderLifecycleAction.permanentDeleteEnabled
                  ? "nt-btn nt-btn--secondary"
                  : "nt-btn nt-btn--danger"
              }
              type="button"
              onClick={confirmProviderLifecycleAction}
            >
              {pendingProviderLifecycleAction?.kind === "enable-permanent-delete"
                ? t("确认开启", "Enable")
                : pendingProviderLifecycleAction?.kind === "purge-archive"
                  ? t("彻底删除归档", "Purge archive")
                  : pendingProviderLifecycleAction?.permanentDeleteEnabled
                    ? t("彻底删除失效号", "Delete permanently")
                    : t("归档失效号", "Archive invalid")}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
