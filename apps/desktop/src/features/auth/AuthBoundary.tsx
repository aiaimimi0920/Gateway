import type { ReactNode } from "react";
import { useManagementSession } from "../../session/useManagementSession";
import { BootstrapPage } from "./BootstrapPage";
import { LoginPage } from "./LoginPage";
import { SessionErrorPage } from "./SessionErrorPage";

export type AuthBoundaryProps = {
  children: ReactNode;
};

export function AuthBoundary({ children }: AuthBoundaryProps) {
  const session = useManagementSession();

  if (session.phase === "checking") {
    return <main role="status">Checking Gateway administrator setup...</main>;
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
