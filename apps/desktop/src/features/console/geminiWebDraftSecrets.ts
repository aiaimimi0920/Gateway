// The capture response is secret-bearing; the editable route document must not be.
import type { CredentialSecretEdit } from "./credentialDocument";
import { isRecord } from "./routeDocument";

export function separateGeminiWebDraftSecrets(
  credential: Record<string, unknown>, providerId: string, credentialId: string,
): { credential: Record<string, unknown>; secretEdits: CredentialSecretEdit[] } {
  const next = { ...credential };
  delete next.api_key;
  delete next.auth_token;
  if (isRecord(next.headers)) {
    next.headers = Object.fromEntries(Object.entries(next.headers)
      .filter(([key]) => key.toLowerCase() !== "cookie"));
  }
  const secretEdits: CredentialSecretEdit[] = [];
  if (isRecord(next.extra_body)) {
    const body = { ...next.extra_body };
    for (const key of ["accessToken", "sessionId"] as const) {
      const value = body[key];
      delete body[key];
      if (typeof value === "string" && value.trim()) {
        secretEdits.push({providerId, credentialId, field: `extra_body/${key}`,
          operation: "replace", value});
      }
    }
    next.extra_body = body;
  }
  return { credential: next, secretEdits };
}
