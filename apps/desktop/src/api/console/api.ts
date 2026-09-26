import type {
  BootstrapStatus,
  ConsoleAccessAffinityResponse,
  ConsoleAccessBundle,
  ConsoleAccessBundleInput,
  ConsoleAccessCatalog,
  ConsoleAccessKey,
  ConsoleAccessKeyBalance,
  ConsoleAccessKeyInput,
  ConsoleAccountGroupSummaryResponse,
  ConsoleAlertQueueResponse,
  ConsoleAnalysisExportDiffResponse,
  ConsoleAnalysisExportInventorySummaryResponse,
  ConsoleAnalysisSampleResponse,
  ConsoleAnomalyIncidentFollowUpInput,
  ConsoleAnomalyIncidentHistoryResponse,
  ConsoleAnomalyIncidentMutationResponse,
  ConsoleAnomalyIncidentResponse,
  ConsoleAnomalyIncidentSummaryResponse,
  ConsoleAnomalyPolicyResponse,
  ConsoleApiAccessRotation,
  ConsoleCostOverviewResponse,
  ConsoleCredentialArchivePurgeResponse,
  ConsoleCredentialPoolAutomationResponse,
  ConsoleCredentialPoolAutomationRunResponse,
  ConsoleCredentialProbeResponse,
  ConsoleCredentialRefillRequestResponse,
  ConsoleCredentialRefillResponse,
  ConsoleCredentialUsageResponse,
  ConsoleGatewayReadinessResponse,
  ConsoleGeminiAuthSessionRequest,
  ConsoleGeminiAuthSessionResponse,
  ConsoleOperatorSummaryResponse,
  ConsolePersistedAnalysisExportResponse,
  ConsolePromptCacheSummaryResponse,
  ConsoleProviderCredentialInventoryResponse,
  ConsoleProviderCredentialModelStateResponse,
  ConsoleProviderProbeResponse,
  ConsoleRateLimitHotspotSummaryResponse,
  ConsoleRemediationEffectivenessResponse,
  ConsoleRemediationQueueResponse,
  ConsoleRemediationRunResponse,
  ConsoleRequestAuditFullSummaryResponse,
  ConsoleRequestAuditResponse,
  ConsoleRequestAuditSummaryResponse,
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigCommitResponse,
  ConsoleRouteConfigResponse,
  ConsoleRouteConfigValidationRequest,
  ConsoleRouteConfigValidationResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
  ConsoleRuntimePressureResponse,
  ConsoleUsageAggregateResponse,
  ConsoleUsageAggregateSummaryResponse,
  ConsoleUserCredentialIssueInput,
  ConsoleUserCredentialIssueResponse,
  ConsoleVerifiedUserCredential,
  ManagementSession,
  OperationSuccess,
  SecretGrant,
} from "../contracts";
import type {
  ConsoleAnalysisExportFilters,
  ConsoleAnomalyFilters,
  ConsoleAnomalyPolicyFilters,
  ConsoleRemediationFilters,
  ConsoleRequestFilters,
} from "./filters";

