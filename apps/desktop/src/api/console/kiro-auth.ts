import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";

const identifier = z.string().min(1).max(512);
const seconds = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const sessionSchema = z.object({ session: z.object({
  id: identifier, providerId: identifier, expectedRevision: identifier,
  status: z.enum(["registering", "waiting_user", "polling", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  verificationUri: z.string().max(4096).url().nullable(), verificationUriComplete: z.string().max(4096).url().nullable(),
  userCode: z.string().min(1).max(128).nullable(), pollIntervalSecs: seconds.nullable(), expiresInSecs: seconds.max(600).nullable(),
  credentialId: identifier.nullable(), targetCredentialId: identifier.nullable(), savedRevision: identifier.nullable(),
  errorCode: z.string().max(256).nullable(),
}) });
export type KiroAuthSession = z.infer<typeof sessionSchema>["session"];
export type KiroDirectory = { authMethod: "builder_id" } | { authMethod: "identity_center"; directoryRegion: string; startUrl: string };
export const kiroScopes = ["codewhisperer:analysis", "codewhisperer:completions", "codewhisperer:conversations", "codewhisperer:taskassist", "codewhisperer:transformations"] as const;
export type KiroScope = typeof kiroScopes[number];
export type KiroAuthInput = {
  providerId: string; expectedRevision: string; credentialId?: string;
  directory: KiroDirectory; scopes: KiroScope[]; machineId?: string;
};
export function createKiroAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<KiroAuthInput, KiroAuthSession>(client, managementToken,
    "/v1/internal/gateway/console/kiro-auth-sessions", sessionSchema);
}
