import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";

const identifier = z.string().min(1).max(512);
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["waiting_user", "exchanging", "ready", "claimed", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(8192).url().nullable(), identityVerified: z.boolean(),
  automaticCallback: z.boolean().optional(),
  deviceAuthorization: z.object({ userCode: z.string().min(1).max(128), pollIntervalSecs: z.number().int().positive(), pollAfterSecs: z.number().int().nonnegative(), expiresInSecs: z.number().int().nonnegative() }).nullable().optional(),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}) });
export type GrokAuthSession = z.infer<typeof sessionSchema>["session"];
export type GrokPrincipal = { kind: "Team" | "User"; id: string };
export type GrokAuthInput = {
  providerId: string; expectedRevision: string; credentialId?: string;
  clientId: string; scopes: string[]; redirectUri?: string; authorizationFlow?: "code" | "device" | "loopback";
  principalSelection?: GrokPrincipal; requiredPrincipal?: GrokPrincipal;
};
export type GrokAuthAction = "complete" | "poll" | "cancel" | "import";
export function createGrokAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<GrokAuthInput, GrokAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/grok-auth-sessions", sessionSchema);
}
