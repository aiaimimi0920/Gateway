import { z } from "zod";

import type {
  ConsoleCredentialArchivePurgeResponse,
  ConsoleCredentialPoolAutomationResponse,
  ConsoleCredentialPoolAutomationRunResponse,
  ConsoleCredentialProbeResponse,
  ConsoleCredentialRefillRequestResponse,
  ConsoleCredentialRefillResponse,
  ConsoleGeminiAuthSessionResponse,
  ConsoleProviderCredentialInventoryResponse,
  ConsoleProviderProbeResponse,
} from "../contracts";

const consoleProviderQuotaWindowSchema = z.object({
  key: z.string().min(1),
  label: z.string().min(1),
  usedPercent: z.number().nullable(),
  remainingRatio: z.number().nullable(),
  limitWindowSeconds: z.number().int().nullable(),
  resetAt: z.string().nullable(),
  resetAfterSeconds: z.number().int().nullable(),
});

const consoleProviderQuotaSchema = z.object({
  providerAccountId: z.string().min(1),
  providerCredentialId: z.string().nullable(),
  providerType: z.string().min(1),
  source: z.string().min(1),
  status: z.string().min(1),
  ready: z.boolean(),
  checkedAt: z.string().min(1),
  nextCheckAt: z.string().min(1),
  nextResetAt: z.string().nullable(),
  planType: z.string().nullable(),
  representativeClaim: z.string().nullable(),
  windows: z.array(consoleProviderQuotaWindowSchema),
  error: z.string().nullable(),
  rawData: z.unknown(),
});

export const consoleProviderCredentialInventoryResponseSchema: z.ZodType<ConsoleProviderCredentialInventoryResponse> =
  z.object({
    credentials: z.array(
      z.object({
        id: z.string().min(1),
        providerAccountId: z.string().min(1),
        label: z.string().min(1),
        status: z.string().min(1),
        credential: z.record(z.string(), z.unknown()),
        sourceKind: z.string(),
        sourcePath: z.string().nullable(),
        syncMode: z.string(),
        syncState: z.string(),
        providerQuota: consoleProviderQuotaSchema.nullable(),
      }),
    ),
  });

const consoleCredentialPoolAutomationProviderSchema = z.object({
  providerId: z.string().min(1),
  providerLabel: z.string().min(1),
  targetSize: z.number().int().positive(),
  credentialCount: z.number().int().nonnegative(),
  activeCredentialCount: z.number().int().nonnegative(),
  autoRefillEnabled: z.boolean(),
  autoPruneEnabled: z.boolean(),
  permanentDeleteEnabled: z.boolean(),
  driverId: z.string().min(1).nullable(),
  driverMode: z.enum(["script", "http"]).nullable(),
  driverConfigured: z.boolean(),
  state: z.enum(["disabled", "not_configured", "idle", "running", "succeeded", "failed"]),
  lastRunAt: z.string().nullable(),
  nextRunAt: z.string().nullable(),
  lastAction: z.string().nullable(),
  createdCount: z.number().int().nonnegative(),
  prunedCount: z.number().int().nonnegative(),
  message: z.string().nullable(),
  revisionId: z.string().nullable(),
});

export const consoleCredentialPoolAutomationResponseSchema: z.ZodType<ConsoleCredentialPoolAutomationResponse> =
  z.object({
    automation: z.object({
      enabled: z.boolean(),
      intervalSeconds: z.number().int().positive(),
      drivers: z.array(
        z.object({
          id: z.string().min(1),
          mode: z.enum(["script", "http"]),
          providerIds: z.array(z.string().min(1)),
        }),
      ),
      providers: z.array(consoleCredentialPoolAutomationProviderSchema),
      revisionId: z.string().min(1),
    }),
  });

export const consoleCredentialPoolAutomationRunResponseSchema: z.ZodType<ConsoleCredentialPoolAutomationRunResponse> =
  z.object({ provider: consoleCredentialPoolAutomationProviderSchema });

export const consoleCredentialArchivePurgeResponseSchema: z.ZodType<ConsoleCredentialArchivePurgeResponse> =
  z.object({ purgedCount: z.number().int().nonnegative() });

const consoleCredentialRefillTaskStateSchema = z.enum([
  "pending",
  "claimed",
  "succeeded",
  "failed",
]);

