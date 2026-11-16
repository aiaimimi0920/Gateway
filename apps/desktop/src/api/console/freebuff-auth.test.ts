import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createFreebuffAuthApi, type FreebuffAuthSession } from "./freebuff-auth";

const binding = { providerId: "renamed-freebuff", expectedRevision: "r1", credentialId: "stable-account" };
const view = (): FreebuffAuthSession => ({ id: "login/特殊", ...binding, credentialId: null, targetCredentialId: "stable-account",
  status: "waiting_user", loginUrl: "https://freebuff.com/login?code=synthetic", pollIntervalSecs: 5,
  expiresInSecs: 590, savedRevision: null, errorCode: null });
function fixture(session: unknown = view()) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createFreebuffAuthApi({ request } as GatewayApiClient, "owner-token") };
}

it("binds approval and explicit granted actions without callback, token or device data in action bodies", async () => {
  const { api, request } = fixture(); expect(await api.create(binding, "grant")).toEqual(view());
  expect(request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/freebuff-auth-sessions", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner-token", secretGrant: "grant", body: binding,
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
  { status: "exchanging" }, { status: "fake-success" }, { pollIntervalSecs: -1 }, { pollIntervalSecs: 1.5 },
  { pollIntervalSecs: Number.MAX_SAFE_INTEGER + 1 }, { expiresInSecs: 601 }, { expiresInSecs: -1 }, { loginUrl: "not a URL" },
  { loginUrl: `https://freebuff.com/?code=${"x".repeat(8192)}` }, { errorCode: "x".repeat(257) },
])("rejects mismatched or malformed FreeBuff receipts: %j", async (patch) => {
  const { api } = fixture({ ...view(), ...patch }); await expect(api.get(view().id, binding)).rejects.toThrow();
});

it.each(["starting", "waiting_user", "polling", "ready", "importing", "succeeded", "save_unconfirmed", "failed", "cancelled", "expired"] as const)(
  "accepts actual FreeBuff status %s without borrowing OAuth callback states", async (status) => {
    const { api } = fixture({ ...view(), status, credentialId: "stable-account", savedRevision: "r2" });
    expect((await api.get(view().id, binding)).status).toBe(status);
  },
);

it("does not expose unexpected upstream authentication material from a public receipt", async () => {
  const { api } = fixture({ ...view(), authToken: "synthetic-secret", fingerprintId: "device", fingerprintHash: "hash" });
  const receipt = await api.get(view().id, binding);
  expect(receipt).not.toHaveProperty("authToken"); expect(receipt).not.toHaveProperty("fingerprintId"); expect(receipt).not.toHaveProperty("fingerprintHash");
});
