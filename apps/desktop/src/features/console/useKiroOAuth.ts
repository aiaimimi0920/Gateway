import { createKiroAuthApi, type KiroAuthSession } from "../../api/console/kiro-auth";
import { GatewayApiError } from "../../api/errors";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";

export type KiroOAuthOperations = OAuthOperations<KiroAuthSession>;
export type KiroOAuthProps = ManagedOAuthProps<KiroAuthSession> & { onActivityChange?(active: boolean): void };
const policy = {
  // Missing pre-save sessions may restart; missing save receipts must retain the duplicate-save fence.
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "kiro_login_rejected" && error.message === "kiro_login_unavailable",
  polling: (status: string) => ["registering", "waiting_user", "polling", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "kiro_login_rejected", "kiro_login_save_rejected", "console_revision_conflict", "console_mutation_not_supported"],
  devicePolling: true,
};
// Timed checks are GET-only; device polling requires the operator's explicit action.
export const useKiroOAuth = (props: KiroOAuthProps) => useManagedOAuth(props, createKiroAuthApi, policy);
