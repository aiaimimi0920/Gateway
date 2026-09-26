import { z } from "zod";

import type {
  ConsoleAnalysisSampleResponse,
  ConsolePromptCacheSummaryResponse,
  ConsoleRateLimitHotspotSummaryResponse,
  ConsoleRequestAuditFullSummaryResponse,
  ConsoleRequestAuditResponse,
  ConsoleRequestAuditSummaryResponse,
  ConsoleUsageAggregateSummaryResponse,
} from "../contracts";
import {
  consoleNullableJsonObjectSchema,
  consoleNullableNumberSchema,
  consoleNullableStringSchema,
  consoleNullableTimestampSchema,
  consoleSummaryBucketsSchema,
  consoleValueBucketsSchema,
} from "./common";

const consoleRequestAuditProviderWindowsSchema = z
  .array(
    z.object({
      label: z.string(),
      bucketStart: z.string(),
      totalRequests: z.number().int().nonnegative(),
      successCount: z.number().int().nonnegative(),
      failureCount: z.number().int().nonnegative(),
    }),
  )
  .nullish()
  .transform((value) => value ?? []);

const consoleRequestAuditProviderAccountsSchema = z
  .array(
    z.object({
      providerAccountId: z.string().min(1),
      totalRequests: z.number().int().nonnegative(),
      completedCount: z.number().int().nonnegative(),
      failedCount: z.number().int().nonnegative(),
      cancelledCount: z.number().int().nonnegative(),
      runningCount: z.number().int().nonnegative(),
      lastRequestAt: consoleNullableTimestampSchema,
      windows: consoleRequestAuditProviderWindowsSchema,
      models: z
        .array(
          z.object({
            model: z.string().min(1),
            totalRequests: z.number().int().nonnegative(),
            completedCount: z.number().int().nonnegative(),
            failedCount: z.number().int().nonnegative(),
            cancelledCount: z.number().int().nonnegative(),
            runningCount: z.number().int().nonnegative(),
            lastRequestAt: consoleNullableTimestampSchema,
            windows: consoleRequestAuditProviderWindowsSchema,
          }),
        )
        // Older gateways answer without the per-model breakdown.
        .nullish()
        .transform((value) => value ?? []),
    }),
  )
  // Older gateways answer without the per-provider breakdown.
  .nullish()
  .transform((value) => value ?? []);

export const consoleRequestAuditSummaryResponseSchema: z.ZodType<ConsoleRequestAuditSummaryResponse> =
  z.object({
    summary: z.object({
      totalRequests: z.number().int().nonnegative(),
      completedCount: z.number().int().nonnegative(),
      failedCount: z.number().int().nonnegative(),
      cancelledCount: z.number().int().nonnegative(),
      runningCount: z.number().int().nonnegative(),
      providerAccounts: consoleRequestAuditProviderAccountsSchema,
    }),
  });

export const consoleRequestAuditResponseSchema: z.ZodType<ConsoleRequestAuditResponse> = z.object({
  requests: z.array(
    z.object({
      id: z.string().min(1),
      projectId: z.string(),
      apiKeyId: consoleNullableStringSchema,
      userCredentialId: consoleNullableStringSchema,
      accessKeyId: consoleNullableStringSchema,
      sourceAccessKeyId: consoleNullableStringSchema,
      sessionId: consoleNullableStringSchema,
      routePolicyId: consoleNullableStringSchema,
      providerAccountId: consoleNullableStringSchema,
      protocolFamily: z.string(),
      endpointKind: z.string(),
      requestedModel: consoleNullableStringSchema,
      resolvedModel: consoleNullableStringSchema,
      modelAlias: consoleNullableStringSchema,
      stream: z.boolean(),
      status: z.string(),
      upstreamStatus: consoleNullableNumberSchema,
      durationMs: consoleNullableNumberSchema,
      promptTokens: consoleNullableNumberSchema,
      completionTokens: consoleNullableNumberSchema,
      totalTokens: consoleNullableNumberSchema,
      cacheCreationInputTokens: consoleNullableNumberSchema,
      cacheReadInputTokens: consoleNullableNumberSchema,
      clientHasCacheControl: z.boolean(),
      autoCacheApplied: z.boolean(),
      errorSummary: consoleNullableStringSchema,
      routeTrace: consoleNullableJsonObjectSchema,
      analysisProfile: consoleNullableJsonObjectSchema,
      requestArtifactObjectKey: consoleNullableStringSchema,
      responseArtifactObjectKey: consoleNullableStringSchema,
      responseId: z.string(),
      previousResponseId: consoleNullableStringSchema,
      clientDisconnectedAt: consoleNullableTimestampSchema,
      createdAt: z.string().min(1),
      completedAt: consoleNullableTimestampSchema,
      updatedAt: z.string().min(1),
    }),
  ),
});

