import { z } from "zod";

const measurementSchema = z.object({
  testSetId: z.string().regex(/^[0-9a-f]{64}$/), elapsedMs: z.number().int().nonnegative().nullable(),
  quota: z.object({ status: z.string(), unit: z.string().nullable(), consumed: z.number().nonnegative().nullable(),
    before: z.number().nonnegative().nullable(), after: z.number().nonnegative().nullable(), source: z.string().nullable() }),
});

export const credentialTestAssessmentSchema = z.object({
  planId: z.string().optional(),
  policySource: z.string(), mode: z.string(),
  models: z.array(z.object({ model: z.string(), callable: z.boolean(), completedCount: z.number().int().nonnegative(),
    gradedCount: z.number().int().nonnegative(), correctCount: z.number().int().nonnegative(),
    score: z.number().int().min(0).max(100).nullable(), capabilityLevel: z.string(), measurement: measurementSchema.optional() })).max(128),
  cases: z.array(z.object({ caseId: z.string(), name: z.string(), model: z.string(), difficulty: z.number().int().min(1).max(3),
    status: z.string(), answer: z.string().nullable(), expectedAnswer: z.string().nullable(), correct: z.boolean().nullable(), message: z.string(), elapsedMs: z.number().int().nonnegative().optional() })).max(128),
});
