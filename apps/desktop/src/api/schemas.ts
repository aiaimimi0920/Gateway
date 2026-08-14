import { z } from "zod";
import type {
  BootstrapStatus,
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialPoolAutomationResponse,
  ConsoleCredentialPoolAutomationRunResponse,
  ConsoleCredentialRefillRequestResponse,
  ConsoleCredentialRefillResponse,
  ConsoleCredentialProbeResponse,
  ConsoleGeminiAuthSessionResponse,
  ConsoleEvent,
  ConsoleRouteConfigCommitResponse,
  ConsoleRouteConfigResponse,
  ConsoleRouteConfigValidationResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
  ManagementSession,
  OperationSuccess,
  PublicHealth,
  PublicModelList,
  PublicReadiness,
  SecretGrant,
} from "./contracts";

export const bootstrapStatusSchema: z.ZodType<BootstrapStatus> = z
  .object({
    needsBootstrap: z.boolean(),
    managementConfigured: z.boolean().optional(),
    environmentOverride: z.boolean().optional(),
  })
  .transform((value) => ({
    needsBootstrap: value.needsBootstrap,
    managementConfigured: value.managementConfigured ?? !value.needsBootstrap,
    environmentOverride: value.environmentOverride ?? false,
  }));

export const managementSessionSchema: z.ZodType<ManagementSession> = z.object({
  role: z.string().min(1),
  capabilities: z.array(z.string()),
  activeRevision: z.string().min(1).nullable(),
  secretAccessGranted: z.boolean(),
});

export const secretGrantSchema: z.ZodType<SecretGrant> = z.object({
  grant: z.string().min(1),
  expiresAt: z.string().min(1),
});

export const operationSuccessSchema: z.ZodType<OperationSuccess> = z
  .union([
    z.object({ success: z.literal(true), message: z.string().optional() }),
    z.object({ ok: z.literal(true), message: z.string().optional() }),
  ])
  .transform((value) => ({ success: true as const, message: value.message }));

export const consoleEventSchema: z.ZodType<ConsoleEvent> = z.object({
  id: z.union([z.string(), z.number()]).transform(String),
  kind: z.string().min(1),
  timestamp: z.string().min(1),
  message: z.string().optional(),
  data: z.record(z.string(), z.unknown()).optional().default({}),
});

const consoleRevisionMetadataSchema = z
  .object({
    id: z.string().min(1),
    sequence: z.number().int().nonnegative(),
    parent: z.string().min(1).nullable().optional(),
    actor: z.string().min(1).optional(),
    timestamp: z.string().min(1).optional(),
    documentDigest: z.string().min(1).optional(),
    yamlDigest: z.string().min(1).optional(),
    message: z.string().nullable().optional(),
  })
  .catchall(z.unknown());

const consoleSecretDescriptorSchema = z
  .object({
    path: z.string().min(1),
    kind: z.string().optional(),
    configured: z.boolean().optional(),
    preview: z.string().nullable().optional(),
    fingerprint: z.string().nullable().optional(),
  })
  .catchall(z.unknown());

const consoleRouteDocumentSchema = z
  .object({
    providers: z.array(z.unknown()),
    model_routes: z.array(z.unknown()),
    aliases: z.record(z.string(), z.string()),
  })
  .catchall(z.unknown());

const consoleDiagnosticsSchema = z.object({
  diagnostics: z.array(
    z.object({
      code: z.string().min(1),
      severity: z.string().min(1),
      path: z.string().min(1),
      message: z.string().min(1),
    }),
  ),
});

const consoleRouteConfigViewSchema = z.object({
  revision: consoleRevisionMetadataSchema,
  source: z.string().min(1),
  diagnostics: consoleDiagnosticsSchema.nullable(),
  requiresRepair: z.boolean(),
  document: consoleRouteDocumentSchema,
  secrets: z.array(consoleSecretDescriptorSchema),
  mutationSupported: z.boolean(),
});

const consoleAccountGroupSummaryGroupSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  description: z.string().nullable().optional(),
  billingMultiplier: z.number(),
  configuredBillingMultiplier: z.number().nullable().optional(),
  enabled: z.boolean(),
  notes: z.string().nullable().optional(),
  memberCount: z.number().int().nonnegative(),
  providerCredentialIds: z.array(z.string()),
  providers: z.array(z.string()),
});

