import type { ConsoleAnomalyIncident, ConsoleAnomalyPolicy, ConsolePersistedAnalysisExport, ConsoleRemediationRun } from "../../api/contracts";
import { OPERATIONS_DEFAULT_REQUEST_FILTERS, type OperationsPanelState, type OperationsWorkspaceProps } from "./operations-contracts";
import { operationsRequestFixture, operationsSummaryFixture } from "./OperationsWorkspace.fixtures";

const timestamp = "2026-11-04T01:00:00Z";
const state = <T,>(data: T): OperationsPanelState<T> => ({ data, error: null, loading: false });

export function operationsIncidentFixture(id = "incident-a", status = "open"): ConsoleAnomalyIncident {
  return {
    id, status, code: id, fingerprint: id, summary: "Synthetic upstream failure", severity: "high",
    policyId: null, projectId: null, routePolicyId: null, tag: null, textMode: null, ownerUserId: null,
    followUpStatus: "pending", syncHitCount: 1, escalationStatus: "none", escalatedAt: null, escalationReason: null,
    latestNote: null, resolutionNote: null, lastActionAt: null, lastAlertAttemptAt: null, lastAlertedAt: null,
    lastAlertSeverity: null, alertDeliveryCount: 0, latestExportId: null, previousExportId: null,
    latestValue: null, previousValue: null, deltaValue: null, deltaRatio: null, thresholdValue: null,
    firstSeenAt: timestamp, lastSeenAt: timestamp, acknowledgedAt: null, resolvedAt: null,
    createdAt: timestamp, updatedAt: timestamp,
  };
}

function policyFixture(): ConsoleAnomalyPolicy {
  return {
    id: "policy-a", name: "Policy A", status: "active", projectId: null, routePolicyId: null, tag: null, textMode: null,
    profileKey: "default", thresholds: {}, autoSyncEnabled: true, autoSyncIntervalMinutes: 5,
    lastSyncedAt: null, lastSyncStatus: null, lastSyncError: null, nextSyncDueAt: timestamp, syncDue: true,
    autoEscalateEnabled: true, escalateSeverityThreshold: null, escalateAfterSyncCount: null,
    autoEscalateOwnerUserId: null, autoEscalateFollowUpStatus: null,
    autoRemediationEnabled: true, autoRemediationIntervalMinutes: null, autoRemediationDryRunFirst: true,
    autoRemediationActionKeys: ["retry-a"], autoRemediationMaxApplyRunsPerIncident: null,
    autoRemediationRequireAlertBeforeApply: false, autoRemediationFreezeOnProviderHealthDegrade: false,
    alertingEnabled: true, alertIntervalMinutes: 5, notifyOperatorsOnEscalation: true, notifyOwnerOnEscalation: false,
    createdAt: timestamp, updatedAt: timestamp,
  };
}

function exportFixture(index: number): ConsolePersistedAnalysisExport {
  return {
    exportId: `export-${index}`, label: index === 0 ? null : `Export ${index}`, tags: [], status: "active",
    createdAt: timestamp, updatedAt: timestamp, objectPrefix: "synthetic", sampleCount: 3,
    requestArtifactCount: 1, responseArtifactCount: 1, retentionExpiresAt: "2026-11-10T01:00:00Z",
    cleanedUpAt: null, lastCleanupError: null, files: [], manifest: {},
    filters: {
      projectId: null, routePolicyId: null, providerAccountId: null, sessionId: null, apiKeyId: null,
      responseId: null, protocolFamily: null, status: null, endpointKind: null, stream: null, errorCode: null,
      fallbackEligible: null, createdFrom: null, createdTo: null, artifactAvailable: null,
      limit: 50, textMode: "redacted", maxTextChars: 2000,
    },
  };
}

