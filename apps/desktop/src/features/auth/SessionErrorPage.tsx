import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type SessionErrorPageProps = {
  busy: boolean;
  error?: string | null;
  onRetry(): Promise<void>;
};

export function SessionErrorPage({ busy, error, onRetry }: SessionErrorPageProps) {
  const { t } = useUiLocale();

  return (
    <main className="auth-page" aria-labelledby="session-error-title">
      <div className="auth-page__toolbar">
        <LanguageToggleButton />
      </div>
      <section className="auth-panel">
        <h1 id="session-error-title">{t("会话不可用", "Session unavailable")}</h1>
        <p role="alert">
          {error ?? t("无法验证 Gateway 管理员状态。", "Gateway administrator status could not be verified.")}
        </p>
        <button type="button" disabled={busy} onClick={() => void onRetry()}>
          {busy ? t("重试中...", "Retrying...") : t("重新初始化", "Retry initialization")}
        </button>
      </section>
    </main>
  );
}
