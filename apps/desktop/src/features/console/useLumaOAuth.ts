import { createLumaAuthApi, type LumaAuthSession } from "../../api/console/luma-auth";
import { GatewayApiError } from "../../api/errors";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";

export type LumaOAuthOperations = OAuthOperations<LumaAuthSession>;
export type LumaOAuthProps = ManagedOAuthProps<LumaAuthSession>;
const policy = {
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "luma_login_rejected" && error.message === "luma_login_unavailable",
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "luma_login_save_rejected", "console_revision_conflict", "console_mutation_not_supported", "luma_login_rejected"],
};
export const useLumaOAuth = (props: LumaOAuthProps) => useManagedOAuth(props, createLumaAuthApi, policy);
