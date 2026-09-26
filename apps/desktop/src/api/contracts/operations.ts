// Alerting, anomaly, and remediation wire contracts.

import type { ConsoleJsonObject, ConsoleSummaryBucket } from "./operations-core";

export type ConsoleUsageAggregateAlert = {
  severity: string;
  code: string;
  message: string;
};

export type ConsoleUsageAggregateSummary = {
  /** `-1` when the queue depth could not be read (Redis unavailable). */
  queueDepth: number;
  recentRequestCount: number;
  recentFailureCount: number;
  recentTotalTokens: number;
  archiveFailureCount: number;
  alerts: ConsoleUsageAggregateAlert[];
};

export type ConsoleUsageAggregateSummaryResponse = {
  summary: ConsoleUsageAggregateSummary;
};

export type ConsolePromptCacheSummary = {
  totalRequests: number;
  cacheHitRequests: number;
  cacheCreationRequests: number;
  clientMarkedRequests: number;
  autoAppliedRequests: number;
  cacheControlCoverageRequests: number;
  totalTokensSaved: number;
  totalCacheCreationInputTokens: number;
  estimatedCostSavedUsd: number;
  cacheHitRate: number;
  cacheControlCoverageRate: number;
  inputPricePerMillion: number;
  cachedInputPricePerMillion: number;
};

export type ConsolePromptCacheSummaryResponse = {
  summary: ConsolePromptCacheSummary;
};

export type ConsoleRateLimitHotspotSummary = {
  totalRateLimitedRequests: number;
  byCode: ConsoleSummaryBucket[];
  byProject: ConsoleSummaryBucket[];
  byRoutePolicyId: ConsoleSummaryBucket[];
  byApiKeyId: ConsoleSummaryBucket[];
  byRequestedModel: ConsoleSummaryBucket[];
  byResolvedModel: ConsoleSummaryBucket[];
  byEndpointKind: ConsoleSummaryBucket[];
};

export type ConsoleRateLimitHotspotSummaryResponse = {
  summary: ConsoleRateLimitHotspotSummary;
};

