import { createGrokAuthApi, type GrokAuthSession } from "../../api/console/grok-auth";
import { GatewayApiError } from "../../api/errors";
import type { GrokOAuthOperations } from "./grokOAuthOperations";
import { useManagedOAuth, type ManagedOAuthProps } from "./useManagedOAuth";

export type GrokOAuthProps = ManagedOAuthProps<GrokAuthSession> & { operations: GrokOAuthOperations };
const policy = {
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "claimed", "importing"].includes(status),
  rejected: ["console_secret_access_required", "grok_login_save_rejected", "console_revision_conflict", "console_mutation_not_supported", "grok_login_rejected"],
  // Only a definitive missing receipt uses recovery; transport failures keep their retry budget.
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "grok_login_rejected" && error.message === "grok_login_unavailable",
};
export const useGrokOAuth = (props: GrokOAuthProps, devicePolling = false) => useManagedOAuth(props, createGrokAuthApi, { ...policy, devicePolling });
