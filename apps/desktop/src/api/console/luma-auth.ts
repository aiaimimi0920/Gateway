import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";

export function validLumaRedirect(value: string): boolean {
  if (!/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}\/callback$/.test(value)) return false;
  try { const url = new URL(value); return url.href === value && Number(url.port) > 0 && Number(url.port) <= 65535; }
  catch { return false; }
}
const identifier = z.string().min(1).max(256);
export const lumaAuthSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["waiting_user", "exchanging", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(8192).url().nullable(), redirectUri: z.string().max(512).refine(validLumaRedirect),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(),
  savedRevision: identifier.nullable(), errorCode: z.string().max(256).nullable(),
}) });
export type LumaAuthSession = z.infer<typeof lumaAuthSchema>["session"];
export type LumaAuthInput = { providerId: string; expectedRevision: string; credentialId?: string; clientId: string; redirectUri: string; offlineAccess?: boolean };
export function createLumaAuthApi(client: GatewayApiClient, managementToken: string) {
  const api = createManagedAuthApi<LumaAuthInput, LumaAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/luma-auth-sessions", lumaAuthSchema);
  return { ...api, create: async (input: LumaAuthInput, grant: string) => {
    const session = await api.create(input, grant);
    if (session.redirectUri !== input.redirectUri) throw new Error("Luma login response does not match the registered redirect");
    return session;
  } };
}
