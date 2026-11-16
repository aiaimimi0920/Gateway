import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createGrokAuthApi } from "./grok-auth";

const binding = { providerId: "build", expectedRevision: "r1" };
const session = { id: "device-session", ...binding, status: "waiting_user", authorizationUrl: "https://accounts.x.ai/device",
  deviceAuthorization: { userCode: "TEST-CODE", pollIntervalSecs: 5, pollAfterSecs: 0, expiresInSecs: 120 }, identityVerified: false,
  credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null };

it("sends device mode without redirect, and a poll action with grant/revision but no callback", async () => {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  const api = createGrokAuthApi({ request } as GatewayApiClient, "owner-token");
  const input = { ...binding, clientId: "permitted-client", scopes: ["offline_access"], authorizationFlow: "device" as const };
  expect((await api.create(input, "grant")).deviceAuthorization?.userCode).toBe("TEST-CODE");
  expect(request).toHaveBeenLastCalledWith(expect.stringMatching(/grok-auth-sessions$/), expect.anything(), expect.objectContaining({ body: input, secretGrant: "grant", managementToken: "owner-token" }));
  await api.act(session.id, "poll", binding, "grant", "private-callback-must-not-leak");
  expect(request).toHaveBeenLastCalledWith(expect.stringMatching(/device-session\/poll$/), expect.anything(), expect.objectContaining({ body: { expectedRevision: "r1" }, secretGrant: "grant" }));
});

it.each([{ userCode: "", pollIntervalSecs: 5, expiresInSecs: 120 }, { userCode: "TEST", pollIntervalSecs: 0, expiresInSecs: 120 }, { userCode: "TEST", pollIntervalSecs: 5, expiresInSecs: -1 }])("rejects invalid device display fields: %j", async (deviceAuthorization) => {
  const request: GatewayApiClient["request"] = async (_path, schema) => schema.parse({ session: { ...session, deviceAuthorization: { pollAfterSecs: 0, ...deviceAuthorization } } });
  await expect(createGrokAuthApi({ request }, "token").get(session.id, binding)).rejects.toThrow();
});
