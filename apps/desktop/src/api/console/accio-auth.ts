import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi, type AuthAction } from "./managed-auth";

const identifier = z.string().min(1).max(512);
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(8192).url().nullable(),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}) });
export type AccioAuthSession = z.infer<typeof sessionSchema>["session"];
export type AccioAuthInput = {
  providerId: string; expectedRevision: string; credentialId?: string;
  clientId: string; loginSite: "china" | "global"; callbackPort: number; utdid?: string;
};
export type AccioAuthAction = AuthAction;
export function createAccioAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<AccioAuthInput, AccioAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/accio-auth-sessions", sessionSchema);
}
