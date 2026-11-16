import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";

export type VertexTarget = { project: string; location: string; quota_project?: string };

const identifier = z.string().min(1).max(256);
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(8192).url().nullable(),
  redirectUri: z.string().max(256).refine((value) => /^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}\/$/.test(value)
    && Number(new URL(value).port) <= 65535),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}) });
export type VertexAuthSession = z.infer<typeof sessionSchema>["session"];
export type VertexAuthInput = {
  providerId: string; expectedRevision: string; credentialId?: string;
  clientId: string; clientSecret?: string; target: VertexTarget;
};
export function createVertexAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<VertexAuthInput, VertexAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/vertex-auth-sessions", sessionSchema);
}
