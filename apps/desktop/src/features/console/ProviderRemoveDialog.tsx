import * as Dialog from "@radix-ui/react-dialog";
import { useRef } from "react";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";

export function ProviderRemoveDialog({ pending, locked, onCancel, onConfirm, t }: {
  pending: AccountsLedgerPilotSection | null;
  locked: boolean;
  onCancel(): void;
  onConfirm(): void;
  t(zh: string, en: string): string;
}) {
  const returnFocus = useRef<HTMLElement | null>(null);
  return (
    <Dialog.Root open={pending !== null} onOpenChange={(open) => { if (!open) onCancel(); }}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content" aria-describedby="provider-remove-description"
          onOpenAutoFocus={() => {
            returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            const trigger = returnFocus.current;
            if (trigger?.isConnected && !trigger.closest("[inert]")) trigger.focus();
          }}>
          <Dialog.Title>{t("确认删除凭据池", "Confirm pool deletion")}</Dialog.Title>
          <Dialog.Description id="provider-remove-description">
            {t(`删除凭据池 ${pending?.providerLabel ?? ""} 及其全部账号和模型映射？关联路由与账号组引用也将移除，配置会自动保存。`,
              `Delete pool ${pending?.providerLabel ?? ""}, all its accounts and model mappings? Its route and group references will also be removed and the configuration saved automatically.`)}
          </Dialog.Description>
          <p className="nt-copy">{t("不会删除磁盘上的凭据文件、归档或历史调用记录。", "Credential files, archives and historical request records are not deleted.")}</p>
          <div className="dialog-actions">
            <Dialog.Close asChild><button type="button">{t("取消", "Cancel")}</button></Dialog.Close>
            <button className="nt-btn nt-btn--danger" type="button" disabled={locked} onClick={onConfirm}>
              {t("确认删除凭据池", "Delete pool")}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
