import { createAccioAuthApi, type AccioAuthSession } from "../../api/console/accio-auth";
import { GatewayApiError } from "../../api/errors";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";

export type AccioOAuthOperations = OAuthOperations<AccioAuthSession>;
export type AccioOAuthProps = ManagedOAuthProps<AccioAuthSession> & { onActivityChange?(active: boolean): void };
const policy = {
  // Missing pre-save sessions may restart; missing save receipts must retain the duplicate-save fence.
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "accio_login_rejected" && error.message === "accio_login_unavailable",
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "accio_login_save_rejected", "console_revision_conflict", "console_mutation_not_supported", "accio_login_rejected"],
};
export const useAccioOAuth = (props: AccioOAuthProps) => useManagedOAuth(props, createAccioAuthApi, policy);
