import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi, type AuthBinding } from "./managed-auth";

const identifier = z.string().min(1).max(512);
const seconds = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
// Browser approval is a session capability, not an OAuth grant or native inference API key.
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["starting", "waiting_user", "polling", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  loginUrl: z.string().max(8192).url().nullable(),
  pollIntervalSecs: seconds.nullable(), expiresInSecs: seconds.max(600).nullable(),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(), savedRevision: identifier.nullable(),
  errorCode: z.string().max(256).nullable(),
}) });
export type FreebuffAuthSession = z.infer<typeof sessionSchema>["session"];
export type FreebuffAuthInput = AuthBinding & { fingerprintId?: string };
export function createFreebuffAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<FreebuffAuthInput, FreebuffAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/freebuff-auth-sessions", sessionSchema);
}
