import { type FormEvent, useState } from "react";
import { Button, Field, Input, Notice, Panel } from "@neuro/ui";
import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type BootstrapPageProps = {
  busy: boolean;
  error?: string | null;
  onSubmit(token: string): Promise<void>;
};

export function BootstrapPage({ busy, error, onSubmit }: BootstrapPageProps) {
  const [token, setToken] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);
  const { t } = useUiLocale();

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (token.trim() !== confirmation.trim()) {
      setLocalError(t("两次输入的管理密钥不一致。", "The management token confirmation does not match."));
      return;
    }
    setLocalError(null);
    try {
      await onSubmit(token);
    } catch {
      // The session provider exposes the server-safe error message.
    }
  };

  return (
    <main className="auth-page" aria-labelledby="bootstrap-title">
      <div className="auth-page__toolbar">
        <LanguageToggleButton />
      </div>
      <Panel className="auth-panel"><form className="auth-form" onSubmit={(event) => void submit(event)}>
        <h1 id="bootstrap-title">{t("初始化管理员", "Set up administrator")}</h1>
        <Field label={t("管理密钥", "Management token")}><Input
          id="bootstrap-token"
          name="management-token"
          type="password"
          autoComplete="new-password"
          value={token}
          onChange={(event) => setToken(event.currentTarget.value)}
          required
        /></Field>
        <Field label={t("确认管理密钥", "Confirm management token")}><Input
          id="bootstrap-confirmation"
          name="management-token-confirmation"
          type="password"
          autoComplete="new-password"
          value={confirmation}
          onChange={(event) => setConfirmation(event.currentTarget.value)}
          required
        /></Field>
        {(localError || error) && <Notice tone="danger">{localError ?? error}</Notice>}
        <Button variant="primary" type="submit" loading={busy}>{t("创建管理员", "Create administrator")}</Button>
      </form></Panel>
    </main>
  );
}
