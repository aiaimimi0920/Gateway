import { ConfirmDialog } from "@neuro/ui";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";

export function ProviderRemoveDialog({ pending, locked, onCancel, onConfirm, t }: {
  pending: AccountsLedgerPilotSection | null;
  locked: boolean;
  onCancel(): void;
  onConfirm(): void;
  t(zh: string, en: string): string;
}) {
  return (
    <ConfirmDialog open={pending !== null} onOpenChange={open => { if (!open) onCancel(); }}
      title={t("确认删除凭据池", "Confirm pool deletion")}
      description={t(`删除凭据池 ${pending?.providerLabel ?? ""} 及其全部账号和模型映射？关联路由与账号组引用也将移除，配置会自动保存。`,
        `Delete pool ${pending?.providerLabel ?? ""}, all its accounts and model mappings? Its route and group references will also be removed and the configuration saved automatically.`)}
      confirmLabel={t("确认删除凭据池", "Delete pool")} cancelLabel={t("取消", "Cancel")}
      confirmDisabled={locked} dismissOnOutside onConfirm={onConfirm}>
      <p>{t("不会删除磁盘上的凭据文件、归档或历史调用记录。", "Credential files, archives and historical request records are not deleted.")}</p>
    </ConfirmDialog>
  );
}
