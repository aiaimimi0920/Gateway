// Access-key, bundle, affinity, and user-credential wire contracts.

import type { ConsoleJsonObject } from "./operations-core";

/* ------------------------------------------------------------------ *
 * Access keys workspace (访问密钥).
 * ------------------------------------------------------------------ */

export type ConsoleProviderCapability = {
  id: string;
  providerAccountId: string;
  modelCode: string;
  endpointKind: string;
  upstreamModel: string | null;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ConsolePlatformAccess = {
  id: string;
  providerCapabilityId: string;
  providerAccountId: string;
  modelCode: string;
  endpointKind: string;
  upstreamModel: string | null;
  platformTier: string;
  status: string;
  operatorWeight: number;
  routingPriority: number;
  enabledForSale: boolean;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ConsoleAccessBundle = {
  id: string;
  projectId: string | null;
  slug: string;
  displayName: string;
  billingMode: string;
  status: string;
  description: string | null;
  metadata: ConsoleJsonObject | null;
  createdAt: string;
  updatedAt: string;
};

export type ConsoleAccessBundleItem = {
  bundleId: string;
  platformAccessId: string;
  createdAt: string;
};

export type ConsoleAccessKeyBundleBinding = {
  accessKeyId: string;
  bundleId: string;
  createdAt: string;
};

export type ConsoleAccessKey = {
  id: string;
  ownerType: string;
  ownerId: string;
  resolvedProjectId: string;
  resolvedTenantId: string;
  keyKind: string;
  status: string;
  publicKeyPrefix: string;
  displayName: string;
  /** Only returned on create/rotate — never persisted in the catalog. */
  token: string | null;
  externalKey: string | null;
  rotatedFromAccessKeyId: string | null;
  legacyGatewayApiKeyId: string | null;
  legacyUserCredentialId: string | null;
  expiresAt: string | null;
  lastUsedAt: string | null;
  metadata: ConsoleJsonObject | null;
  revokedAt: string | null;
  revokeReason: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ConsoleAccessKeyBalance = {
  accessKeyId: string;
  balanceMode: string;
  status: string;
  unlimitedUntil: string | null;
  periodStartsAt: string | null;
  periodEndsAt: string | null;
  totalTokens: number | null;
  remainingTokens: number | null;
  totalMessages: number | null;
  remainingMessages: number | null;
  cash?: {
    currency: "USD";
    totalMicros: number;
    spentMicros: number;
    reservedMicros: number;
    pendingRequests: number;
  };
  updatedAt: string;
};

export type ConsoleAccessKeyAggregateMembership = {
  aggregateAccessKeyId: string;
  memberAccessKeyId: string;
  priority: number;
  createdAt: string;
};

/** `/access/catalog` answers this view directly, with no envelope. */
export type ConsoleAccessKeyGroup = {
  id: string;
  name: string;
  enabled: boolean;
  memberCount: number;
};

export type ConsoleAccessCatalog = {
  /** Absent on older gateways; never infer local rights from the browser URL. */
  storageMode?: "local" | "server";
  cashQuotaSupported?: boolean;
  accountGroups?: ConsoleAccessKeyGroup[];
  providerCapabilities: ConsoleProviderCapability[];
  platformAccessRows: ConsolePlatformAccess[];
  bundles: ConsoleAccessBundle[];
  bundleItems: ConsoleAccessBundleItem[];
  accessKeys: ConsoleAccessKey[];
  keyBundleBindings: ConsoleAccessKeyBundleBinding[];
  balances: ConsoleAccessKeyBalance[];
  aggregateMemberships: ConsoleAccessKeyAggregateMembership[];
};

export type ConsoleAccessStickyAffinity = {
  scope: string;
  model: string;
  requestingAccessKeyId: string;
  sourceAccessKeyId: string;
  platformAccessId: string;
  providerAccountId: string;
  realCredentialRef: string | null;
  expiresAt: string | null;
};

export type ConsoleAccessAffinityResponse = {
  affinity: ConsoleAccessStickyAffinity | null;
};

export type ConsoleAccessKeyInput = {
  quota?: {
    mode: "unlimited" | "message_prepaid" | "token_prepaid" | "cash_prepaid";
    limit?: number | null;
    currency?: "USD";
  };
  ownerType: string;
  ownerId: string;
  resolvedProjectId: string;
  resolvedTenantId: string;
  keyKind: string;
  publicKeyPrefix: string;
  displayName: string;
  expiresAt?: string | null;
  metadata?: ConsoleJsonObject | null;
  bundleIds?: string[];
};

export type ConsoleAccessBundleInput = {
  projectId?: string | null;
  slug: string;
  displayName: string;
  billingMode?: string | null;
  status: string;
  description?: string | null;
  metadata?: ConsoleJsonObject | null;
};

export type ConsoleIssuedUserCredential = {
  id: string;
  credentialKey: string;
  expiresAt: string;
  scope: string[];
  userId: string;
  projectId: string;
  tenantId: string;
  credentialType: string;
};

export type ConsoleUserCredentialIssueResponse = {
  success: boolean;
  credential: ConsoleIssuedUserCredential;
  message: string;
};

export type ConsoleUserCredentialCacheEntry = {
  id: string;
  userId: string;
  projectId: string;
  scope: string[];
  expiresAt: string;
  status: string;
  tenantId: string;
};

/** `/user-credentials/verify` answers this view directly, with no envelope. */
export type ConsoleVerifiedUserCredential = {
  valid: boolean;
  credential: ConsoleUserCredentialCacheEntry | null;
  reason: string;
};

export type ConsoleUserCredentialIssueInput = {
  userId: string;
  projectId?: string | null;
  credentialType: string;
  durationDays: number;
  scope: string[];
  metadata?: ConsoleJsonObject | null;
};

export type ConsoleApiAccessRotation = {
  projectId: string;
  tenantId: string;
  project: ConsoleJsonObject;
  tenant: ConsoleJsonObject;
  apiKey: ConsoleJsonObject;
  token: string;
};
