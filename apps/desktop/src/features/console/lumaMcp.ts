import { validLumaRedirect } from "../../api/console/luma-auth";
import { isRecord } from "./routeDocument";

// UI dispatch only; backend admission remains authoritative for the complete profile.
export const isLumaMcpProvider = (provider: unknown): boolean => isRecord(provider)
  && provider.adapter === "lumalabs_mcp_readonly" && provider.protocol_profile === "lumalabs_mcp_readonly";
export type LumaRegistration = { clientId: string; redirectUri: string; offlineAccess?: boolean };
export function savedLumaRegistration(provider: unknown, credentialId?: string): LumaRegistration | undefined {
  if (!isRecord(provider) || !isLumaMcpProvider(provider) || !credentialId || !Array.isArray(provider.credentials)) return;
  const credential: unknown = provider.credentials.find((c: unknown) => isRecord(c) && c.id === credentialId);
  const context = isRecord(credential) ? credential.luma_oauth : undefined;
  if (isRecord(context) && typeof context.client_id === "string" && typeof context.redirect_uri === "string"
    && validLumaRedirect(context.redirect_uri)) return { clientId: context.client_id, redirectUri: context.redirect_uri,
      ...(context.offline_access === true ? { offlineAccess: true } : {}) };
}
