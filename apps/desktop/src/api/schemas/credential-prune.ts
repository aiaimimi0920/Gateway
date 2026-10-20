import { z } from "zod";

export const credentialPrunePreviewSchema = z.object({ preview: z.object({
  providerId: z.string(), revision: z.string(), digest: z.string(), archiveRequired: z.literal(true),
  items: z.array(z.object({ credentialId: z.string(), classification: z.enum([
    "permanent_auth_failure", "account_deleted", "permanent_upstream_rejection",
  ]) })).max(1024),
}) });

export const credentialPruneReceiptSchema = z.object({ receipt: z.object({
  providerId: z.string(), sourceRevision: z.string(), revision: z.string(),
  outcome: z.enum(["completed", "revision_changed", "commit_unconfirmed"]),
  removedCount: z.number().int().nonnegative().nullable(),
  items: z.array(z.object({ credentialId: z.string(), status: z.enum([
    "archived_and_removed", "archive_failed", "archive_budget_exhausted",
    "retained_revision_changed", "commit_unconfirmed",
  ]) })).max(128),
}) });

export type CredentialPrunePreview = z.infer<typeof credentialPrunePreviewSchema>["preview"];
export type CredentialPruneReceipt = z.infer<typeof credentialPruneReceiptSchema>["receipt"];
export type CredentialPruneSelection = {
  expectedRevision: string;
  previewDigest: string;
  credentialIds: string[];
};
