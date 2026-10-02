// Account inventory, credential-pool, refill, and probe wire contracts.

export type ConsoleAccountGroupSummaryGroup = {
  id: string;
  name: string;
  description?: string | null;
  billingMultiplier: number;
  configuredBillingMultiplier?: number | null;
  enabled: boolean;
  notes?: string | null;
  memberCount: number;
  providerCredentialIds: string[];
  providers: string[];
};

export type ConsoleAccountGroupSummaryAccount = {
  id: string;
  displayName: string;
  providerId: string;
  providerLabel: string;
  /** Stable vendor identifier; omitted by legacy route documents. */
  vendorKey?: string | null;
  /** Human-readable vendor name; omitted by legacy route documents. */
  vendorName?: string | null;
  providerPreset?: string | null;
  credentialId?: string | null;
  baseUrl?: string | null;
  mode: string;
  /** Effective routing status; legacy summaries may omit this field. */
  enabled: boolean;
  supportedModels: string[];
  groupIds: string[];
};

export type ConsoleAccountGroupSummaryProvider = {
  id: string;
  label: string;
  /** Stable vendor identifier; omitted by legacy route documents. */
  vendorKey?: string | null;
  /** Human-readable vendor name; omitted by legacy route documents. */
  vendorName?: string | null;
  preset?: string | null;
  baseUrl?: string | null;
  accountIds: string[];
  supportedModels: string[];
};

export type ConsoleAccountGroupSummary = {
  routeConfigRevision: string;
  source: string;
  accountGroups: ConsoleAccountGroupSummaryGroup[];
  accounts: ConsoleAccountGroupSummaryAccount[];
  providers: ConsoleAccountGroupSummaryProvider[];
};

export type ConsoleAccountGroupSummaryResponse = {
  summary: ConsoleAccountGroupSummary;
};

export type ConsoleProviderQuotaWindow = {
  key: string;
  label: string;
  usedPercent: number | null;
  remainingRatio: number | null;
  limitWindowSeconds: number | null;
  resetAt: string | null;
  resetAfterSeconds: number | null;
};

export type ConsoleProviderQuota = {
  providerAccountId: string;
  providerCredentialId: string | null;
  providerType: string;
  source: string;
  status: string;
  ready: boolean;
  checkedAt: string;
  nextCheckAt: string;
  nextResetAt: string | null;
  planType: string | null;
  representativeClaim: string | null;
  windows: ConsoleProviderQuotaWindow[];
  error: string | null;
  rawData: unknown;
};

export type ConsoleProviderCredentialInventoryItem = {
  id: string;
  providerAccountId: string;
  label: string;
  status: string;
  credential: Record<string, unknown>;
  sourceKind: string;
  sourcePath: string | null;
  syncMode: string;
  syncState: string;
  providerQuota: ConsoleProviderQuota | null;
};

export type ConsoleProviderCredentialInventoryResponse = {
  credentials: ConsoleProviderCredentialInventoryItem[];
};

export type ConsoleCredentialPoolAutomationDriver = {
  id: string;
  mode: "script" | "http";
  providerIds: string[];
};

export type ConsoleCredentialPoolAutomationProvider = {
  providerId: string;
  providerLabel: string;
  targetSize: number;
  credentialCount: number;
  activeCredentialCount: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  permanentDeleteEnabled: boolean;
  driverId: string | null;
  driverMode: "script" | "http" | null;
  driverConfigured: boolean;
  state: "disabled" | "not_configured" | "idle" | "running" | "succeeded" | "failed";
  lastRunAt: string | null;
  nextRunAt: string | null;
  lastAction: string | null;
  createdCount: number;
  prunedCount: number;
  message: string | null;
  revisionId: string | null;
};

export type ConsoleCredentialPoolAutomationResponse = {
  automation: {
    enabled: boolean;
    intervalSeconds: number;
    drivers: ConsoleCredentialPoolAutomationDriver[];
    providers: ConsoleCredentialPoolAutomationProvider[];
    revisionId: string;
  };
};

export type ConsoleCredentialPoolAutomationRunResponse = {
  provider: ConsoleCredentialPoolAutomationProvider;
};

export type ConsoleCredentialArchivePurgeResponse = {
  purgedCount: number;
};

export type ConsoleCredentialRefillTrigger = "notification" | "inquiry" | "user_requested";
export type ConsoleCredentialRefillTaskState = "pending" | "claimed" | "succeeded" | "failed";
export type ConsoleCredentialRefillDeliveryMode =
  | "folder_sync"
  | "gateway_pull"
  | "direct_callback";

export type ConsoleCredentialRefillDemand = {
  providerId: string;
  providerLabel: string;
  targetSize: number;
  credentialCount: number;
  activeCredentialCount: number;
  deficit: number;
  needsRefill: boolean;
  autoRefillEnabled: boolean;
  directDriverConfigured: boolean;
  notificationEnabled: boolean;
  inquiryEnabled: boolean;
  userRequestEnabled: boolean;
  outstandingTaskId: string | null;
  outstandingTaskState: ConsoleCredentialRefillTaskState | null;
  notificationApi: string;
  inquiryApi: string;
  credentialStoragePath: string | null;
  storagePasswordConfigured: boolean;
  archiveStoragePath: string | null;
  archivedCredentialCount: number | null;
  archiveStorageError?: string | null;
  archivePurgeSupported?: boolean;
  archivePurgeUnsupportedReason?: string | null;
  storageAuthConfigured?: boolean;
  archiveAuthConfigured?: boolean;
  permanentDeleteEnabled: boolean;
  revisionId: string;
};

export type ConsoleCredentialRefillTask = {
  id: string;
  providerId: string;
  providerLabel: string;
  trigger: ConsoleCredentialRefillTrigger;
  state: ConsoleCredentialRefillTaskState;
  requestedCount: number;
  targetSize: number;
  activeCredentialCount: number;
  routeRevision: string;
  createdAt: string;
  updatedAt: string;
  workerId: string | null;
  leaseUntil: string | null;
  attempt: number;
  deliveryMode: ConsoleCredentialRefillDeliveryMode | null;
  createdCount: number;
  message: string | null;
  revisionId: string | null;
};

export type ConsoleCredentialRefillResponse = {
  refill: {
    enabled: boolean;
    streamKey: string | null;
    storageBackend?: "sqlite" | "redis";
    notificationIntervalSeconds: number;
    defaultLeaseSeconds: number;
    maxLeaseSeconds: number;
    revisionId: string;
    providers: ConsoleCredentialRefillDemand[];
    recentTasks: ConsoleCredentialRefillTask[];
  };
};

export type ConsoleCredentialRefillRequestResponse = {
  task: ConsoleCredentialRefillTask;
  created: boolean;
};

export type ConsoleCredentialProbeStatus = "passed" | "failed" | "unsupported";

export type ConsoleCredentialProbeResult = {
  credentialId: string;
  providerId: string;
  probePoint: string;
  status: ConsoleCredentialProbeStatus;
  message: string;
  checkedAt: string;
};

export type ConsoleCredentialProbeResponse = {
  result: ConsoleCredentialProbeResult;
};

export type ConsoleProviderProbeResult = {
  providerId: string;
  status: ConsoleCredentialProbeStatus;
  message: string;
  checkedAt: string;
  totalCount: number;
  passedCount: number;
  failedCount: number;
  unsupportedCount: number;
  results: ConsoleCredentialProbeResult[];
};

export type ConsoleProviderProbeResponse = {
  result: ConsoleProviderProbeResult;
};
