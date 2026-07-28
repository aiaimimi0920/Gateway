import { type FormEvent, useState } from "react";
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
      <form className="auth-panel" onSubmit={(event) => void submit(event)}>
        <h1 id="bootstrap-title">{t("初始化管理员", "Set up administrator")}</h1>
        <p>{t("为这个本地 Gateway 实例创建管理密钥。", "Create the management token for this local Gateway instance.")}</p>
        <label htmlFor="bootstrap-token">{t("管理密钥", "Management token")}</label>
        <input
          id="bootstrap-token"
          name="management-token"
          type="password"
          autoComplete="new-password"
          value={token}
          onChange={(event) => setToken(event.currentTarget.value)}
          required
        />
        <label htmlFor="bootstrap-confirmation">{t("确认管理密钥", "Confirm management token")}</label>
        <input
          id="bootstrap-confirmation"
          name="management-token-confirmation"
          type="password"
          autoComplete="new-password"
          value={confirmation}
          onChange={(event) => setConfirmation(event.currentTarget.value)}
          required
        />
        {(localError || error) && <p role="alert">{localError ?? error}</p>}
        <button type="submit" disabled={busy}>
          {busy
            ? t("创建管理员中...", "Creating administrator...")
            : t("创建管理员", "Create administrator")}
        </button>
      </form>
    </main>
  );
}
