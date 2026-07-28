import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent, useEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type SecretConfirmDialogProps = {
  open: boolean;
  busy: boolean;
  error?: string | null;
  onOpenChange(open: boolean): void;
  onConfirm(token: string): Promise<void>;
};

export function SecretConfirmDialog({
  open,
  busy,
  error,
  onOpenChange,
  onConfirm,
}: SecretConfirmDialogProps) {
  const [token, setToken] = useState("");
  const { t } = useUiLocale();

  useEffect(() => {
    if (!open) {
      setToken("");
    }
  }, [open]);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onConfirm(token);
      onOpenChange(false);
    } catch {
      // The session provider exposes the server-safe error message.
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content" aria-describedby="secret-confirm-description">
          <Dialog.Title>{t("确认敏感信息访问权限", "Confirm secret access")}</Dialog.Title>
          <Dialog.Description id="secret-confirm-description">
            {t(
              "重新输入管理密钥，以获取一个短时有效的敏感信息访问授权。",
              "Re-enter the management token to receive a short-lived secret grant.",
            )}
          </Dialog.Description>
          <form onSubmit={(event) => void submit(event)}>
            <label htmlFor="secret-confirm-token">{t("管理密钥", "Management token")}</label>
            <input
              id="secret-confirm-token"
              type="password"
              autoComplete="current-password"
              value={token}
              onChange={(event) => setToken(event.currentTarget.value)}
              required
              autoFocus
            />
            {error && <p role="alert">{error}</p>}
            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button" disabled={busy}>{t("取消", "Cancel")}</button>
              </Dialog.Close>
              <button type="submit" disabled={busy}>
                {busy ? t("确认中...", "Confirming...") : t("确认", "Confirm")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
