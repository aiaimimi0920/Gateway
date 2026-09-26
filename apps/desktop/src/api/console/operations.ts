import type { GatewayApiClient } from "../client";
import {
  consoleAlertQueueResponseSchema,
  consoleAnalysisExportDiffResponseSchema,
  consoleAnalysisExportInventorySummaryResponseSchema,
  consoleAnalysisSampleResponseSchema,
  consoleAnomalyIncidentHistoryResponseSchema,
  consoleAnomalyIncidentMutationResponseSchema,
  consoleAnomalyIncidentResponseSchema,
  consoleAnomalyIncidentSummaryResponseSchema,
  consoleAnomalyPolicyResponseSchema,
  consoleGatewayReadinessResponseSchema,
  consoleOperatorSummaryResponseSchema,
  consolePersistedAnalysisExportResponseSchema,
  consolePromptCacheSummaryResponseSchema,
  consoleRateLimitHotspotSummaryResponseSchema,
  consoleRemediationEffectivenessResponseSchema,
  consoleRemediationQueueResponseSchema,
  consoleRemediationRunResponseSchema,
  consoleRequestAuditFullSummaryResponseSchema,
  consoleRequestAuditResponseSchema,
  consoleUsageAggregateSummaryResponseSchema,
} from "../schemas";
import type { ConsoleApi } from "./api";
import { buildQuery } from "./filters";
import { INTERNAL_ROOT } from "./roots";

type ConsoleOperationsApi = Pick<
  ConsoleApi,
  | "getGatewayReadiness"
  | "getOperatorSummary"
  | "listRequestAudits"
  | "listAnalysisSamples"
  | "getRequestAuditFullSummary"
  | "getUsageAggregateSummary"
  | "getPromptCacheSummary"
  | "getRateLimitHotspots"
  | "listAnomalyIncidents"
  | "getAnomalyIncidentSummary"
  | "getAnomalyAlertQueue"
  | "listAnomalyIncidentHistory"
  | "acknowledgeAnomalyIncident"
  | "resolveAnomalyIncident"
  | "updateAnomalyIncidentFollowUp"
  | "listAnomalyPolicies"
  | "listRemediationQueue"
  | "listRemediationRuns"
  | "getRemediationEffectiveness"
  | "listPersistedAnalysisExports"
  | "getAnalysisExportInventorySummary"
  | "getAnalysisExportDiff"
>;

export function createConsoleOperationsApi(client: GatewayApiClient): ConsoleOperationsApi {
  return {
    getGatewayReadiness: (managementToken) =>
      client.request(`${INTERNAL_ROOT}/readiness`, consoleGatewayReadinessResponseSchema, {
        managementToken,
      }),
    getOperatorSummary: (managementToken) =>
      client.request(`${INTERNAL_ROOT}/operations/summary`, consoleOperatorSummaryResponseSchema, {
        managementToken,
      }),
    listRequestAudits: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/requests${buildQuery(params, { limit: 200 })}`,
        consoleRequestAuditResponseSchema,
        { managementToken },
      ),
    listAnalysisSamples: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/samples${buildQuery(params, { limit: 100 })}`,
        consoleAnalysisSampleResponseSchema,
        { managementToken },
      ),
    getRequestAuditFullSummary: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/requests/summary${buildQuery(params, { limit: 1000 })}`,
        consoleRequestAuditFullSummaryResponseSchema,
        { managementToken },
      ),
    getUsageAggregateSummary: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/usage-aggregates/summary${buildQuery(params, { limit: 2000 })}`,
        consoleUsageAggregateSummaryResponseSchema,
        { managementToken },
      ),
    getPromptCacheSummary: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/prompt-cache/summary${buildQuery(params, { limit: 1000 })}`,
        consolePromptCacheSummaryResponseSchema,
        { managementToken },
      ),
    getRateLimitHotspots: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/rate-limit-hotspots${buildQuery(params, { limit: 1000 })}`,
        consoleRateLimitHotspotSummaryResponseSchema,
        { managementToken },
      ),
    listAnomalyIncidents: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents${buildQuery(params, { limit: 200 })}`,
        consoleAnomalyIncidentResponseSchema,
        { managementToken },
      ),
    getAnomalyIncidentSummary: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents/summary${buildQuery(params, { limit: 500 })}`,
        consoleAnomalyIncidentSummaryResponseSchema,
        { managementToken },
      ),
    getAnomalyAlertQueue: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents/alert-queue${buildQuery(params, { limit: 100 })}`,
        consoleAlertQueueResponseSchema,
        { managementToken },
      ),
    listAnomalyIncidentHistory: (managementToken, incidentId) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents/${encodeURIComponent(incidentId)}/history`,
        consoleAnomalyIncidentHistoryResponseSchema,
        { managementToken },
      ),
    acknowledgeAnomalyIncident: (managementToken, incidentId) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents/${encodeURIComponent(incidentId)}/acknowledge`,
        consoleAnomalyIncidentMutationResponseSchema,
        { method: "POST", managementToken },
      ),
    resolveAnomalyIncident: (managementToken, incidentId) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents/${encodeURIComponent(incidentId)}/resolve`,
        consoleAnomalyIncidentMutationResponseSchema,
        { method: "POST", managementToken },
      ),
    updateAnomalyIncidentFollowUp: (managementToken, incidentId, input) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-incidents/${encodeURIComponent(incidentId)}/follow-up`,
        consoleAnomalyIncidentMutationResponseSchema,
        { method: "POST", managementToken, body: input },
      ),
    listAnomalyPolicies: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/anomaly-policies${buildQuery(params, { limit: 200 })}`,
        consoleAnomalyPolicyResponseSchema,
        { managementToken },
      ),
    listRemediationQueue: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/remediation-queue${buildQuery(params, { limit: 100 })}`,
        consoleRemediationQueueResponseSchema,
        { managementToken },
      ),
    listRemediationRuns: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/remediation-runs${buildQuery(params, { limit: 200 })}`,
        consoleRemediationRunResponseSchema,
        { managementToken },
      ),
    getRemediationEffectiveness: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/remediation-runs/effectiveness${buildQuery(params, { limit: 200 })}`,
        consoleRemediationEffectivenessResponseSchema,
        { managementToken },
      ),
    listPersistedAnalysisExports: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/exports${buildQuery(params, { limit: 100 })}`,
        consolePersistedAnalysisExportResponseSchema,
        { managementToken },
      ),
    getAnalysisExportInventorySummary: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/exports/summary${buildQuery(params, { limit: 500 })}`,
        consoleAnalysisExportInventorySummaryResponseSchema,
        { managementToken },
      ),
    getAnalysisExportDiff: (managementToken, leftExportId, rightExportId) =>
      client.request(
        `${INTERNAL_ROOT}/analysis/exports/diff${buildQuery({ leftExportId, rightExportId })}`,
        consoleAnalysisExportDiffResponseSchema,
        { managementToken },
      ),
  };
}
