import { z } from "zod";

import type {
  ConsoleCostOverviewResponse,
  ConsoleCredentialUsageResponse,
  ConsoleProviderCredentialModelStateResponse,
  ConsoleRuntimePressureResponse,
  ConsoleUsageAggregateResponse,
} from "../contracts";
import { consoleNullableStringSchema, consoleNullableTimestampSchema } from "./common";

const consoleUsageAggregateBucketSchema = z.object({
  bucketStart: z.string().min(1),
  bucketGranularity: z.string().min(1),
  projectId: z.string(),
  userId: z.string(),
  provider: z.string(),
  providerCredentialRef: z.string().min(1),
  model: z.string(),
  requestCount: z.number().int().nonnegative(),
  failureCount: z.number().int().nonnegative(),
  promptTokens: z.number().int().nonnegative(),
  completionTokens: z.number().int().nonnegative(),
  totalTokens: z.number().int().nonnegative(),
  cacheCreationInputTokens: z.number().int().nonnegative(),
  cacheReadInputTokens: z.number().int().nonnegative(),
  latencyMsSum: z.number().int().nonnegative(),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

export const consoleCredentialUsageResponseSchema: z.ZodType<ConsoleCredentialUsageResponse> =
  z.object({
    buckets: z.array(consoleUsageAggregateBucketSchema),
  });

export const consoleUsageAggregateResponseSchema: z.ZodType<ConsoleUsageAggregateResponse> =
  z.object({
    buckets: z.array(consoleUsageAggregateBucketSchema),
  });

export const consoleRuntimePressureResponseSchema: z.ZodType<ConsoleRuntimePressureResponse> =
  z.object({
    pressure: z.object({
      totalRunningRequests: z.number().int().nonnegative(),
      totalProjectConcurrency: z.number().int().nonnegative(),
      totalProviderConcurrency: z.number().int().nonnegative(),
      projects: z.array(
        z.object({
          projectId: z.string().min(1),
          displayName: z.string(),
          activeConcurrency: z.number().int().nonnegative(),
          runningRequestCount: z.number().int().nonnegative(),
        }),
      ),
      providers: z.array(
        z.object({
          providerAccountId: z.string().min(1),
          label: z.string(),
          status: z.string(),
          protocolFamily: z.string(),
          activeConcurrency: z.number().int().nonnegative(),
          concurrencyLimit: z
            .number()
            .int()
            .nonnegative()
            .nullish()
            .transform((value) => value ?? null),
          concurrencyAvailable: z
            .number()
            .int()
            .nonnegative()
            .nullish()
            .transform((value) => value ?? null),
          runningRequestCount: z.number().int().nonnegative(),
          breakerOpen: z.boolean(),
        }),
      ),
    }),
  });

const consolePriceRateSchema = z.object({
  promptMicrosPer1kTokens: z
    .number()
    .nullish()
    .transform((value) => value ?? null),
  completionMicrosPer1kTokens: z
    .number()
    .nullish()
    .transform((value) => value ?? null),
  currency: z.string(),
  configured: z.boolean(),
  source: z.string(),
});

const consoleNullablePriceRateSchema = consolePriceRateSchema
  .nullish()
  .transform((value) => value ?? null);

const consoleNullableMicrosSchema = z
  .number()
  .nullish()
  .transform((value) => value ?? null);

export const consoleCostOverviewResponseSchema: z.ZodType<ConsoleCostOverviewResponse> = z.object({
  overview: z.object({
    providerBuckets: z.array(
      z.object({
        providerAccountId: z.string().min(1),
        label: z.string(),
        adapter: z.string(),
        protocolFamily: z.string(),
        requestCount: z.number().int().nonnegative(),
        promptTokens: z.number().nonnegative(),
        completionTokens: z.number().nonnegative(),
        totalTokens: z.number().nonnegative(),
        estimatedMarketCostMicros: consoleNullableMicrosSchema,
        pricedModelCount: z.number().int().nonnegative(),
        unpricedModelCount: z.number().int().nonnegative(),
        lastRequestAt: consoleNullableTimestampSchema,
        models: z.array(
          z.object({
            model: z.string(),
            requestCount: z.number().int().nonnegative(),
            promptTokens: z.number().nonnegative(),
            completionTokens: z.number().nonnegative(),
            totalTokens: z.number().nonnegative(),
            marketRate: consoleNullablePriceRateSchema,
            estimatedMarketCostMicros: consoleNullableMicrosSchema,
            lastRequestAt: consoleNullableTimestampSchema,
          }),
        ),
      }),
    ),
    pricingEditors: z.array(
      z.object({
        providerAccountId: z.string().min(1),
        label: z.string(),
        adapter: z.string(),
        protocolFamily: z.string(),
        modelCount: z.number().int().nonnegative(),
        configuredModelCount: z.number().int().nonnegative(),
        rows: z.array(
          z.object({
            model: z.string(),
            marketRate: consoleNullablePriceRateSchema,
          }),
        ),
      }),
    ),
  }),
});

export const consoleProviderCredentialModelStateResponseSchema: z.ZodType<ConsoleProviderCredentialModelStateResponse> =
  z.object({
    states: z.array(
      z.object({
        id: z.string().min(1),
        providerAccountId: z.string().min(1),
        providerCredentialId: consoleNullableStringSchema,
        providerCredentialRef: consoleNullableStringSchema,
        protocolProfile: consoleNullableStringSchema,
        model: z.string(),
        status: z.string(),
        failureClass: consoleNullableStringSchema,
        failureScope: consoleNullableStringSchema,
        failureCount: z.number().int().nonnegative(),
        lastError: consoleNullableStringSchema,
        lastUpstreamStatus: z
          .number()
          .int()
          .nullish()
          .transform((value) => value ?? null),
        cooldownUntil: consoleNullableTimestampSchema,
        lastSuccessAt: consoleNullableTimestampSchema,
        lastFailureAt: consoleNullableTimestampSchema,
        updatedAt: z.string().min(1),
      }),
    ),
  });
