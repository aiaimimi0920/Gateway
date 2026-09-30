// Usage, pressure, cost, credential health, and audit-summary contracts.

export type ConsoleUsageAggregateBucket = {
  bucketStart: string;
  bucketGranularity: string;
  projectId: string;
  userId: string;
  provider: string;
  providerCredentialRef: string;
  model: string;
  requestCount: number;
  failureCount: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
  latencyMsSum: number;
  createdAt: string;
  updatedAt: string;
};

export type ConsoleCredentialUsageResponse = {
  buckets: ConsoleUsageAggregateBucket[];
};

export type ConsoleUsageAggregateResponse = {
  buckets: ConsoleUsageAggregateBucket[];
};

export type ConsoleRuntimePressureProject = {
  projectId: string;
  displayName: string;
  activeConcurrency: number;
  runningRequestCount: number;
};

export type ConsoleRuntimePressureProvider = {
  providerAccountId: string;
  label: string;
  status: string;
  protocolFamily: string;
  activeConcurrency: number;
  concurrencyLimit: number | null;
  concurrencyAvailable: number | null;
  runningRequestCount: number;
  breakerOpen: boolean;
};

export type ConsoleRuntimePressure = {
  totalRunningRequests: number;
  totalProjectConcurrency: number;
  totalProviderConcurrency: number;
  projects: ConsoleRuntimePressureProject[];
  providers: ConsoleRuntimePressureProvider[];
};

export type ConsoleRuntimePressureResponse = {
  pressure: ConsoleRuntimePressure;
};

export type ConsolePriceRate = {
  promptMicrosPer1kTokens: number | null;
  completionMicrosPer1kTokens: number | null;
  currency: string;
  configured: boolean;
  source: string;
};

export type ConsoleCostModelRow = {
  model: string;
  requestCount: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  marketRate: ConsolePriceRate | null;
  estimatedMarketCostMicros: number | null;
  lastRequestAt: string | null;
};

export type ConsoleCostProviderBucket = {
  providerAccountId: string;
  label: string;
  adapter: string;
  protocolFamily: string;
  requestCount: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  estimatedMarketCostMicros: number | null;
  pricedModelCount: number;
  unpricedModelCount: number;
  lastRequestAt: string | null;
  models: ConsoleCostModelRow[];
};

export type ConsoleCostPricingEditorRow = {
  model: string;
  marketRate: ConsolePriceRate | null;
};

export type ConsoleCostPricingEditor = {
  providerAccountId: string;
  label: string;
  adapter: string;
  protocolFamily: string;
  modelCount: number;
  configuredModelCount: number;
  rows: ConsoleCostPricingEditorRow[];
};

export type ConsoleCostOverview = {
  providerBuckets: ConsoleCostProviderBucket[];
  pricingEditors: ConsoleCostPricingEditor[];
};

export type ConsoleCostOverviewResponse = {
  overview: ConsoleCostOverview;
};

/** Health of one provider credential for one model, as recorded by the pipeline. */
export type ConsoleProviderCredentialModelState = {
  id: string;
  providerAccountId: string;
  providerCredentialId: string | null;
  /** Route-document credential id (or platform access id) that served the call. */
  providerCredentialRef: string | null;
  protocolProfile: string | null;
  model: string;
  status: string;
  failureClass: string | null;
  failureScope: string | null;
  failureCount: number;
  lastError: string | null;
  lastUpstreamStatus: number | null;
  cooldownUntil: string | null;
  lastSuccessAt: string | null;
  lastFailureAt: string | null;
  updatedAt: string;
};

export type ConsoleProviderCredentialModelStateResponse = {
  states: ConsoleProviderCredentialModelState[];
};

export type ConsoleRequestAuditProviderWindow = {
  label: string;
  bucketStart: string;
  totalRequests: number;
  successCount: number;
  failureCount: number;
};

/**
 * One provider account's traffic for a single upstream model. Keyed the way the
 * cost overview keys its model rows, so per-model requests and per-model money
 * can be joined without guessing.
 */
export type ConsoleRequestAuditProviderModelStats = {
  model: string;
  totalRequests: number;
  completedCount: number;
  failedCount: number;
  cancelledCount: number;
  runningCount: number;
  lastRequestAt: string | null;
  windows: ConsoleRequestAuditProviderWindow[];
};

export type ConsoleRequestAuditProviderStats = {
  providerAccountId: string;
  totalRequests: number;
  completedCount: number;
  failedCount: number;
  cancelledCount: number;
  runningCount: number;
  lastRequestAt: string | null;
  windows: ConsoleRequestAuditProviderWindow[];
  models: ConsoleRequestAuditProviderModelStats[];
};

export type ConsoleRequestAuditSummary = {
  totalRequests: number;
  completedCount: number;
  failedCount: number;
  cancelledCount: number;
  runningCount: number;
  providerAccounts: ConsoleRequestAuditProviderStats[];
  credentials?: (ConsoleRequestAuditProviderStats & { credentialRef: string })[];
};

export type ConsoleRequestAuditSummaryResponse = {
  summary: ConsoleRequestAuditSummary;
};
