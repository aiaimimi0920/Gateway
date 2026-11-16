import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../client";
import { createLobsterAiAuthApi } from "./lobsterai-auth";
import { lobsterSession } from "../../features/console/lobsteraiTestFixtures";
const binding = { providerId: "renamed", expectedRevision: "r1", credentialId: "stable" };
function fixture(session: unknown = lobsterSession("waiting_user")) {
  const request = vi.fn(async (_path, schema, _options?: GatewayApiRequestOptions) => schema.parse({ session }));
  return { request, api: createLobsterAiAuthApi({ request } as GatewayApiClient, "owner") };
}
it("uses the owner-bound browser session root and sends no credentials or registration", async () => {
  const { api, request } = fixture(); await api.create(binding, "grant");
  expect(request.mock.calls[0][0]).toBe("/v1/internal/gateway/console/lobsterai-auth-sessions");
  expect(request.mock.calls[0][2]?.body).toEqual(binding);
  await api.act("login", "import", binding, "grant"); expect(request.mock.calls.at(-1)![2]?.body).toEqual({ expectedRevision: "r1" });
  await api.get("login", binding); expect(request.mock.calls.at(-1)![2]).not.toHaveProperty("secretGrant");
});
it.each([{ providerId: "other" }, { expectedRevision: "r2" }, { targetCredentialId: "other" }, { redirectUri: "http://evil.invalid/callback" },
  { authorizationUrl: "https://lobsterai.youdao.com/portal#/other" }, { status: "succeeded", credentialId: "stable", savedRevision: null }, { accessToken: "unexpected-secret" }])("rejects invalid or mismatched session %j", async (patch) => {
  const { api } = fixture({ ...lobsterSession("waiting_user"), ...patch }); await expect(api.get("login", binding)).rejects.toThrow();
});
