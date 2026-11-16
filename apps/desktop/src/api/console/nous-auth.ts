import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";
const identifier = z.string().min(1).max(256);
export const nousSessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(4096).url().nullable(),
  deviceAuthorization: z.object({ userCode: z.string().min(1).max(128), pollAfterSecs: z.number().int().nonnegative(), expiresInSecs: z.number().int().nonnegative() }).nullable(),
  receiptReceived: z.boolean(), credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}).strict() }).strict();
export type NousAuthSession = z.infer<typeof nousSessionSchema>["session"];
export type NousAuthInput = { providerId: string; expectedRevision: string; credentialId?: string; clientId: string };
export function createNousAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<NousAuthInput, NousAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/nous-auth-sessions", nousSessionSchema);
}
