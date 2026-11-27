import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../client";
import { createCursorAuthApi, cursorSessionSchema, type CursorInput } from "./cursor-auth";
const input: CursorInput = { attemptId: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee", providerId: "cursor", expectedRevision: "r1" };
const response = () => ({ session: { id: input.attemptId, providerId: "cursor", expectedRevision: "r1", targetCredentialId: `cursor-${input.attemptId}`,
  status: "succeeded", authorizationUrl: null, authReceived: true, accountMatched: true, credentialPersisted: true, expiresAt: null,
  savedRevision: "r2", errorCode: null, inferenceStatus: "not_checked" } });
it("binds session receipts to exact account, revision and original attempt", async () => {
  const request = vi.fn().mockResolvedValue(response()), api = createCursorAuthApi({ request } as unknown as GatewayApiClient, "owner");
  await expect(api.create(input, "grant")).resolves.toMatchObject({ credentialPersisted: true });
  expect(request).toHaveBeenCalledWith("/v1/internal/gateway/console/cursor-auth-sessions", cursorSessionSchema,
    expect.objectContaining({ managementToken: "owner", secretGrant: "grant", method: "POST", body: input }));
  for (const patch of [{ providerId: "other" }, { expectedRevision: "r3" }, { targetCredentialId: "other" }, { credentialPersisted: false }, { accountMatched: false }, { authReceived: false }]) {
    request.mockResolvedValue({ session: { ...response().session, ...patch } });
    await expect(api.get(input)).rejects.toThrow("does not match");
  }
});
it("preserves an optional legacy email constraint without requiring it", async () => {
  const request = vi.fn().mockResolvedValue(response()), api = createCursorAuthApi({ request } as unknown as GatewayApiClient, "owner");
  const legacy = { ...input, expectedEmail: "owner@example.test" };
  await api.create(legacy, "grant");
  expect(request.mock.calls[0][2].body).toEqual(legacy);
});
it("binds email-free reauthorization to the original credential", async () => {
  const saved = { ...input, credentialId: "saved-account" };
  const request = vi.fn().mockResolvedValue({ session: { ...response().session, targetCredentialId: saved.credentialId } });
  const api = createCursorAuthApi({ request } as unknown as GatewayApiClient, "owner");
  await expect(api.create(saved, "grant")).resolves.toMatchObject({ targetCredentialId: saved.credentialId });
  expect(request.mock.calls[0][2].body).toEqual(saved);
  request.mockResolvedValue(response());
  await expect(api.get(saved)).rejects.toThrow("does not match");
});
