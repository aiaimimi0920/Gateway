import { expect, it, vi } from "vitest";
import { createNousAuthApi, nousSessionSchema } from "./nous-auth";
const session = { id: "s", providerId: "p", expectedRevision: "r", status: "waiting_user", authorizationUrl: "https://portal.nousresearch.com/device",
  deviceAuthorization: { userCode: "ABCD", pollAfterSecs: 1, expiresInSecs: 600 }, receiptReceived: false,
  credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null };
it("uses management auth and explicit secret consent without sending a callback or token", async () => {
  const request = vi.fn().mockResolvedValue({ session }); const api = createNousAuthApi({ request }, "owner");
  const input = { providerId: "p", expectedRevision: "r", clientId: "allowed" };
  await api.create(input, "grant"); await api.act("s", "poll", input, "grant"); await api.get("s", input);
  expect(request.mock.calls[0][2]).toMatchObject({ managementToken: "owner", secretGrant: "grant", body: input });
  expect(request.mock.calls[1][2].body).toEqual({ expectedRevision: "r" });
  expect(request.mock.calls[2][2].secretGrant).toBeUndefined();
  expect(nousSessionSchema.safeParse({ session }).success).toBe(true);
  expect(nousSessionSchema.safeParse({ session: { ...session, accessToken: "secret" } }).success).toBe(false);
});
it.each([{ providerId: "foreign" }, { expectedRevision: "stale" }, { status: "succeeded" }])("rejects foreign or incomplete receipt %j", async (patch) => {
  const request = vi.fn().mockResolvedValue({ session: { ...session, ...patch } });
  await expect(createNousAuthApi({ request }, "owner").create({ providerId: "p", expectedRevision: "r", clientId: "allowed" }, "grant")).rejects.toThrow();
});
