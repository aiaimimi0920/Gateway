import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createDimAuthApi, type DimAuthSession } from "./dim-auth";

const binding = { providerId: "renamed-dim", expectedRevision: "r1", credentialId: "stable" };
const input = { ...binding, site: "china" as const, clientId: "permitted-gateway", scopes: ["openid", "profile", "email"],
  redirectUri: "http://127.0.0.1:43567/oauth/callback" };
const view = (): DimAuthSession => ({ id: "login/特殊", ...binding, site: "china", status: "waiting_user",
  authorizationUrl: "https://dimagent.cn/oauth/authorize?client_id=permitted-gateway", redirectUri: input.redirectUri,
  identityAssurance: null, credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null });
function fixture(session: unknown = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createDimAuthApi({ request } as GatewayApiClient, "owner") };
}
it("uses the DIM root and explicit public registration; only complete sends a callback", async () => {
  const { api, request } = fixture(); await api.create(input, "grant");
  expect(request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/dim-auth-sessions", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", body: input,
  }));
  expect(request.mock.calls[0][2]?.body).not.toHaveProperty("clientSecret");
  for (const action of ["complete", "cancel", "import"] as const) {
    await api.act(view().id, action, binding, "grant", "private-callback");
    expect(request).toHaveBeenLastCalledWith(expect.stringContaining(`/login%2F%E7%89%B9%E6%AE%8A/${action}`), expect.anything(), expect.objectContaining({
      body: { expectedRevision: "r1", ...(action === "complete" ? { callbackUrl: "private-callback" } : {}) }, secretGrant: "grant",
    }));
  }
  await api.get(view().id, binding); expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("secretGrant");
});
it.each([{ site: "global" }, { redirectUri: "http://127.0.0.1:43568/oauth/callback" }])("rejects a changed registration receipt: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch }); await expect(api.create(input, "grant")).rejects.toThrow();
});
it.each([{ site: "foreign" }, { identityAssurance: "verified-account" }, { status: "fake-success" },
  { redirectUri: "http://localhost:43567/oauth/callback" }, { redirectUri: "http://127.0.0.1:0/oauth/callback" },
  { redirectUri: "http://127.0.0.1:65536/oauth/callback" }, { redirectUri: "http://127.0.0.1:043567/oauth/callback" },
  { redirectUri: "http://127.0.0.1:43567/other" }, { redirectUri: "http://127.0.0.1:43567/oauth/callback?code=c" },
  { providerId: "another-product" }, { expectedRevision: "r2" }, { targetCredentialId: "another-account" },
  { status: "succeeded", credentialId: "stable", savedRevision: null }])("rejects an invalid or mismatched session: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch }); await expect(api.get(view().id, binding)).rejects.toThrow();
});
