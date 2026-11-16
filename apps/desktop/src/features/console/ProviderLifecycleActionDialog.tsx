import { ConfirmDialog, Notice } from "@neuro/ui";
import { resolveProviderCardDialogFocus } from "./providerCardDialogFocus";

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
  if (!pendingProviderLifecycleAction) return null;
  const danger = pendingProviderLifecycleAction.kind !== "prune" || pendingProviderLifecycleAction.permanentDeleteEnabled;
  return (
    <ConfirmDialog open
      onOpenChange={(open) => { if (!open) onCancel(); }}
      onConfirm={confirmProviderLifecycleAction}
      resolveReturnFocus={resolveProviderCardDialogFocus} dismissOnOutside danger={danger}
      title={pendingProviderLifecycleAction.kind === "enable-permanent-delete"
              ? t("开启彻底删除模式", "Enable permanent deletion")
              : pendingProviderLifecycleAction.kind === "purge-archive"
                ? t("确认清空账号归档", "Confirm archive purge")
                : pendingProviderLifecycleAction.permanentDeleteEnabled
                  ? t("确认彻底删除失效号", "Confirm permanent invalid credential deletion")
                  : t("确认归档失效号", "Confirm invalid credential archival")}
      description={pendingProviderLifecycleAction.kind === "enable-permanent-delete"
              ? t(
                  `开启后，${pendingProviderLifecycleAction.providerLabel} 的自动清理和旧版直接清理接口将跳过归档；预览清理始终要求归档。保存当前草稿后生效。`,
                  `Automatic and legacy direct cleanup for ${pendingProviderLifecycleAction.providerLabel} will skip archival; reviewed cleanup always requires archival. Save the current draft to apply it.`,
                )
              : pendingProviderLifecycleAction.kind === "purge-archive"
                ? t(
                    `将彻底删除 ${pendingProviderLifecycleAction.providerLabel} 归档目录内的 ${pendingProviderLifecycleAction.archivedCredentialCount} 个账号。`,
                    `This permanently deletes ${pendingProviderLifecycleAction.archivedCredentialCount} archived credentials for ${pendingProviderLifecycleAction.providerLabel}.`,
                  )
                : pendingProviderLifecycleAction.permanentDeleteEnabled
                  ? t(
                      `将调用受信任驱动识别 ${pendingProviderLifecycleAction.providerLabel} 的失效号，并直接彻底删除。`,
                      `The trusted driver will identify invalid credentials for ${pendingProviderLifecycleAction.providerLabel} and permanently delete them.`,
                    )
                  : t(
                      `将调用受信任驱动识别 ${pendingProviderLifecycleAction.providerLabel} 的失效号，并将其移入账号归档目录。`,
                      `The trusted driver will identify invalid credentials for ${pendingProviderLifecycleAction.providerLabel} and move them into the account archive.`,
                    )}
      cancelLabel={t("取消", "Cancel")}
      confirmLabel={pendingProviderLifecycleAction.kind === "enable-permanent-delete"
        ? t("确认开启", "Enable")
        : pendingProviderLifecycleAction.kind === "purge-archive"
          ? t("彻底删除归档", "Purge archive")
          : pendingProviderLifecycleAction.permanentDeleteEnabled
            ? t("彻底删除失效号", "Delete permanently")
            : t("归档失效号", "Archive invalid")}>
          <Notice tone="warning" role="note">
            <strong>{t("请再次确认", "Please confirm")}</strong>
            <ul>
              {pendingProviderLifecycleAction.kind === "purge-archive" ? <li>{t(
                "仅移除当前归档条目；云服务的历史版本和保留策略不在本次清空范围内。",
                "Only current archive entries are removed; cloud historical versions and retention policies are outside this purge.",
              )}</li> : null}
              <li>
                {pendingProviderLifecycleAction.kind === "prune" &&
                !pendingProviderLifecycleAction.permanentDeleteEnabled
                  ? t("默认删除会保留独立归档记录。", "Default deletion retains an independent archive record.")
                  : t("此操作涉及不可恢复的数据删除。", "This action involves irreversible data deletion.")}
              </li>
            </ul>
          </Notice>
    </ConfirmDialog>
  );
}
