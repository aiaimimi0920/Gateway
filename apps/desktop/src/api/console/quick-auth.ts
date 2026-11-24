import { z } from "zod";
import type { GatewayApiClient } from "../client";
import { createManagedAuthApi } from "./managed-auth";

export function validQuickRedirect(value: string): boolean {
  if (!/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/.test(value)) return false;
  try {
    const url = new URL(value);
    return url.protocol === "http:" && url.hostname === "127.0.0.1" && Number(url.port) > 0 && Number(url.port) <= 65535;
  } catch {
    return false;
  }
}

const identifier = z.string().min(1).max(256);

const sessionSchema = z.object({
  session: z.object({
    id: identifier,
    providerId: identifier,
    expectedRevision: identifier,
    status: z.enum([
      "waiting_user",
      "exchanging",
      "ready",
      "importing",
      "succeeded",
      "save_unconfirmed",
      "failed",
      "cancelled",
      "expired",
    ]),
    step: z.enum(["discovery", "authorize"]),
    authorizationUrl: z.string().max(8192).url().nullable(),
    redirectUri: z.string().max(512).refine(validQuickRedirect),
    accountSummary: z.string().max(512).nullable(),
    credentialId: identifier.nullable(),
    targetCredentialId: identifier.nullable(),
    savedRevision: identifier.nullable(),
    errorCode: z.string().max(256).nullable(),
  }),
});

export type QuickAuthSession = z.infer<typeof sessionSchema>["session"];
export type QuickAuthInput = {
  providerId: string;
  expectedRevision: string;
  credentialId?: string;
};

export function createQuickAuthApi(client: GatewayApiClient, managementToken: string) {
  return createManagedAuthApi<QuickAuthInput, QuickAuthSession>(
    client,
    managementToken,
    "/v1/internal/gateway/console/quick-auth-sessions",
    sessionSchema,
  );
}
