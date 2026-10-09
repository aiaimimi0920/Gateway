// Shared operations views and request-audit contracts.

import type { ConsoleRequestAuditProviderStats } from "./telemetry";

/* ------------------------------------------------------------------ *
 * Operations workspace (运维): readiness, request audits, anomalies.
 * ------------------------------------------------------------------ */

/** Free-form JSON payload the gateway stores verbatim (route traces, metadata, thresholds). */
export type ConsoleJsonObject = Record<string, unknown>;

/** `{key, count}` bucket used by every summary endpoint. */
export type ConsoleSummaryBucket = {
  key: string;
  count: number;
};

/** `{value, count}` bucket used by the request-audit summary. */
export type ConsoleValueBucket = {
  value: string;
  count: number;
};

export type ConsoleDependencyReadiness = {
  configured: boolean;
  required: boolean;
  ready: boolean;
  timedOut: boolean;
  /** Only present on the object-storage dependency. */
  driver?: string | null;
};

export type ConsoleGatewayReadiness = {
  ok: boolean;
  checks: {
    database: boolean;
    databaseConfigured: boolean;
    redis: boolean;
    objectStorage: boolean;
    apiKeySecret: boolean;
    publicBaseUrl: boolean;
    draining: boolean;
  };
  dependencies: {
    sqlite?: ConsoleDependencyReadiness | null;
    redis: ConsoleDependencyReadiness;
    postgresql: ConsoleDependencyReadiness;
    objectStorage: ConsoleDependencyReadiness;
  };
  draining: boolean;
  drainStartedAt: string | null;
  drainReason: string | null;
  activeRequests: number;
  providerStats: ConsoleJsonObject;
};

export type ConsoleGatewayReadinessResponse = {
  readiness: ConsoleGatewayReadiness;
};

export type ConsoleOperatorSummary = {
  schemaVersion: number;
  generatedAt: string;
  build: {
    name: string;
    version: string;
    target: { os: string; arch: string };
    debugAssertions: boolean;
  };
  runtime: { role: string; processId: number; port: number };
  lifecycle: {
    state: string;
    draining: boolean;
    activeRequests: number;
    drainStartedAt: string | null;
    drainReason: string | null;
    shutdownRequested: boolean;
    shutdownRequestedAt: string | null;
    shutdownReason: string | null;
  };
  readiness: {
    ok: boolean;
    dependencies: {
      sqlite?: ConsoleDependencyReadiness | null;
      redis: ConsoleDependencyReadiness;
      postgresql: ConsoleDependencyReadiness;
      objectStorage: ConsoleDependencyReadiness;
    };
    configuration: { apiKeySecret: boolean; publicBaseUrl: boolean };
  };
  routing: {
    configured: boolean;
    providerCount: number;
    routeCount: number;
    publishedModelCount: number;
  };
  credentialCache: { entryCount: number };
  requestMetrics: {
    requestsTotal: number;
    requestErrorsTotal: number;
    requestDrainRejectionsTotal: number;
    requestInFlight: number;
    requestDurationMsCount: number;
    requestDurationMsSum: number;
    rateLimitChecksTotal: number;
    rateLimitRejectionsTotal: number;
    rateLimitStoreFailuresTotal: number;
  };
  providerStats: ConsoleJsonObject;
};

export type ConsoleOperatorSummaryResponse = {
  summary: ConsoleOperatorSummary;
};

/** One row of `/v1/internal/gateway/requests`. */
export type ConsoleRequestAudit = {
  id: string;
  projectId: string;
  apiKeyId: string | null;
  userCredentialId: string | null;
  accessKeyId: string | null;
  sourceAccessKeyId: string | null;
  sessionId: string | null;
  routePolicyId: string | null;
  providerAccountId: string | null;
  protocolFamily: string;
  endpointKind: string;
  requestedModel: string | null;
  resolvedModel: string | null;
  modelAlias: string | null;
  stream: boolean;
  status: string;
  upstreamStatus: number | null;
  durationMs: number | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  cacheCreationInputTokens: number | null;
  cacheReadInputTokens: number | null;
  clientHasCacheControl: boolean;
  autoCacheApplied: boolean;
  errorSummary: string | null;
  routeTrace: ConsoleJsonObject | null;
  analysisProfile: ConsoleJsonObject | null;
  requestArtifactObjectKey: string | null;
  responseArtifactObjectKey: string | null;
  responseId: string;
  previousResponseId: string | null;
  clientDisconnectedAt: string | null;
  createdAt: string;
  completedAt: string | null;
  updatedAt: string;
};

export type ConsoleRequestAuditResponse = {
  requests: ConsoleRequestAudit[];
};

export type ConsoleAnalysisSample = {
  requestAuditId: string;
  responseId: string;
  projectId: string;
  routePolicyId: string | null;
  sessionId: string | null;
  providerAccountId: string | null;
  protocolFamily: string;
  endpointKind: string;
  requestedModel: string | null;
  resolvedModel: string | null;
  status: string;
  stream: boolean;
  createdAt: string;
  completedAt: string | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  cacheCreationInputTokens: number | null;
  cacheReadInputTokens: number | null;
  analysisProfile: ConsoleJsonObject | null;
  requestArtifactObjectKey: string | null;
  responseArtifactObjectKey: string | null;
  routeTrace: ConsoleJsonObject | null;
};

export type ConsoleAnalysisSampleResponse = {
  samples: ConsoleAnalysisSample[];
};

/** Full request-audit summary, including the bucket arrays the card view ignores. */
export type ConsoleRequestAuditFullSummary = {
  totalRequests: number;
  completedCount: number;
  failedCount: number;
  cancelledCount: number;
  runningCount: number;
  fallbackEligibleFailures: number;
  fallbackExhaustedFailures: number;
  byStatus: ConsoleValueBucket[];
  byProviderAccount: ConsoleValueBucket[];
  byEndpointKind: ConsoleValueBucket[];
  byErrorCode: ConsoleValueBucket[];
  providerAccounts: ConsoleRequestAuditProviderStats[];
};

export type ConsoleRequestAuditFullSummaryResponse = {
  summary: ConsoleRequestAuditFullSummary;
};
