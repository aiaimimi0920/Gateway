import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createLumaAuthApi, type LumaAuthSession, validLumaRedirect } from "./luma-auth";

const binding = { providerId: "mcp", expectedRevision: "r1", credentialId: "stable" };
const input = { ...binding, clientId: "registered", redirectUri: "http://127.0.0.1:43567/callback" };
const view = (): LumaAuthSession => ({ id: "login/特殊", ...binding, status: "waiting_user",
  authorizationUrl: "https://agents.lumalabs.ai/mcp/oauth/authorize?state=synthetic", redirectUri: input.redirectUri,
  credentialId: null, targetCredentialId: "stable", savedRevision: null, errorCode: null });
function fixture(session: unknown = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createLumaAuthApi({ request } as GatewayApiClient, "owner") };
}
it("uses the Luma contract, grants mutations only, and sends callbacks only for complete", async () => {
  const { api, request } = fixture(); await api.create(input, "grant");
  expect(request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/luma-auth-sessions", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", body: input,
  }));
  expect(request.mock.calls[0][2]?.body).not.toHaveProperty("scopes");
  expect(request.mock.calls[0][2]?.body).not.toHaveProperty("site");
  for (const action of ["complete", "cancel", "import"] as const) {
    await api.act(view().id, action, binding, "grant", "private-callback");
    expect(request).toHaveBeenLastCalledWith(expect.stringContaining(`/login%2F%E7%89%B9%E6%AE%8A/${action}`), expect.anything(), expect.objectContaining({
      body: { expectedRevision: "r1", ...(action === "complete" ? { callbackUrl: "private-callback" } : {}) }, secretGrant: "grant",
    }));
  }
  await api.get(view().id, binding); expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("secretGrant");
});
it.each(["http://localhost:43567/callback", "http://127.0.0.1:0/callback", "http://127.0.0.1:65536/callback",
  "http://127.0.0.1:043567/callback", "http://127.0.0.1:43567/other", "http://127.0.0.1:43567/callback?code=c",
  "http://127.0.0.1:43567/callback#", "https://127.0.0.1:43567/callback"])("rejects a noncanonical redirect %s", (value) => {
  expect(validLumaRedirect(value)).toBe(false);
});
it("rejects a changed registered port", async () => {
  const { api } = fixture({ ...view(), redirectUri: "http://127.0.0.1:43568/callback" });
  await expect(api.create(input, "grant")).rejects.toThrow();
});
it.each([{ status: "fake-success" }, { providerId: "other" }, { expectedRevision: "r2" }, { targetCredentialId: "other" },
  { status: "succeeded", credentialId: "stable", savedRevision: null }, { id: "wrong-session" }])("rejects mismatched receipts %j", async (patch) => {
  await expect(fixture({ ...view(), ...patch }).api.get(view().id, binding)).rejects.toThrow();
});
