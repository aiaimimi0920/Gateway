import type {
  ConsoleOperatorSummary,
  ConsoleRequestAudit,
} from "../../api/contracts";

export function operationsSummaryFixture(
  postgresql = true,
): ConsoleOperatorSummary {
  const dependency = {
    configured: false,
    required: false,
    ready: true,
    timedOut: false,
  };
  return {
    schemaVersion: 1,
    generatedAt: "2026-10-08T01:00:00Z",
    build: {
      name: "gateway",
      version: "0.1.0",
      target: { os: "windows", arch: "x86_64" },
      debugAssertions: false,
    },
    runtime: { role: "standalone", processId: 1, port: 4200 },
    lifecycle: {
      state: "serving",
      draining: false,
      activeRequests: 0,
      drainStartedAt: null,
      drainReason: null,
      shutdownRequested: false,
      shutdownRequestedAt: null,
      shutdownReason: null,
    },
    readiness: {
      ok: true,
      dependencies: {
        postgresql: {
          ...dependency,
          configured: postgresql,
          required: postgresql,
        },
        sqlite: postgresql
          ? null
          : { ...dependency, configured: true, required: true },
        redis: dependency,
        objectStorage: {
          ...dependency,
          configured: true,
          required: true,
          driver: "local",
        },
      },
      configuration: { apiKeySecret: false, publicBaseUrl: false },
    },
    routing: {
      configured: false,
      providerCount: 0,
      routeCount: 0,
      publishedModelCount: 0,
    },
    credentialCache: { entryCount: 0 },
    requestMetrics: {
      requestsTotal: 0,
      requestErrorsTotal: 0,
      requestDrainRejectionsTotal: 0,
      requestInFlight: 0,
      requestDurationMsCount: 0,
      requestDurationMsSum: 0,
      rateLimitChecksTotal: 0,
      rateLimitRejectionsTotal: 0,
      rateLimitStoreFailuresTotal: 0,
    },
    providerStats: {},
  };
}

export function operationsRequestFixture(
  id = "failed-request",
): ConsoleRequestAudit {
  return {
    id,
    projectId: "sample-project",
    apiKeyId: null,
    userCredentialId: null,
    accessKeyId: null,
    sourceAccessKeyId: null,
    sessionId: null,
    routePolicyId: null,
    providerAccountId: null,
    protocolFamily: "openai",
    endpointKind: "chat",
    requestedModel: "sample-model",
    resolvedModel: null,
    modelAlias: null,
    stream: false,
    status: "failed",
    upstreamStatus: 429,
    durationMs: 100,
    promptTokens: null,
    completionTokens: null,
    totalTokens: null,
    cacheCreationInputTokens: null,
    cacheReadInputTokens: null,
    clientHasCacheControl: false,
    autoCacheApplied: false,
    errorSummary: "upstream rate limited",
    routeTrace: null,
    analysisProfile: null,
    requestArtifactObjectKey: null,
    responseArtifactObjectKey: null,
    responseId: id,
    previousResponseId: null,
    clientDisconnectedAt: null,
    createdAt: "2026-10-08T01:00:00Z",
    completedAt: null,
    updatedAt: "2026-10-08T01:00:00Z",
  };
}