const consoleAccountGroupSummaryAccountSchema = z.object({
  id: z.string().min(1),
  displayName: z.string().min(1),
  providerId: z.string().min(1),
  providerLabel: z.string().min(1),
  vendorKey: z.string().nullable().optional(),
  vendorName: z.string().nullable().optional(),
  providerPreset: z.string().nullable().optional(),
  credentialId: z.string().nullable().optional(),
  baseUrl: z.string().nullable().optional(),
  mode: z.string().min(1),
  // Older Gateway builds did not expose account status; preserve their
  // historical routable-by-default behavior while normalizing the result.
  enabled: z.boolean().default(true),
  supportedModels: z.array(z.string()),
  groupIds: z.array(z.string()),
});

const consoleAccountGroupSummaryProviderSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  vendorKey: z.string().nullable().optional(),
  vendorName: z.string().nullable().optional(),
  preset: z.string().nullable().optional(),
  baseUrl: z.string().nullable().optional(),
  accountIds: z.array(z.string()),
  supportedModels: z.array(z.string()),
});

export const consoleRouteConfigResponseSchema: z.ZodType<ConsoleRouteConfigResponse> = z.object({
  routeConfig: consoleRouteConfigViewSchema,
});

export const consoleRouteConfigCommitResponseSchema: z.ZodType<ConsoleRouteConfigCommitResponse> =
  z.object({
    routeConfig: consoleRouteConfigViewSchema,
    committed: z.boolean(),
  });

export const consoleRouteConfigValidationResponseSchema: z.ZodType<ConsoleRouteConfigValidationResponse> =
  z.object({
    validation: z.object({
      document: consoleRouteDocumentSchema,
      secrets: z.array(consoleSecretDescriptorSchema),
      diagnostics: consoleDiagnosticsSchema,
      requiresRepair: z.boolean(),
    }),
  });

export const consoleRouteRevisionListResponseSchema: z.ZodType<ConsoleRouteRevisionListResponse> =
  z.object({
    revisions: z.array(
      z.object({
        revision: consoleRevisionMetadataSchema,
        active: z.boolean(),
        hasArchive: z.boolean(),
        source: z.string().min(1),
      }),
    ),
  });

export const consoleRouteRevisionDetailResponseSchema: z.ZodType<ConsoleRouteRevisionDetailResponse> =
  z.object({
    routeConfig: consoleRouteConfigViewSchema,
    active: z.boolean(),
    hasArchive: z.boolean(),
  });

export const consoleAccountGroupSummaryResponseSchema: z.ZodType<ConsoleAccountGroupSummaryResponse> =
  z.object({
    summary: z.object({
      routeConfigRevision: z.string().min(1),
      source: z.string().min(1),
      accountGroups: z.array(consoleAccountGroupSummaryGroupSchema),
      accounts: z.array(consoleAccountGroupSummaryAccountSchema),
      providers: z.array(consoleAccountGroupSummaryProviderSchema),
    }),
  });

const consoleCredentialPoolAutomationProviderSchema = z.object({
  providerId: z.string().min(1),
  providerLabel: z.string().min(1),
  targetSize: z.number().int().positive(),
  credentialCount: z.number().int().nonnegative(),
  activeCredentialCount: z.number().int().nonnegative(),
  autoRefillEnabled: z.boolean(),
  autoPruneEnabled: z.boolean(),
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
      streamKey: z.string().min(1),
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

export const consoleCredentialProbeResponseSchema: z.ZodType<ConsoleCredentialProbeResponse> =
  z.object({
    result: z.object({
      credentialId: z.string().min(1),
      providerId: z.string().min(1),
      status: z.enum(["passed", "failed", "unsupported"]),
      message: z.string().min(1),
      checkedAt: z.string().min(1),
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

export const publicHealthSchema: z.ZodType<PublicHealth> = z
  .object({ status: z.string().min(1) })
  .catchall(z.unknown());

export const publicReadinessSchema: z.ZodType<PublicReadiness> = z
  .object({
    ready: z.boolean().optional(),
    status: z.string().min(1),
  })
  .catchall(z.unknown());

const publicModelSchema = z
  .object({
    id: z.string().min(1),
    object: z.string().optional(),
    created: z.number().optional(),
    owned_by: z.string().optional(),
    ownedBy: z.string().optional(),
  })
  .catchall(z.unknown());

export const publicModelListSchema: z.ZodType<PublicModelList> = z
  .object({
    object: z.string().optional(),
    data: z.array(publicModelSchema),
  })
  .catchall(z.unknown());

export const unknownObjectSchema = z.record(z.string(), z.unknown());
