import { type FormEvent, useState } from "react";
import { Button, Field, Input, Notice, Panel } from "@neuro/ui";
import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type LoginPageProps = {
  busy: boolean;
  error?: string | null;
  onSubmit(token: string): Promise<void>;
};

export function LoginPage({ busy, error, onSubmit }: LoginPageProps) {
  const [token, setToken] = useState("");
  const { t } = useUiLocale();

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onSubmit(token);
    } catch {
      // The session provider exposes the server-safe error message.
    }
  };

  return (
    <main className="auth-page" aria-labelledby="login-title">
      <div className="auth-page__toolbar">
        <LanguageToggleButton />
      </div>
      <Panel className="auth-panel"><form className="auth-form" onSubmit={(event) => void submit(event)}>
        <h1 id="login-title">{t("登录", "Sign in")}</h1>
        <Field label={t("管理密钥", "Management token")}><Input
          id="login-token"
          name="management-token"
          type="password"
          autoComplete="current-password"
          value={token}
          onChange={(event) => setToken(event.currentTarget.value)}
          required
          autoFocus
        /></Field>
        {error && <Notice tone="danger">{error}</Notice>}
        <Button variant="primary" type="submit" loading={busy}>{t("登录", "Sign in")}</Button>
      </form></Panel>
    </main>
  );
}
