import { z } from "zod";

const assetSchema = z.object({ id: z.string(), kind: z.string(), state: z.string(), url: z.string().nullable() });
export const lumaReviewSchema = z.object({
  fingerprint: z.string(), invocationId: z.string().nullable(), kind: z.string(),
  content: z.record(z.string(), z.unknown()), answerFormat: z.record(z.string(), z.unknown()).nullable(),
  show: z.array(z.string()), assets: z.array(assetSchema),
});
export type LumaReviewAnswer = { invocationId: string; fingerprint: string; result: unknown };
export const lumaCreationTaskSchema = z.object({
  id: z.string(), providerId: z.string(), credentialId: z.string(), revision: z.string(),
  kind: z.enum(["image", "video"]), budgetCredits: z.number(), state: z.string(),
  requestedModel: z.string().nullable().optional(), operation: z.string().nullable().optional(),
  sessionId: z.string().nullable(), runId: z.string().nullable(), ended: z.boolean(),
  workspaceBinding: z.object({ taskId: z.string(), key: z.string(), id: z.string() }).nullable().optional(),
  leaseUntil: z.number(), error: z.string().nullable(),
  assets: z.array(assetSchema),
  review: lumaReviewSchema.nullable().optional(),
  responses: z.array(z.object({ invocationId: z.string(), status: z.string() })).optional(),
  sessionUpdate: z.record(z.string(), z.unknown()).nullable().optional(),
  usage: z.object({ credits: z.number(), settled: z.boolean(), estimated: z.boolean(),
    unresolvedEvents: z.number(), scope: z.literal("session") }).nullable(),
});
export const lumaCreationResponseSchema = z.object({ revision: z.string(), providerId: z.string(),
  credentialId: z.string(), task: lumaCreationTaskSchema });
export type LumaCreationTask = z.infer<typeof lumaCreationTaskSchema>;