/** Typed synthetic observations, shared by UI regression and isolated browser verification. */
export function operationsWorkspaceFixture(overrides: Partial<OperationsWorkspaceProps> = {}): OperationsWorkspaceProps {
  const summary = operationsSummaryFixture();
  const incident = operationsIncidentFixture();
  const run: ConsoleRemediationRun = {
    id: "run-a", incidentId: "incident-very-long-stable-identifier", policyId: null, routePolicyId: null,
    actionKey: "retry-a", title: "Retry A", executionMode: "automatic", status: "succeeded", dryRun: true,
    actorUserId: "synthetic", note: null, input: null, result: null, errorSummary: null,
    createdAt: timestamp, completedAt: "2026-11-04T01:01:00Z",
  };
  const metric = { improvedRuns: 1, regressedRuns: 0, neutralRuns: 0, unavailableRuns: 0 };
  return {
    t: (_, en) => en, editorLocked: false, refreshing: false, onRefresh: () => {},
    pressure: state({
      totalRunningRequests: 1, totalProjectConcurrency: 1, totalProviderConcurrency: 1,
      providers: [{ providerAccountId: "provider-a", label: "Account A", status: "active", protocolFamily: "openai",
        activeConcurrency: 1, concurrencyLimit: 4, concurrencyAvailable: 3, runningRequestCount: 1, breakerOpen: false }],
      projects: [{ projectId: "project-a", displayName: "Project A", activeConcurrency: 1, runningRequestCount: 1 }],
    }),
    readiness: state({
      ok: true, dependencies: summary.readiness.dependencies, activeRequests: 1,
      draining: false, drainReason: null, drainStartedAt: null, providerStats: {},
      checks: { database: true, databaseConfigured: true, redis: false, objectStorage: true, apiKeySecret: true, publicBaseUrl: true, draining: false },
    }),
    operatorSummary: state(summary),
    requests: state([
      { ...operationsRequestFixture("request-a"), requestedModel: "Model A", stream: true, upstreamStatus: null },
      { ...operationsRequestFixture("request-b"), requestedModel: "Model B" },
    ]),
    requestSummary: state({ totalRequests: 2, completedCount: 0, failedCount: 2, cancelledCount: 0, runningCount: 0,
      fallbackEligibleFailures: 1, fallbackExhaustedFailures: 0, byStatus: [], byProviderAccount: [], byEndpointKind: [],
      byErrorCode: [{ value: "upstream_error", count: 2 }], providerAccounts: [] }),
    usageSummary: state({ queueDepth: -1, recentRequestCount: 2, recentFailureCount: 2, recentTotalTokens: 0,
      archiveFailureCount: 1, alerts: [{ code: "archive", severity: "warning", message: "Synthetic archive warning" }] }),
    promptCache: state({ totalRequests: 4, cacheHitRequests: 2, cacheCreationRequests: 1, clientMarkedRequests: 1,
      autoAppliedRequests: 1, cacheControlCoverageRequests: 2, totalTokensSaved: 100, totalCacheCreationInputTokens: 300,
      estimatedCostSavedUsd: 0.01, cacheHitRate: 0.5, cacheControlCoverageRate: 0.5, inputPricePerMillion: 1, cachedInputPricePerMillion: 0.1 }),
    incidents: state([incident, operationsIncidentFixture("incident-b")]),
    incidentSummary: state({ totalIncidents: 2, openIncidents: 2, acknowledgedIncidents: 0, resolvedIncidents: 0,
      escalatedIncidents: 0, byStatus: [], bySeverity: [], byCode: [], byFollowUpStatus: [], byEscalationStatus: [] }),
    alertQueue: state({ generatedAt: timestamp, limit: 50, dueOnly: false, incidentCount: 1, dueCount: 1,
      items: [{ incident, policy: null, routePolicy: null, alertIntervalMinutes: 5, alertDue: true,
        nextAlertDueAt: timestamp, notifyOperators: true, notifyOwner: false, alertLevel: 1,
        webhookSeverity: "warning", remediationActionKeys: ["retry-a"] }] }),
    policies: state([policyFixture()]),
    remediationQueue: state({ generatedAt: timestamp, limit: 50, dueOnly: false, itemCount: 1, dueCount: 1,
      items: [{ incident, policy: null, routePolicy: null,
        action: { actionKey: "retry-a", title: "Retry A", description: "Synthetic action", category: "retry", priority: "high",
          routePolicyId: null, executable: true, executionMode: "automatic", defaultExecutionInput: null, recommendedChanges: null },
        remediationDue: true, nextExecutionStatus: null, nextRunDueAt: timestamp, blockedReason: "Waiting for approval", latestRun: null }] }),
    remediationRuns: state([run]),
    remediationEffectiveness: state({ generatedAt: timestamp, windowMinutes: 15, totalRuns: 1, impactedRuns: 1, unavailableRuns: 0,
      byStatus: [], byExecutionMode: [], byActionKey: [], completionRate: metric, failureRate: metric, requestArtifactCoverage: metric,
      responseArtifactCoverage: metric, firstTokenLatencyMsAvg: metric, totalTokensPerSample: metric, actions: [] }),
    hotspots: state({ totalRateLimitedRequests: 2, byCode: [{ key: "429", count: 2 }], byProject: [], byRoutePolicyId: [],
      byApiKeyId: [], byRequestedModel: [], byResolvedModel: [], byEndpointKind: [] }),
    exportInventory: state({ totalExports: 21, activeExports: 21, deletedExports: 0, pinnedExports: 0,
      expiringWithin24Hours: 0, expiredActiveExports: 0, totalSampleCount: 63, totalRequestArtifactCount: 21,
      totalResponseArtifactCount: 21, byStatus: [], byTextMode: [], byTag: [], byProject: [] }),
    exports: state(Array.from({ length: 21 }, (_, index) => exportFixture(index))),
    requestFilters: { ...OPERATIONS_DEFAULT_REQUEST_FILTERS }, onRequestFiltersChange: () => {}, onApplyRequestFilters: () => {},
    incidentBusyId: null, onAcknowledgeIncident: () => {}, onResolveIncident: () => {},
    ...overrides,
  };
}
