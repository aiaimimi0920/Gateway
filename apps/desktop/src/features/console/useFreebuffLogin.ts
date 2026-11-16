import { createFreebuffAuthApi, type FreebuffAuthSession } from "../../api/console/freebuff-auth";
import { GatewayApiError } from "../../api/errors";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";

export type FreebuffLoginOperations = OAuthOperations<FreebuffAuthSession>;
export type FreebuffLoginProps = ManagedOAuthProps<FreebuffAuthSession> & { onActivityChange?(active: boolean): void };
const policy = {
  polling: (status: string) => ["starting", "waiting_user", "polling", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "freebuff_login_rejected", "freebuff_login_save_rejected", "console_revision_conflict", "console_mutation_not_supported"],
  devicePolling: true,
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "freebuff_login_rejected" && error.message === "freebuff_login_unavailable",
};
// Reuse bounded management/session ownership only; the FreeBuff schema and approval policy stay separate.
export const useFreebuffLogin = (props: FreebuffLoginProps) => useManagedOAuth(props, createFreebuffAuthApi, policy);
