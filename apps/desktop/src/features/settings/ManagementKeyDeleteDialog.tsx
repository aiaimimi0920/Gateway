import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { useRef } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { ManagementKey } from "./managementKeysApi";

type Props = {
  entry: ManagementKey | null;
  pending: boolean;
  locked: boolean;
  error: string;
  onCancel: () => void;
  onConfirm: (key: ManagementKey) => void;
};

export function ManagementKeyDeleteDialog({ entry, pending, locked, error, onCancel, onConfirm }: Props) {
  const { t } = useUiLocale();
  const returnFocus = useRef<HTMLElement | null>(null);
  return <AlertDialog.Root open={entry !== null} onOpenChange={open => { if (!open && !pending) onCancel(); }}>
    <AlertDialog.Portal>
      <AlertDialog.Overlay className="dialog-overlay" />
      <AlertDialog.Content className="dialog-content nt-management-key-delete-dialog"
        onOpenAutoFocus={() => { returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null; }}
        onCloseAutoFocus={event => {
          event.preventDefault();
          if (returnFocus.current?.isConnected) returnFocus.current.focus();
        }}
        onEscapeKeyDown={event => { if (pending) event.preventDefault(); }}>
        <AlertDialog.Title>{t("确认删除管理密钥", "Confirm management key deletion")}</AlertDialog.Title>
        <AlertDialog.Description>
          {t(`确定删除管理密钥“${entry?.name ?? ""}”吗？删除后该密钥立即失效。`, `Delete management key “${entry?.name ?? ""}”? Access is revoked immediately.`)}
          {entry?.current && <> {t("删除当前密钥后将退出登录，请确认已保存另一个有效密钥。", "Deleting the current key signs you out. Keep another valid key.")}</>}
        </AlertDialog.Description>
        {error && <p role="alert">{error}</p>}
        <div className="dialog-actions">
          <AlertDialog.Cancel asChild><button type="button" className="nt-btn nt-btn--secondary" disabled={pending}>{t("取消", "Cancel")}</button></AlertDialog.Cancel>
          <button type="button" className="nt-btn nt-btn--danger" disabled={locked}
            onClick={() => { if (entry) onConfirm(entry); }}>{t("确认删除", "Confirm delete")}</button>
        </div>
      </AlertDialog.Content>
    </AlertDialog.Portal>
  </AlertDialog.Root>;
}
