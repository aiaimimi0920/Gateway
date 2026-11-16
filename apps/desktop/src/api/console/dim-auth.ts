import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";

export type DimSite = "china" | "global";
export function validDimRedirect(value: string): boolean {
  if (!/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}\/oauth\/callback$/.test(value)) return false;
  try {
    const url = new URL(value);
    return url.href === value && Number(url.port) > 0 && Number(url.port) <= 65535;
  } catch { return false; }
}
const identifier = z.string().min(1).max(256);
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier, site: z.enum(["china", "global"]),
  status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(8192).url().nullable(),
  redirectUri: z.string().max(512).refine(validDimRedirect),
  identityAssurance: z.enum(["user_info_endpoint", "signed_id_token"]).nullable(),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}) });
export type DimAuthSession = z.infer<typeof sessionSchema>["session"];
export type DimAuthInput = {
  providerId: string; expectedRevision: string; credentialId?: string;
  site: DimSite; clientId: string; scopes: string[]; redirectUri: string;
};
export function createDimAuthApi(client: GatewayApiClient, managementToken: string) {
  const api = createManagedAuthApi<DimAuthInput, DimAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/dim-auth-sessions", sessionSchema);
  return { ...api, create: async (input: DimAuthInput, secretGrant: string) => {
    const session = await api.create(input, secretGrant);
    // A registered callback is an exact identity, not a suggestion for an available local port.
    if (session.site !== input.site || session.redirectUri !== input.redirectUri) {
      throw new Error("DIM login response does not match the explicit registration");
    }
    return session;
  } };
}
