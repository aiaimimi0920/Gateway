import type { ConsoleRouteDocument } from "../../api/contracts";
import { updateExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import { parseProviderSessionMaterial, type ProviderSessionMaterial } from "./providerSessionMaterial";
import { sessionMaterialTarget, sessionAuthForProduct, type SessionMaterialBinding } from "./providerSessionTarget";

const record = (value: unknown): Record<string, unknown> => value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};

/** No side effects before validation. Secrets only enter the existing revision/grant-protected patch chain. */
export function applyProviderSessionMaterial(document: ConsoleRouteDocument, providerId: string, credentialId: string, supplied: ProviderSessionMaterial) {
  const material = parseProviderSessionMaterial(JSON.stringify(supplied), supplied.product);
  const binding: SessionMaterialBinding = { product: material.product, ...(material.realmId ? { realmId: material.realmId } : {}),
    ...(["perplexity-web", "qwen-web"].includes(material.product) ? { userAgent: material.userAgent } : {}),
    ...(material.agentId ? { agentId: material.agentId } : {}), ...(material.expiresAt ? { expiresAt: material.expiresAt } : {}) };
  const { credential } = sessionMaterialTarget(document, providerId, credentialId, binding);
  const headers = Object.fromEntries(Object.entries(record(credential.headers)).filter(([key]) => !["cookie", "authorization", ...(material.userAgent ? ["user-agent"] : [])].includes(key.toLowerCase())));
  const extraBody: Record<string, unknown> = { ...record(credential.extra_body), ...(material.realmId ? { realmId: material.realmId } : {}),
    ...(material.agentId ? { freebuffAgentId: material.agentId } : {}) };
  const sessionAuth = sessionAuthForProduct(material.product, material.expiresAt);
  const nextDocument = updateExplicitCredential(document, { providerId, credentialId }, { api_key: undefined, auth_token: undefined, refresh_token: undefined,
    headers: { ...headers, ...(material.userAgent ? { "User-Agent": material.userAgent } : {}) },
    extra_body: extraBody, expires_at: material.expiresAt, session_auth: sessionAuth });
  const secretEdits: CredentialSecretEdit[] = [
    material.token ? { providerId, credentialId, field: "api_key", operation: "replace", value: material.token }
      : { providerId, credentialId, field: "api_key", operation: "clear" },
    material.cookieHeader ? { providerId, credentialId, field: "headers/Cookie", operation: "replace", value: material.cookieHeader }
      : { providerId, credentialId, field: "headers/Cookie", operation: "clear" },
    { providerId, credentialId, field: "headers/Authorization", operation: "clear" },
    { providerId, credentialId, field: "auth_token", operation: "clear" },
    { providerId, credentialId, field: "refresh_token", operation: "clear" },
  ];
  return { document: nextDocument, secretEdits: secretEdits.map((edit) => ({ ...edit, sessionBinding: binding })) };
}
