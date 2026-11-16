import { createVertexAuthApi, type VertexAuthSession } from "../../api/console/vertex-auth";
import { GatewayApiError } from "../../api/errors";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";
import type { VertexTarget } from "./vertexOAuth";

export type VertexOAuthOperations = OAuthOperations<VertexAuthSession>;
export type VertexOAuthProps = ManagedOAuthProps<VertexAuthSession> & { target: VertexTarget | null; onActivityChange?(active: boolean): void };
const policy = {
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "vertex_login_save_rejected", "console_revision_conflict", "console_mutation_not_supported", "vertex_login_rejected"],
  unavailable: (error: unknown) => error instanceof GatewayApiError && error.code === "vertex_login_rejected" && error.message === "vertex_login_unavailable",
};
export const useVertexOAuth = (props: VertexOAuthProps) => useManagedOAuth(props, createVertexAuthApi, policy);
