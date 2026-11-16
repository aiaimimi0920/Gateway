import { createDimAuthApi, type DimAuthSession } from "../../api/console/dim-auth";
import { GatewayApiError } from "../../api/errors";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";

export type DimOAuthOperations = OAuthOperations<DimAuthSession>;
export type DimOAuthProps = ManagedOAuthProps<DimAuthSession>;
const policy = {
  // A missing pre-save session can restart; a missing save receipt is not a failed commit.
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "dim_login_rejected" && error.message === "dim_login_unavailable",
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "dim_login_save_rejected", "dim_login_identity_mismatch", "console_revision_conflict", "console_mutation_not_supported", "dim_login_rejected"],
};
export const useDimOAuth = (props: DimOAuthProps) => useManagedOAuth(props, createDimAuthApi, policy);