const consoleCredentialRefillTaskSchema = z.object({
  id: z.string().min(1),
  providerId: z.string().min(1),
  providerLabel: z.string().min(1),
  trigger: z.enum(["notification", "inquiry", "user_requested"]),
  state: consoleCredentialRefillTaskStateSchema,
  requestedCount: z.number().int().positive(),
  targetSize: z.number().int().positive(),
  activeCredentialCount: z.number().int().nonnegative(),
  routeRevision: z.string().min(1),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
  workerId: z.string().min(1).nullable(),
  leaseUntil: z.string().min(1).nullable(),
  attempt: z.number().int().nonnegative(),
  deliveryMode: z.enum(["folder_sync", "gateway_pull", "direct_callback"]).nullable(),
  createdCount: z.number().int().nonnegative(),
  message: z.string().nullable(),
  revisionId: z.string().nullable(),
});

export const consoleCredentialRefillResponseSchema: z.ZodType<ConsoleCredentialRefillResponse> =
  z.object({
    refill: z.object({
      enabled: z.boolean(),
      streamKey: z.string().min(1).nullable(),
      storageBackend: z.enum(["sqlite", "redis"]).optional(),
      notificationIntervalSeconds: z.number().int().positive(),
      defaultLeaseSeconds: z.number().int().positive(),
      maxLeaseSeconds: z.number().int().positive(),
      revisionId: z.string().min(1),
      providers: z.array(
        z.object({
          providerId: z.string().min(1),
          providerLabel: z.string().min(1),
          targetSize: z.number().int().positive(),
          credentialCount: z.number().int().nonnegative(),
          activeCredentialCount: z.number().int().nonnegative(),
          deficit: z.number().int().nonnegative(),
          needsRefill: z.boolean(),
          autoRefillEnabled: z.boolean(),
          directDriverConfigured: z.boolean(),
          notificationEnabled: z.boolean(),
          inquiryEnabled: z.boolean(),
          userRequestEnabled: z.boolean(),
          outstandingTaskId: z.string().min(1).nullable(),
          outstandingTaskState: consoleCredentialRefillTaskStateSchema.nullable(),
          notificationApi: z.string().min(1),
          inquiryApi: z.string().min(1),
          credentialStoragePath: z.string().min(1).nullable(),
          storagePasswordConfigured: z.boolean(),
          archiveStoragePath: z.string().min(1).nullable(),
          archivedCredentialCount: z.number().int().nonnegative(),
          permanentDeleteEnabled: z.boolean(),
          revisionId: z.string().min(1),
        }),
      ),
      recentTasks: z.array(consoleCredentialRefillTaskSchema),
    }),
  });

export const consoleCredentialRefillRequestResponseSchema: z.ZodType<ConsoleCredentialRefillRequestResponse> =
  z.object({
    task: consoleCredentialRefillTaskSchema,
    created: z.boolean(),
  });

const consoleCredentialProbeResultSchema = z.object({
  credentialId: z.string().min(1),
  providerId: z.string().min(1),
  probePoint: z.string().min(1),
  status: z.enum(["passed", "failed", "unsupported"]),
  message: z.string().min(1),
  checkedAt: z.string().min(1),
});

export const consoleCredentialProbeResponseSchema: z.ZodType<ConsoleCredentialProbeResponse> =
  z.object({ result: consoleCredentialProbeResultSchema });

export const consoleProviderProbeResponseSchema: z.ZodType<ConsoleProviderProbeResponse> =
  z.object({
    result: z.object({
      providerId: z.string().min(1),
      status: z.enum(["passed", "failed", "unsupported"]),
      message: z.string().min(1),
      checkedAt: z.string().min(1),
      totalCount: z.number().int().nonnegative(),
      passedCount: z.number().int().nonnegative(),
      failedCount: z.number().int().nonnegative(),
      unsupportedCount: z.number().int().nonnegative(),
      results: z.array(consoleCredentialProbeResultSchema),
    }),
  });

const consoleGeminiAuthSecretEditSchema = z.object({
  field: z.enum(["api_key", "auth_token"]),
  operation: z.literal("replace"),
  value: z.string().min(1),
});

const consoleGeminiGeneratedCredentialDraftSchema = z.object({
  providerId: z.string().min(1),
  credential: z.record(z.string(), z.unknown()),
  secretEdits: z.array(consoleGeminiAuthSecretEditSchema),
});

export const consoleGeminiAuthSessionResponseSchema: z.ZodType<ConsoleGeminiAuthSessionResponse> =
  z.object({
    session: z.object({
      id: z.string().min(1),
      targetFamily: z.enum([
        "gemini-canvas",
        "gemini-canvas-chat",
        "gemini-business",
        "gemini-web",
      ]),
      providerId: z.string().min(1),
      status: z.enum(["pending", "waiting_user", "succeeded", "failed"]),
      message: z.string().min(1),
      createdAt: z.string().min(1),
      updatedAt: z.string().min(1),
      generatedDrafts: z.array(consoleGeminiGeneratedCredentialDraftSchema),
    }),
  });
