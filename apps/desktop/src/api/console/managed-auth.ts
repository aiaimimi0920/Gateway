import type { ZodType } from "zod";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";

export type AuthBinding = { providerId: string; expectedRevision: string; credentialId?: string };
export type AuthSession = {
  id: string; providerId: string; expectedRevision: string; status: string;
  credentialId: string | null; targetCredentialId: string | null; savedRevision: string | null;
};
export type AuthAction = "complete" | "poll" | "cancel" | "import";
export type ManagedAuthApi<I extends AuthBinding, S extends AuthSession> = {
  create(input: I, secretGrant: string): Promise<S>;
  get(id: string, binding: AuthBinding, signal?: AbortSignal): Promise<S>;
  act(id: string, action: AuthAction, binding: AuthBinding, secretGrant: string, callbackUrl?: string): Promise<S>;
};

// Shared transport only: each product owns its schema, status set and authorization inputs.
export function createManagedAuthApi<I extends AuthBinding, S extends AuthSession>(
  client: GatewayApiClient, managementToken: string, root: string, schema: ZodType<{ session: S }>,
): ManagedAuthApi<I, S> {
  const request = async (path: string, options: GatewayApiRequestOptions, binding: AuthBinding, id?: string) => {
    const abort = new AbortController();
    const cancel = () => abort.abort();
    if (options.signal?.aborted) abort.abort();
    options.signal?.addEventListener("abort", cancel, { once: true });
    const timer = setTimeout(cancel, 30_000);
    try {
      const { session } = await client.request(path, schema, { ...options, managementToken, signal: abort.signal });
      if (session.providerId !== binding.providerId || session.expectedRevision !== binding.expectedRevision
        || id !== undefined && session.id !== id
        || binding.credentialId !== undefined && [session.targetCredentialId, session.credentialId].some((value) => value !== null && value !== binding.credentialId)
        || session.status === "succeeded" && (!session.credentialId || !session.savedRevision)) {
        throw new Error("Login response does not match the requested operation");
      }
      return session;
    } finally {
      clearTimeout(timer);
      options.signal?.removeEventListener("abort", cancel);
    }
  };
  return {
    create: (input, secretGrant) => request(root, { method: "POST", secretGrant, body: input }, input),
    get: (id, binding, signal) => request(`${root}/${encodeURIComponent(id)}`, { signal }, binding, id),
    act: (id, action, binding, secretGrant, callbackUrl) =>
      request(`${root}/${encodeURIComponent(id)}/${action}`, { method: "POST", secretGrant,
        body: { expectedRevision: binding.expectedRevision, ...(action === "complete" ? { callbackUrl } : {}) } }, binding, id),
  };
}
