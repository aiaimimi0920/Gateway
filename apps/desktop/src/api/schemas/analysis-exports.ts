import { z } from "zod";

import type {
  ConsoleAnalysisExportDiffResponse,
  ConsoleAnalysisExportInventorySummaryResponse,
  ConsolePersistedAnalysisExportResponse,
} from "../contracts";
import {
  consoleJsonObjectSchema,
  consoleNullableBooleanSchema,
  consoleNullableNumberSchema,
  consoleNullableStringSchema,
  consoleNullableTimestampSchema,
  consoleStringArraySchema,
  consoleSummaryBucketsSchema,
} from "./common";

const consolePersistedAnalysisExportSchema = z.object({
  exportId: z.string().min(1),
  label: consoleNullableStringSchema,
  tags: consoleStringArraySchema,
  status: z.string(),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
  objectPrefix: z.string(),
  filters: z.object({
    projectId: consoleNullableStringSchema,
    routePolicyId: consoleNullableStringSchema,
    providerAccountId: consoleNullableStringSchema,
    sessionId: consoleNullableStringSchema,
    apiKeyId: consoleNullableStringSchema,
    responseId: consoleNullableStringSchema,
    protocolFamily: consoleNullableStringSchema,
    status: consoleNullableStringSchema,
    endpointKind: consoleNullableStringSchema,
    stream: consoleNullableBooleanSchema,
    errorCode: consoleNullableStringSchema,
    fallbackEligible: consoleNullableBooleanSchema,
    createdFrom: consoleNullableTimestampSchema,
    createdTo: consoleNullableTimestampSchema,
    artifactAvailable: consoleNullableBooleanSchema,
    limit: z.number().int().nonnegative(),
    textMode: z.string(),
    maxTextChars: z.number().int().nonnegative(),
  }),
  sampleCount: z.number().int().nonnegative(),
  requestArtifactCount: z.number().int().nonnegative(),
  responseArtifactCount: z.number().int().nonnegative(),
  retentionExpiresAt: consoleNullableTimestampSchema,
  cleanedUpAt: consoleNullableTimestampSchema,
  lastCleanupError: consoleNullableStringSchema,
  files: z
    .array(
      z.object({
        kind: z.string(),
        objectKey: z.string(),
        contentType: z.string(),
        sizeBytes: z.number().int().nonnegative(),
        sha256: z.string(),
        lineCount: consoleNullableNumberSchema,
      }),
    )
    .nullish()
    .transform((value) => value ?? []),
  manifest: consoleJsonObjectSchema,
});

export const consolePersistedAnalysisExportResponseSchema: z.ZodType<ConsolePersistedAnalysisExportResponse> =
  z.object({ exports: z.array(consolePersistedAnalysisExportSchema) });

export const consoleAnalysisExportInventorySummaryResponseSchema: z.ZodType<ConsoleAnalysisExportInventorySummaryResponse> =
  z.object({
    summary: z.object({
      totalExports: z.number().int().nonnegative(),
      activeExports: z.number().int().nonnegative(),
      deletedExports: z.number().int().nonnegative(),
      pinnedExports: z.number().int().nonnegative(),
      expiringWithin24Hours: z.number().int().nonnegative(),
      expiredActiveExports: z.number().int().nonnegative(),
      totalSampleCount: z.number().int().nonnegative(),
      totalRequestArtifactCount: z.number().int().nonnegative(),
      totalResponseArtifactCount: z.number().int().nonnegative(),
      byStatus: consoleSummaryBucketsSchema,
      byTextMode: consoleSummaryBucketsSchema,
      byTag: consoleSummaryBucketsSchema,
      byProject: consoleSummaryBucketsSchema,
    }),
  });

const consoleAnalysisExportMetricDeltaSchema = z.object({
  leftValue: consoleNullableNumberSchema,
  rightValue: consoleNullableNumberSchema,
  deltaValue: consoleNullableNumberSchema,
});

const consoleAnalysisExportBucketDeltasSchema = z
  .array(
    z.object({
      key: z.string(),
      leftCount: z.number().int().nonnegative(),
      rightCount: z.number().int().nonnegative(),
      deltaCount: z.number().int(),
    }),
  )
  .nullish()
  .transform((value) => value ?? []);

export const consoleAnalysisExportDiffResponseSchema: z.ZodType<ConsoleAnalysisExportDiffResponse> =
  z.object({
    diff: z.object({
      leftExport: consolePersistedAnalysisExportSchema,
      rightExport: consolePersistedAnalysisExportSchema,
      overlapRequestCount: z.number().int().nonnegative(),
      leftOnlyRequestCount: z.number().int().nonnegative(),
      rightOnlyRequestCount: z.number().int().nonnegative(),
      sampleCount: consoleAnalysisExportMetricDeltaSchema,
      requestArtifactCount: consoleAnalysisExportMetricDeltaSchema,
      responseArtifactCount: consoleAnalysisExportMetricDeltaSchema,
      promptTokens: consoleAnalysisExportMetricDeltaSchema,
      completionTokens: consoleAnalysisExportMetricDeltaSchema,
      totalTokens: consoleAnalysisExportMetricDeltaSchema,
      byStatus: consoleAnalysisExportBucketDeltasSchema,
      byProtocolFamily: consoleAnalysisExportBucketDeltasSchema,
      byEndpointKind: consoleAnalysisExportBucketDeltasSchema,
      byResolvedModel: consoleAnalysisExportBucketDeltasSchema,
      byProviderAccount: consoleAnalysisExportBucketDeltasSchema,
    }),
  });
