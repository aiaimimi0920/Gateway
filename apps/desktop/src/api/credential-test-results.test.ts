import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "./client";
import { createConsoleCredentialsApi } from "./console/credentials";

it("reads a scoped result through POST to preserve the browser grant origin, never through the run endpoint", async () => {
  const request = vi.fn().mockResolvedValue({ result: { results: [] } });
  const api = createConsoleCredentialsApi({ request } as unknown as GatewayApiClient);
  const signal = new AbortController().signal;
  const scope = { kind: "account" as const, id: "account-129" };
  await api.readProviderProbeResults?.("token", "grant", "pool/name", { signal }, scope);
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/providers/pool%2Fname/probe/results",
    expect.anything(), { method: "POST", body: { scope }, managementToken: "token", secretGrant: "grant", signal });
});
