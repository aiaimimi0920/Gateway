import { z } from "zod";

import type { ConsoleGatewayReadinessResponse, ConsoleOperatorSummaryResponse } from "../contracts";
import {
  consoleJsonObjectSchema,
  consoleNullableStringSchema,
  consoleNullableTimestampSchema,
} from "./common";

const consoleDependencyReadinessSchema = z.object({
  configured: z.boolean(),
  required: z.boolean(),
  ready: z.boolean(),
  timedOut: z.boolean(),
  driver: z
    .string()
    .nullish()
    .transform((value) => value ?? null)
    .optional(),
});

const consoleReadinessDependenciesSchema = z.object({
  redis: consoleDependencyReadinessSchema,
  postgresql: consoleDependencyReadinessSchema,
  objectStorage: consoleDependencyReadinessSchema,
});

export const consoleGatewayReadinessResponseSchema: z.ZodType<ConsoleGatewayReadinessResponse> =
  z.object({
    readiness: z.object({
      ok: z.boolean(),
      checks: z.object({
        database: z.boolean(),
        databaseConfigured: z.boolean(),
        redis: z.boolean(),
        objectStorage: z.boolean(),
        apiKeySecret: z.boolean(),
        publicBaseUrl: z.boolean(),
        draining: z.boolean(),
      }),
      dependencies: consoleReadinessDependenciesSchema,
      draining: z.boolean(),
      drainStartedAt: consoleNullableTimestampSchema,
      drainReason: consoleNullableStringSchema,
      activeRequests: z.number().int().nonnegative(),
      providerStats: consoleJsonObjectSchema,
    }),
  });

export const consoleOperatorSummaryResponseSchema: z.ZodType<ConsoleOperatorSummaryResponse> =
  z.object({
    summary: z.object({
      schemaVersion: z.number().int(),
      generatedAt: z.string().min(1),
      build: z.object({
        name: z.string(),
        version: z.string(),
        target: z.object({ os: z.string(), arch: z.string() }),
        debugAssertions: z.boolean(),
      }),
      runtime: z.object({
        role: z.string(),
        processId: z.number().int(),
        port: z.number().int(),
      }),
      lifecycle: z.object({
        state: z.string(),
        draining: z.boolean(),
        activeRequests: z.number().int().nonnegative(),
        drainStartedAt: consoleNullableTimestampSchema,
        drainReason: consoleNullableStringSchema,
        shutdownRequested: z.boolean(),
        shutdownRequestedAt: consoleNullableTimestampSchema,
        shutdownReason: consoleNullableStringSchema,
      }),
      readiness: z.object({
        ok: z.boolean(),
        dependencies: consoleReadinessDependenciesSchema,
        configuration: z.object({
          apiKeySecret: z.boolean(),
          publicBaseUrl: z.boolean(),
        }),
      }),
      routing: z.object({
        configured: z.boolean(),
        providerCount: z.number().int().nonnegative(),
        routeCount: z.number().int().nonnegative(),
        publishedModelCount: z.number().int().nonnegative(),
      }),
      credentialCache: z.object({ entryCount: z.number().int().nonnegative() }),
      requestMetrics: z.object({
        requestsTotal: z.number().nonnegative(),
        requestErrorsTotal: z.number().nonnegative(),
        requestDrainRejectionsTotal: z.number().nonnegative(),
        requestInFlight: z.number(),
        requestDurationMsCount: z.number().nonnegative(),
        requestDurationMsSum: z.number().nonnegative(),
        rateLimitChecksTotal: z.number().nonnegative(),
        rateLimitRejectionsTotal: z.number().nonnegative(),
        rateLimitStoreFailuresTotal: z.number().nonnegative(),
      }),
      providerStats: consoleJsonObjectSchema,
    }),
  });