export type ConsoleAnomalyIncident = {
  id: string;
  policyId: string | null;
  fingerprint: string;
  projectId: string | null;
  routePolicyId: string | null;
  tag: string | null;
  textMode: string | null;
  code: string;
  severity: string;
  status: string;
  ownerUserId: string | null;
  followUpStatus: string;
  syncHitCount: number;
  escalationStatus: string;
  escalatedAt: string | null;
  escalationReason: string | null;
  latestNote: string | null;
  resolutionNote: string | null;
  lastActionAt: string | null;
  lastAlertAttemptAt: string | null;
  lastAlertedAt: string | null;
  lastAlertSeverity: string | null;
  alertDeliveryCount: number;
  summary: string;
  latestExportId: string | null;
  previousExportId: string | null;
  latestValue: number | null;
  previousValue: number | null;
  deltaValue: number | null;
  deltaRatio: number | null;
  thresholdValue: number | null;
  firstSeenAt: string;
  lastSeenAt: string;
  acknowledgedAt: string | null;
  resolvedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ConsoleAnomalyIncidentResponse = {
  incidents: ConsoleAnomalyIncident[];
};

export type ConsoleAnomalyIncidentMutationResponse = {
  incident: ConsoleAnomalyIncident;
};

export type ConsoleAnomalyIncidentSummary = {
  totalIncidents: number;
  openIncidents: number;
  acknowledgedIncidents: number;
  resolvedIncidents: number;
  escalatedIncidents: number;
  byStatus: ConsoleSummaryBucket[];
  bySeverity: ConsoleSummaryBucket[];
  byCode: ConsoleSummaryBucket[];
  byFollowUpStatus: ConsoleSummaryBucket[];
  byEscalationStatus: ConsoleSummaryBucket[];
};

export type ConsoleAnomalyIncidentSummaryResponse = {
  summary: ConsoleAnomalyIncidentSummary;
};

export type ConsoleAnomalyIncidentHistoryEntry = {
  id: string;
  incidentId: string;
  eventType: string;
  actorUserId: string | null;
  note: string | null;
  metadata: ConsoleJsonObject | null;
  createdAt: string;
};

export type ConsoleAnomalyIncidentHistoryResponse = {
  history: ConsoleAnomalyIncidentHistoryEntry[];
};

export type ConsoleAlertQueueItem = {
  incident: ConsoleAnomalyIncident;
  policy: ConsoleJsonObject | null;
  routePolicy: ConsoleJsonObject | null;
  alertIntervalMinutes: number;
  alertDue: boolean;
  nextAlertDueAt: string | null;
  notifyOperators: boolean;
  notifyOwner: boolean;
  alertLevel: number;
  webhookSeverity: string;
  remediationActionKeys: string[];
};

export type ConsoleAlertQueue = {
  generatedAt: string;
  limit: number;
  dueOnly: boolean;
  incidentCount: number;
  dueCount: number;
  items: ConsoleAlertQueueItem[];
};

export type ConsoleAlertQueueResponse = {
  queue: ConsoleAlertQueue;
};

export type ConsoleAnomalyPolicy = {
  id: string;
  name: string;
  status: string;
  projectId: string | null;
  routePolicyId: string | null;
  tag: string | null;
  textMode: string | null;
  profileKey: string;
  thresholds: ConsoleJsonObject;
  autoSyncEnabled: boolean;
  autoSyncIntervalMinutes: number | null;
  lastSyncedAt: string | null;
  lastSyncStatus: string | null;
  lastSyncError: string | null;
  nextSyncDueAt: string | null;
  syncDue: boolean;
  autoEscalateEnabled: boolean;
  escalateSeverityThreshold: string | null;
  escalateAfterSyncCount: number | null;
  autoEscalateOwnerUserId: string | null;
  autoEscalateFollowUpStatus: string | null;
  autoRemediationEnabled: boolean;
  autoRemediationIntervalMinutes: number | null;
  autoRemediationDryRunFirst: boolean;
  autoRemediationActionKeys: string[];
  autoRemediationMaxApplyRunsPerIncident: number | null;
  autoRemediationRequireAlertBeforeApply: boolean;
  autoRemediationFreezeOnProviderHealthDegrade: boolean;
  alertingEnabled: boolean;
  alertIntervalMinutes: number | null;
  notifyOperatorsOnEscalation: boolean;
  notifyOwnerOnEscalation: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ConsoleAnomalyPolicyResponse = {
  policies: ConsoleAnomalyPolicy[];
};

export type ConsoleAnomalyPolicyMutationResponse = {
  policy: ConsoleAnomalyPolicy;
};

export type ConsoleRemediationAction = {
  actionKey: string;
  title: string;
  description: string;
  category: string;
  priority: string;
  routePolicyId: string | null;
  executable: boolean;
  executionMode: string;
  defaultExecutionInput: ConsoleJsonObject | null;
  recommendedChanges: ConsoleJsonObject | null;
};

export type ConsoleRemediationRun = {
  id: string;
  incidentId: string;
  policyId: string | null;
  routePolicyId: string | null;
  actionKey: string;
  title: string;
  executionMode: string;
  status: string;
  dryRun: boolean;
  actorUserId: string;
  note: string | null;
  input: ConsoleJsonObject | null;
  result: ConsoleJsonObject | null;
  errorSummary: string | null;
  createdAt: string;
  completedAt: string | null;
};

export type ConsoleRemediationRunResponse = {
  runs: ConsoleRemediationRun[];
};

export type ConsoleRemediationQueueItem = {
  incident: ConsoleAnomalyIncident;
  policy: ConsoleJsonObject | null;
  routePolicy: ConsoleJsonObject | null;
  action: ConsoleRemediationAction;
  remediationDue: boolean;
  nextExecutionStatus: string | null;
  nextRunDueAt: string | null;
  blockedReason: string | null;
  latestRun: ConsoleRemediationRun | null;
};

export type ConsoleRemediationQueue = {
  generatedAt: string;
  limit: number;
  dueOnly: boolean;
  itemCount: number;
  dueCount: number;
  items: ConsoleRemediationQueueItem[];
};

export type ConsoleRemediationQueueResponse = {
  queue: ConsoleRemediationQueue;
};

export type ConsoleRemediationMetric = {
  improvedRuns: number;
  regressedRuns: number;
  neutralRuns: number;
  unavailableRuns: number;
};

export type ConsoleRemediationActionEffectiveness = {
  actionKey: string;
  runCount: number;
  impactedRunCount: number;
  unavailableRunCount: number;
  completionRate: ConsoleRemediationMetric;
  failureRate: ConsoleRemediationMetric;
  requestArtifactCoverage: ConsoleRemediationMetric;
  responseArtifactCoverage: ConsoleRemediationMetric;
  firstTokenLatencyMsAvg: ConsoleRemediationMetric;
  totalTokensPerSample: ConsoleRemediationMetric;
};

export type ConsoleRemediationEffectiveness = {
  generatedAt: string;
  windowMinutes: number;
  totalRuns: number;
  impactedRuns: number;
  unavailableRuns: number;
  byStatus: ConsoleSummaryBucket[];
  byExecutionMode: ConsoleSummaryBucket[];
  byActionKey: ConsoleSummaryBucket[];
  completionRate: ConsoleRemediationMetric;
  failureRate: ConsoleRemediationMetric;
  requestArtifactCoverage: ConsoleRemediationMetric;
  responseArtifactCoverage: ConsoleRemediationMetric;
  firstTokenLatencyMsAvg: ConsoleRemediationMetric;
  totalTokensPerSample: ConsoleRemediationMetric;
  actions: ConsoleRemediationActionEffectiveness[];
};

export type ConsoleRemediationEffectivenessResponse = {
  effectiveness: ConsoleRemediationEffectiveness;
};
