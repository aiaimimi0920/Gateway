import { z } from "zod";

import type {
  ConsoleAccessAffinityResponse,
  ConsoleAccessBundle,
  ConsoleAccessCatalog,
  ConsoleAccessKey,
  ConsoleAccessKeyAggregateMembership,
  ConsoleAccessKeyBalance,
  ConsoleApiAccessRotation,
  ConsoleUserCredentialIssueResponse,
  ConsoleVerifiedUserCredential,
} from "../contracts";
import {
  consoleJsonObjectSchema,
  consoleNullableJsonObjectSchema,
  consoleNullableNumberSchema,
  consoleNullableStringSchema,
  consoleNullableTimestampSchema,
  consoleStringArraySchema,
} from "./common";

export const consoleAccessKeySchema: z.ZodType<ConsoleAccessKey> = z.object({
  id: z.string().min(1),
  ownerType: z.string(),
  ownerId: z.string(),
  resolvedProjectId: z.string(),
  resolvedTenantId: z.string(),
  keyKind: z.string(),
  status: z.string(),
  publicKeyPrefix: z.string(),
  displayName: z.string(),
  token: consoleNullableStringSchema,
  externalKey: consoleNullableStringSchema,
  rotatedFromAccessKeyId: consoleNullableStringSchema,
  legacyGatewayApiKeyId: consoleNullableStringSchema,
  legacyUserCredentialId: consoleNullableStringSchema,
  expiresAt: consoleNullableTimestampSchema,
  lastUsedAt: consoleNullableTimestampSchema,
  metadata: consoleNullableJsonObjectSchema,
  revokedAt: consoleNullableTimestampSchema,
  revokeReason: consoleNullableStringSchema,
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

export const consoleAccessBundleSchema: z.ZodType<ConsoleAccessBundle> = z.object({
  id: z.string().min(1),
  projectId: consoleNullableStringSchema,
  slug: z.string(),
  displayName: z.string(),
  billingMode: z.string(),
  status: z.string(),
  description: consoleNullableStringSchema,
  metadata: consoleNullableJsonObjectSchema,
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

export const consoleAccessKeyBalanceSchema: z.ZodType<ConsoleAccessKeyBalance> = z.object({
  accessKeyId: z.string().min(1),
  balanceMode: z.string(),
  status: z.string(),
  unlimitedUntil: consoleNullableTimestampSchema,
  periodStartsAt: consoleNullableTimestampSchema,
  periodEndsAt: consoleNullableTimestampSchema,
  totalTokens: consoleNullableNumberSchema,
  remainingTokens: consoleNullableNumberSchema,
  totalMessages: consoleNullableNumberSchema,
  remainingMessages: consoleNullableNumberSchema,
  updatedAt: z.string().min(1),
});

/** `/access/keys/:id/balance` answers `null` when no balance row exists yet. */
export const consoleNullableAccessKeyBalanceSchema: z.ZodType<ConsoleAccessKeyBalance | null> =
  consoleAccessKeyBalanceSchema.nullish().transform((value) => value ?? null);

export const consoleAccessKeyAggregateMembershipsSchema: z.ZodType<
  ConsoleAccessKeyAggregateMembership[]
> = z
  .array(
    z.object({
      aggregateAccessKeyId: z.string().min(1),
      memberAccessKeyId: z.string().min(1),
      priority: z.number().int(),
      createdAt: z.string().min(1),
    }),
  )
  .nullish()
  .transform((value) => value ?? []);

/** `/access/catalog` answers the view directly — no envelope. */
export const consoleAccessCatalogSchema: z.ZodType<ConsoleAccessCatalog> = z.object({
  providerCapabilities: z
    .array(
      z.object({
        id: z.string().min(1),
        providerAccountId: z.string(),
        modelCode: z.string(),
        endpointKind: z.string(),
        upstreamModel: consoleNullableStringSchema,
        enabled: z.boolean(),
        createdAt: z.string().min(1),
        updatedAt: z.string().min(1),
      }),
    )
    .nullish()
    .transform((value) => value ?? []),
  platformAccessRows: z
    .array(
      z.object({
        id: z.string().min(1),
        providerCapabilityId: z.string(),
        providerAccountId: z.string(),
        modelCode: z.string(),
        endpointKind: z.string(),
        upstreamModel: consoleNullableStringSchema,
        platformTier: z.string(),
        status: z.string(),
        operatorWeight: z.number().int(),
        routingPriority: z.number().int(),
        enabledForSale: z.boolean(),
        notes: consoleNullableStringSchema,
        createdAt: z.string().min(1),
        updatedAt: z.string().min(1),
      }),
    )
    .nullish()
    .transform((value) => value ?? []),
  bundles: z
    .array(consoleAccessBundleSchema)
    .nullish()
    .transform((value) => value ?? []),
  bundleItems: z
    .array(
      z.object({
        bundleId: z.string().min(1),
        platformAccessId: z.string().min(1),
        createdAt: z.string().min(1),
      }),
    )
    .nullish()
    .transform((value) => value ?? []),
  accessKeys: z
    .array(consoleAccessKeySchema)
    .nullish()
    .transform((value) => value ?? []),
  keyBundleBindings: z
    .array(
      z.object({
        accessKeyId: z.string().min(1),
        bundleId: z.string().min(1),
        createdAt: z.string().min(1),
      }),
    )
    .nullish()
    .transform((value) => value ?? []),
  balances: z
    .array(consoleAccessKeyBalanceSchema)
    .nullish()
    .transform((value) => value ?? []),
  aggregateMemberships: consoleAccessKeyAggregateMembershipsSchema,
});

export const consoleAccessAffinityResponseSchema: z.ZodType<ConsoleAccessAffinityResponse> =
  z.object({
    affinity: z
      .object({
        scope: z.string(),
        model: z.string(),
        requestingAccessKeyId: z.string(),
        sourceAccessKeyId: z.string(),
        platformAccessId: z.string(),
        providerAccountId: z.string(),
        realCredentialRef: consoleNullableStringSchema,
        expiresAt: consoleNullableTimestampSchema,
      })
      .nullish()
      .transform((value) => value ?? null),
  });

export const consoleUserCredentialIssueResponseSchema: z.ZodType<ConsoleUserCredentialIssueResponse> =
  z.object({
    success: z.boolean(),
    credential: z.object({
      id: z.string().min(1),
      credentialKey: z.string().min(1),
      expiresAt: z.string().min(1),
      scope: consoleStringArraySchema,
      userId: z.string(),
      projectId: z.string(),
      tenantId: z.string(),
      credentialType: z.string(),
    }),
    message: z.string(),
  });

/** `/user-credentials/verify` answers the view directly — no envelope. */
export const consoleVerifiedUserCredentialSchema: z.ZodType<ConsoleVerifiedUserCredential> =
  z.object({
    valid: z.boolean(),
    credential: z
      .object({
        id: z.string().min(1),
        userId: z.string(),
        projectId: z.string(),
        scope: consoleStringArraySchema,
        expiresAt: z.string().min(1),
        status: z.string(),
        tenantId: z.string(),
      })
      .nullish()
      .transform((value) => value ?? null),
    reason: z.string(),
  });

export const consoleApiAccessRotationSchema: z.ZodType<ConsoleApiAccessRotation> = z.object({
  projectId: z.string().min(1),
  tenantId: z.string().min(1),
  project: consoleJsonObjectSchema,
  tenant: consoleJsonObjectSchema,
  apiKey: consoleJsonObjectSchema,
  token: z.string().min(1),
});
