import type { ReactNode } from "react";
import { useManagementSession } from "../../session/useManagementSession";
import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { BootstrapPage } from "./BootstrapPage";
import { LoginPage } from "./LoginPage";
import { SessionErrorPage } from "./SessionErrorPage";

export type AuthBoundaryProps = {
  children: ReactNode;
};

export function AuthBoundary({ children }: AuthBoundaryProps) {
  const session = useManagementSession();
  const { t } = useUiLocale();

  if (session.phase === "checking") {
    return (
      <main className="auth-page auth-page--status" role="status">
        <div className="auth-page__toolbar">
          <LanguageToggleButton />
        </div>
        <section className="auth-panel">
          <p>{t("正在检查 Gateway 管理员初始化状态...", "Checking Gateway administrator setup...")}</p>
        </section>
      </main>
    );
  }
  if (session.phase === "bootstrap-required") {
    return (
      <BootstrapPage busy={session.busy} error={session.error} onSubmit={session.bootstrap} />
    );
  }
  if (session.phase === "authenticated") {
    return <>{children}</>;
  }
  if (session.phase === "error") {
    return (
      <SessionErrorPage
        busy={session.busy}
        error={session.error}
        onRetry={session.retryInitialization}
      />
    );
  }
  return <LoginPage busy={session.busy} error={session.error} onSubmit={session.login} />;
}
