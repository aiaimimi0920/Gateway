import { z } from "zod";
import type { GatewayApiClient } from "../client";
const root = "/v1/internal/gateway/console/cursor-auth-sessions";
export const cursorSessionSchema = z.object({ session: z.object({
  id: z.string().uuid(), providerId: z.string(), expectedRevision: z.string(), targetCredentialId: z.string(),
  status: z.enum(["waiting_user", "verifying", "awaiting_permission", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"]),
  authorizationUrl: z.string().max(4096).nullable(),
  authReceived: z.boolean(), accountMatched: z.boolean(), credentialPersisted: z.boolean(),
  expiresAt: z.string().nullable(), savedRevision: z.string().nullable(), errorCode: z.string().nullable(),
  inferenceStatus: z.literal("not_checked"),
}).strict() }).strict();
export type CursorSession = z.infer<typeof cursorSessionSchema>["session"];
export type CursorInput = {
  attemptId: string; providerId: string; expectedRevision: string; credentialId?: string;
  // Optional extra constraint for older callers; the UI obtains identity from authorization.
  expectedEmail?: string; accessToken?: string;
};
export function createCursorAuthApi(client: GatewayApiClient, managementToken: string) {
  const request = async (path: string, input: Pick<CursorInput, "attemptId" | "providerId" | "expectedRevision" | "credentialId">,
    body?: CursorInput, secretGrant?: string, signal?: AbortSignal, cancel = false) => {
    const abort = new AbortController();
    const stop = () => abort.abort();
    if (signal?.aborted) abort.abort();
    signal?.addEventListener("abort", stop, { once: true });
    const timer = setTimeout(stop, 30_000);
    try {
      const { session } = await client.request(path, cursorSessionSchema, {
        managementToken, secretGrant, method: body || cancel ? "POST" : "GET",
        body, signal: abort.signal,
      });
      if (session.id !== input.attemptId || session.providerId !== input.providerId ||
          session.expectedRevision !== input.expectedRevision ||
          session.targetCredentialId !== (input.credentialId ?? `cursor-${input.attemptId}`) ||
          session.status === "succeeded" && (!session.authReceived || !session.credentialPersisted || !session.accountMatched || !session.savedRevision)) {
        throw new Error("Cursor import response does not match this operation");
      }
      return session;
    } finally { clearTimeout(timer); signal?.removeEventListener("abort", stop); }
  };
  return {
    create: (input: CursorInput, grant: string) => request(root, input, input, grant),
    get: (input: CursorInput, signal?: AbortSignal) => request(`${root}/${encodeURIComponent(input.attemptId)}`, input, undefined, undefined, signal),
    cancel: (input: CursorInput, grant: string) => request(`${root}/${encodeURIComponent(input.attemptId)}/cancel`, input, undefined, grant, undefined, true),
    resume: (input: CursorInput, grant: string) => request(`${root}/${encodeURIComponent(input.attemptId)}/resume`, input, undefined, grant, undefined, true),
  };
}
