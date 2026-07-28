import { type FormEvent, useState } from "react";
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
      <form className="auth-panel" onSubmit={(event) => void submit(event)}>
        <h1 id="login-title">{t("登录", "Sign in")}</h1>
        <p>{t("使用当前 Gateway 配置的管理密钥登录。", "Use the management token configured for this Gateway.")}</p>
        <label htmlFor="login-token">{t("管理密钥", "Management token")}</label>
        <input
          id="login-token"
          name="management-token"
          type="password"
          autoComplete="current-password"
          value={token}
          onChange={(event) => setToken(event.currentTarget.value)}
          required
          autoFocus
        />
        {error && <p role="alert">{error}</p>}
        <button type="submit" disabled={busy}>
          {busy ? t("登录中...", "Signing in...") : t("登录", "Sign in")}
        </button>
      </form>
    </main>
  );
}