export type ConsoleApi = {
  getBootstrapStatus(): Promise<BootstrapStatus>;
  bootstrap(token: string): Promise<OperationSuccess>;
  verifySession(token: string): Promise<ManagementSession>;
  confirmSecretAccess(managementToken: string, confirmationToken: string): Promise<SecretGrant>;
  rotateSession(currentToken: string, newToken: string): Promise<OperationSuccess>;
  logout(token: string): Promise<OperationSuccess>;
  getRouteConfig(managementToken: string): Promise<ConsoleRouteConfigResponse>;
  getAccountGroupSummary(managementToken: string): Promise<ConsoleAccountGroupSummaryResponse>;
  getProviderCredentialInventory?(
    managementToken: string,
  ): Promise<ConsoleProviderCredentialInventoryResponse>;
  getCredentialPoolAutomation(
    managementToken: string,
  ): Promise<ConsoleCredentialPoolAutomationResponse>;
  runCredentialPoolAutomation(
    managementToken: string,
    providerId: string,
  ): Promise<ConsoleCredentialPoolAutomationRunResponse>;
  pruneCredentialPool(
    managementToken: string,
    providerId: string,
  ): Promise<ConsoleCredentialPoolAutomationRunResponse>;
  purgeCredentialArchive(
    managementToken: string,
    providerId: string,
  ): Promise<ConsoleCredentialArchivePurgeResponse>;
  getCredentialRefill(managementToken: string): Promise<ConsoleCredentialRefillResponse>;
  requestCredentialRefill(
    managementToken: string,
    providerId: string,
    requestedCount?: number,
  ): Promise<ConsoleCredentialRefillRequestResponse>;
  createGeminiAuthSession(
    managementToken: string,
    request: ConsoleGeminiAuthSessionRequest,
  ): Promise<ConsoleGeminiAuthSessionResponse>;
  completeGeminiAuthSession(
    managementToken: string,
    sessionId: string,
  ): Promise<ConsoleGeminiAuthSessionResponse>;
  getGeminiAuthSession(
    managementToken: string,
    sessionId: string,
  ): Promise<ConsoleGeminiAuthSessionResponse>;
  probeCredential(
    managementToken: string,
    secretGrant: string,
    credentialId: string,
  ): Promise<ConsoleCredentialProbeResponse>;
  probeProvider(
    managementToken: string,
    secretGrant: string,
    providerId: string,
  ): Promise<ConsoleProviderProbeResponse>;
  getCredentialUsage?(
    managementToken: string,
    credentialId: string,
    createdFrom: string,
  ): Promise<ConsoleCredentialUsageResponse>;
  /** Every usage bucket in the window, used to fill the whole credential ledger at once. */
  listUsageAggregates?(
    managementToken: string,
    params?: { createdFrom?: string; limit?: number },
  ): Promise<ConsoleUsageAggregateResponse>;
  /** Live concurrency and breaker state per provider account. */
  getRuntimePressure?(managementToken: string): Promise<ConsoleRuntimePressureResponse>;
  /** Token spend plus the price rates needed to turn tokens into money. */
  getCostOverview?(managementToken: string): Promise<ConsoleCostOverviewResponse>;
  /** Per-credential, per-model health recorded by the routing pipeline. */
  listProviderCredentialModelStates?(
    managementToken: string,
    params?: { limit?: number },
  ): Promise<ConsoleProviderCredentialModelStateResponse>;
  /** Request outcomes bucketed per provider account, for success-rate strips. */
  getRequestAuditSummary?(
    managementToken: string,
    params?: { createdFrom?: string; limit?: number },
  ): Promise<ConsoleRequestAuditSummaryResponse>;
  /** Dependency wiring plus drain state - answers even without Postgres. */
  getGatewayReadiness?(managementToken: string): Promise<ConsoleGatewayReadinessResponse>;
  /** Build, lifecycle, routing and request counters in one payload. */
  getOperatorSummary?(managementToken: string): Promise<ConsoleOperatorSummaryResponse>;
  /** Raw request-audit rows for the detail table. */
  listRequestAudits?(
    managementToken: string,
    params?: ConsoleRequestFilters,
  ): Promise<ConsoleRequestAuditResponse>;
  /** Token/artifact-oriented projection of the same rows. */
  listAnalysisSamples?(
    managementToken: string,
    params?: ConsoleRequestFilters,
  ): Promise<ConsoleAnalysisSampleResponse>;
  /** Every counter `/requests/summary` answers, including the bucket arrays. */
  getRequestAuditFullSummary?(
    managementToken: string,
    params?: ConsoleRequestFilters,
  ): Promise<ConsoleRequestAuditFullSummaryResponse>;
  /** Usage-archive queue depth and alerting. `queueDepth` is -1 without Redis. */
  getUsageAggregateSummary?(
    managementToken: string,
    params?: { createdFrom?: string; createdTo?: string; limit?: number },
  ): Promise<ConsoleUsageAggregateSummaryResponse>;
  getPromptCacheSummary?(
    managementToken: string,
    params?: ConsoleRequestFilters & { inputPricePerMillion?: number },
  ): Promise<ConsolePromptCacheSummaryResponse>;
  getRateLimitHotspots?(
    managementToken: string,
    params?: ConsoleRequestFilters & { lookbackHours?: number },
  ): Promise<ConsoleRateLimitHotspotSummaryResponse>;
  listAnomalyIncidents?(
    managementToken: string,
    params?: ConsoleAnomalyFilters,
  ): Promise<ConsoleAnomalyIncidentResponse>;
  getAnomalyIncidentSummary?(
    managementToken: string,
    params?: ConsoleAnomalyFilters,
  ): Promise<ConsoleAnomalyIncidentSummaryResponse>;
  /** Incidents whose alert interval has elapsed, with the actions to take. */
  getAnomalyAlertQueue?(
    managementToken: string,
    params?: ConsoleAnomalyFilters,
  ): Promise<ConsoleAlertQueueResponse>;
  listAnomalyIncidentHistory?(
    managementToken: string,
    incidentId: string,
  ): Promise<ConsoleAnomalyIncidentHistoryResponse>;
  acknowledgeAnomalyIncident?(
    managementToken: string,
    incidentId: string,
  ): Promise<ConsoleAnomalyIncidentMutationResponse>;
  resolveAnomalyIncident?(
    managementToken: string,
    incidentId: string,
  ): Promise<ConsoleAnomalyIncidentMutationResponse>;
  updateAnomalyIncidentFollowUp?(
    managementToken: string,
    incidentId: string,
    input: ConsoleAnomalyIncidentFollowUpInput,
  ): Promise<ConsoleAnomalyIncidentMutationResponse>;
  listAnomalyPolicies?(
    managementToken: string,
    params?: ConsoleAnomalyPolicyFilters,
  ): Promise<ConsoleAnomalyPolicyResponse>;
  /** Needs both Postgres and Redis; 503s when either is missing. */
  listRemediationQueue?(
    managementToken: string,
    params?: ConsoleRemediationFilters,
  ): Promise<ConsoleRemediationQueueResponse>;
  listRemediationRuns?(
    managementToken: string,
    params?: ConsoleRemediationFilters,
  ): Promise<ConsoleRemediationRunResponse>;
  getRemediationEffectiveness?(
    managementToken: string,
    params?: ConsoleRemediationFilters & { windowMinutes?: number },
  ): Promise<ConsoleRemediationEffectivenessResponse>;
  listPersistedAnalysisExports?(
    managementToken: string,
    params?: ConsoleAnalysisExportFilters,
  ): Promise<ConsolePersistedAnalysisExportResponse>;
  getAnalysisExportInventorySummary?(
    managementToken: string,
    params?: ConsoleAnalysisExportFilters,
  ): Promise<ConsoleAnalysisExportInventorySummaryResponse>;
  getAnalysisExportDiff?(
    managementToken: string,
    leftExportId: string,
    rightExportId: string,
  ): Promise<ConsoleAnalysisExportDiffResponse>;
  /** Whole access graph in one round trip; answers the bare view. */
  getAccessCatalog?(managementToken: string): Promise<ConsoleAccessCatalog>;
  /** Answers `token` exactly once - it is never persisted. */
  createAccessKey?(
    managementToken: string,
    input: ConsoleAccessKeyInput,
  ): Promise<ConsoleAccessKey>;
  rotateAccessKey?(managementToken: string, accessKeyId: string): Promise<ConsoleAccessKey>;
  revokeAccessKey?(
    managementToken: string,
    accessKeyId: string,
    reason?: string,
  ): Promise<OperationSuccess>;
  /** `null` until a balance row is provisioned for the key. */
  getAccessKeyBalance?(
    managementToken: string,
    accessKeyId: string,
  ): Promise<ConsoleAccessKeyBalance | null>;
  createAccessBundle?(
    managementToken: string,
    input: ConsoleAccessBundleInput,
  ): Promise<ConsoleAccessBundle>;
  /** Redis-only, so it stays usable when Postgres is unconfigured. */
  inspectAccessAffinity?(
    managementToken: string,
    params: { accessKeyId: string; model: string; explicitSessionKey?: string },
  ): Promise<ConsoleAccessAffinityResponse>;
  resetAccessAffinity?(
    managementToken: string,
    params: { accessKeyId: string; model: string; explicitSessionKey?: string },
  ): Promise<OperationSuccess>;
  /** Mints a fresh project API key; answers the plaintext token once. */
  rotateApiAccess?(
    managementToken: string,
    input: { projectId: string; name?: string; actorUserId?: string },
  ): Promise<ConsoleApiAccessRotation>;
  issueUserCredential?(
    managementToken: string,
    input: ConsoleUserCredentialIssueInput,
  ): Promise<ConsoleUserCredentialIssueResponse>;
  /** `scope` is a single required scope, matching `VerifyUserCredentialBody`. */
  verifyUserCredential?(
    managementToken: string,
    credentialKey: string,
    scope?: string,
  ): Promise<ConsoleVerifiedUserCredential>;
  revokeUserCredential?(
    managementToken: string,
    credentialKey: string,
    reason?: string,
  ): Promise<OperationSuccess>;
  validateRouteConfig(
    managementToken: string,
    draft: ConsoleRouteConfigValidationRequest,
    secretGrant?: string,
  ): Promise<ConsoleRouteConfigValidationResponse>;
  commitRouteConfig(
    managementToken: string,
    draft: ConsoleRouteConfigCommitRequest,
    secretGrant?: string,
  ): Promise<ConsoleRouteConfigCommitResponse>;
  listRouteConfigRevisions(managementToken: string): Promise<ConsoleRouteRevisionListResponse>;
  getRouteConfigRevision(
    managementToken: string,
    revisionId: string,
  ): Promise<ConsoleRouteRevisionDetailResponse>;
};
