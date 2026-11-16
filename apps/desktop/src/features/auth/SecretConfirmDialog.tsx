import { Button, Dialog, Field, Input, Notice } from "@neuro/ui";
import { type FormEvent, useEffect, useRef, useState } from "react";
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
  const tokenRef = useRef<HTMLInputElement>(null);
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
    <Dialog open={open} onOpenChange={onOpenChange} busy={busy} initialFocusRef={tokenRef}
      title={t("确认敏感信息访问权限", "Confirm secret access")} closeLabel={t("关闭", "Close")}
      className="gateway-secret-dialog" description={t(
        "重新输入管理密钥，以获取一个短时有效的敏感信息访问授权。",
        "Re-enter the management token to receive a short-lived secret grant.",
      )}>
      <form className="gateway-secret-form" onSubmit={(event) => void submit(event)}>
        <div className="gateway-secret-body">
          <Field label={t("管理密钥", "Management token")}><Input
            ref={tokenRef}
            id="secret-confirm-token"
            type="password"
            autoComplete="current-password"
            value={token}
            onChange={(event) => setToken(event.currentTarget.value)}
            required
            disabled={busy}
          /></Field>
          {error && <Notice tone="danger">{error}</Notice>}
        </div>
        <div className="n-dialog-actions">
          <Button disabled={busy} onClick={() => onOpenChange(false)}>{t("取消", "Cancel")}</Button>
          <Button variant="primary" type="submit" loading={busy}>{t("确认", "Confirm")}</Button>
        </div>
      </form>
    </Dialog>
  );
}
