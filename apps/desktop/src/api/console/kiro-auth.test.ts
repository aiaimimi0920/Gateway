import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createKiroAuthApi, type KiroAuthSession } from "./kiro-auth";

const binding = { providerId: "renamed-kiro", expectedRevision: "r1", credentialId: "stable-account" };
const input = { ...binding, directory: { authMethod: "identity_center" as const, directoryRegion: "eu-west-1", startUrl: "https://organization.awsapps.com/start" }, scopes: ["codewhisperer:conversations" as const] };
const view = (): KiroAuthSession => ({ id: "login/特殊", providerId: binding.providerId, expectedRevision: "r1", status: "waiting_user",
  verificationUri: "https://device.sso.eu-west-1.api.aws/", verificationUriComplete: null, userCode: "ABCD-EFGH", pollIntervalSecs: 5, expiresInSecs: 590,
  credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null });
function fixture(session: unknown = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createKiroAuthApi({ request } as GatewayApiClient, "owner-token") };
}

it("uses explicit Kiro directory/scopes and secret-granted poll without callbacks", async () => {
  const { api, request } = fixture(); expect(await api.create(input, "grant")).toEqual(view());
  expect(request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/kiro-auth-sessions", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner-token", secretGrant: "grant", body: input,
  }));
  for (const action of ["poll", "cancel", "import"] as const) {
    await api.act(view().id, action, binding, "grant");
    expect(request).toHaveBeenLastCalledWith(expect.stringContaining(`/login%2F%E7%89%B9%E6%AE%8A/${action}`), expect.anything(), expect.objectContaining({
      method: "POST", managementToken: "owner-token", secretGrant: "grant", body: { expectedRevision: "r1" },
    }));
  }
  await api.get(view().id, binding);
  expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("secretGrant");
  expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("body");
});

it.each([
  { providerId: "different" }, { expectedRevision: "r2" }, { id: "different" }, { targetCredentialId: "different" }, { credentialId: "different" },
  { status: "succeeded", credentialId: "stable-account", savedRevision: null }, { status: "succeeded", credentialId: null, savedRevision: "r2" },
  { status: "exchanging" }, { status: "fake-success" }, { pollIntervalSecs: -1 }, { expiresInSecs: 601 }, { userCode: "x".repeat(129) },
])("rejects mismatched or malformed Kiro responses: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch }); await expect(api.get(view().id, binding)).rejects.toThrow();
});

it.each(["registering", "polling", "save_unconfirmed"] as const)("accepts the actual Kiro status %s without borrowing callback statuses", async (status) => {
  const { api } = fixture({ ...view(), status }); expect((await api.get(view().id, binding)).status).toBe(status);
});