export const consoleAnalysisSampleResponseSchema: z.ZodType<ConsoleAnalysisSampleResponse> =
  z.object({
    samples: z.array(
      z.object({
        requestAuditId: z.string().min(1),
        responseId: z.string(),
        projectId: z.string(),
        routePolicyId: consoleNullableStringSchema,
        sessionId: consoleNullableStringSchema,
        providerAccountId: consoleNullableStringSchema,
        protocolFamily: z.string(),
        endpointKind: z.string(),
        requestedModel: consoleNullableStringSchema,
        resolvedModel: consoleNullableStringSchema,
        status: z.string(),
        stream: z.boolean(),
        createdAt: z.string().min(1),
        completedAt: consoleNullableTimestampSchema,
        promptTokens: consoleNullableNumberSchema,
        completionTokens: consoleNullableNumberSchema,
        totalTokens: consoleNullableNumberSchema,
        cacheCreationInputTokens: consoleNullableNumberSchema,
        cacheReadInputTokens: consoleNullableNumberSchema,
        analysisProfile: consoleNullableJsonObjectSchema,
        requestArtifactObjectKey: consoleNullableStringSchema,
        responseArtifactObjectKey: consoleNullableStringSchema,
        routeTrace: consoleNullableJsonObjectSchema,
      }),
    ),
  });

export const consoleRequestAuditFullSummaryResponseSchema: z.ZodType<ConsoleRequestAuditFullSummaryResponse> =
  z.object({
    summary: z.object({
      totalRequests: z.number().int().nonnegative(),
      completedCount: z.number().int().nonnegative(),
      failedCount: z.number().int().nonnegative(),
      cancelledCount: z.number().int().nonnegative(),
      runningCount: z.number().int().nonnegative(),
      fallbackEligibleFailures: z.number().int().nonnegative(),
      fallbackExhaustedFailures: z.number().int().nonnegative(),
      byStatus: consoleValueBucketsSchema,
      byProviderAccount: consoleValueBucketsSchema,
      byEndpointKind: consoleValueBucketsSchema,
      byErrorCode: consoleValueBucketsSchema,
      providerAccounts: consoleRequestAuditProviderAccountsSchema,
    }),
  });

export const consoleUsageAggregateSummaryResponseSchema: z.ZodType<ConsoleUsageAggregateSummaryResponse> =
  z.object({
    summary: z.object({
      // `-1` signals "queue depth unavailable", so this cannot be `.nonnegative()`.
      queueDepth: z.number().int(),
      recentRequestCount: z.number().int().nonnegative(),
      recentFailureCount: z.number().int().nonnegative(),
      recentTotalTokens: z.number().int().nonnegative(),
      archiveFailureCount: z.number().int().nonnegative(),
      alerts: z
        .array(
          z.object({
            severity: z.string(),
            code: z.string(),
            message: z.string(),
          }),
        )
        .nullish()
        .transform((value) => value ?? []),
    }),
  });

export const consolePromptCacheSummaryResponseSchema: z.ZodType<ConsolePromptCacheSummaryResponse> =
  z.object({
    summary: z.object({
      totalRequests: z.number().int().nonnegative(),
      cacheHitRequests: z.number().int().nonnegative(),
      cacheCreationRequests: z.number().int().nonnegative(),
      clientMarkedRequests: z.number().int().nonnegative(),
      autoAppliedRequests: z.number().int().nonnegative(),
      cacheControlCoverageRequests: z.number().int().nonnegative(),
      totalTokensSaved: z.number().int(),
      totalCacheCreationInputTokens: z.number().int(),
      estimatedCostSavedUsd: z.number(),
      cacheHitRate: z.number(),
      cacheControlCoverageRate: z.number(),
      inputPricePerMillion: z.number(),
      cachedInputPricePerMillion: z.number(),
    }),
  });

export const consoleRateLimitHotspotSummaryResponseSchema: z.ZodType<ConsoleRateLimitHotspotSummaryResponse> =
  z.object({
    summary: z.object({
      totalRateLimitedRequests: z.number().int().nonnegative(),
      byCode: consoleSummaryBucketsSchema,
      byProject: consoleSummaryBucketsSchema,
      byRoutePolicyId: consoleSummaryBucketsSchema,
      byApiKeyId: consoleSummaryBucketsSchema,
      byRequestedModel: consoleSummaryBucketsSchema,
      byResolvedModel: consoleSummaryBucketsSchema,
      byEndpointKind: consoleSummaryBucketsSchema,
    }),
  });
