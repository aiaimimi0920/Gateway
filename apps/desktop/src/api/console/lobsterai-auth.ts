import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi, type AuthBinding } from "./managed-auth";
import { validLobsterRedirect, lobsteraiAuthorizationUrl } from "../../features/console/lobsteraiAuthorizationUrl";
const identifier = z.string().min(1).max(256);
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().refine((url) => lobsteraiAuthorizationUrl(url) !== null).nullable(),
  redirectUri: z.string().refine(validLobsterRedirect), credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}).strict() });
export type LobsterAiAuthSession = z.infer<typeof sessionSchema>["session"];
export function createLobsterAiAuthApi(client: GatewayApiClient, token: string) {
  return createManagedAuthApi<AuthBinding, LobsterAiAuthSession>(client, token, "/v1/internal/gateway/console/lobsterai-auth-sessions", sessionSchema);
}
