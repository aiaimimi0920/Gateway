import { expect, it, vi } from "vitest";
import { createConsoleCredentialsApi } from "./credentials";
import type { GatewayApiClient } from "../client";
import { credentialPrunePreviewSchema, credentialPruneReceiptSchema } from "../schemas/credential-prune";

it("uses separate encoded management-only POSTs and sends only the explicit selection on apply", async () => {
  const client: GatewayApiClient = { request: async <T,>() => undefined as T };
  const request = vi.spyOn(client, "request");
  const api = createConsoleCredentialsApi(client);
  await api.previewCredentialPrune("token", "p/特殊");
  expect(request).toHaveBeenCalledExactlyOnceWith(
    "/v1/internal/gateway/credential-pool-automation/providers/p%2F%E7%89%B9%E6%AE%8A/prune/preview",
    credentialPrunePreviewSchema, { method: "POST", managementToken: "token" });
  const body = { expectedRevision: "r1", previewDigest: "d", credentialIds: ["a"] };
  await api.applyCredentialPrune("token", "p/特殊", body);
  expect(request).toHaveBeenLastCalledWith(
    "/v1/internal/gateway/credential-pool-automation/providers/p%2F%E7%89%B9%E6%AE%8A/prune/apply",
    credentialPruneReceiptSchema, { method: "POST", managementToken: "token", body });
});

it("rejects unknown classifications, over-limit lists and ambiguous receipt counts", () => {
  const row = { credentialId: "a", classification: "account_deleted" };
  const preview = { providerId: "p", revision: "r", digest: "d", archiveRequired: true, items: [row] };
  expect(credentialPrunePreviewSchema.safeParse({ preview }).success).toBe(true);
  expect(credentialPrunePreviewSchema.safeParse({ preview: { ...preview, items: [{ ...row, classification: "quota_exhausted" }] } }).success).toBe(false);
  expect(credentialPrunePreviewSchema.safeParse({ preview: { ...preview, items: Array(1025).fill(row) } }).success).toBe(false);
  const receipt = { providerId: "p", sourceRevision: "r", revision: "r", outcome: "commit_unconfirmed", removedCount: null,
    items: [{ credentialId: "a", status: "commit_unconfirmed" }] };
  expect(credentialPruneReceiptSchema.safeParse({ receipt }).success).toBe(true);
  expect(credentialPruneReceiptSchema.safeParse({ receipt: { ...receipt, removedCount: -1 } }).success).toBe(false);
  expect(credentialPruneReceiptSchema.safeParse({ receipt: { ...receipt, items: Array(129).fill(receipt.items[0]) } }).success).toBe(false);
});
