import type { ReactNode } from "react";
import type {
  ConsoleAlertQueue,
  ConsoleAnalysisExportInventorySummary,
  ConsoleAnomalyIncident,
  ConsoleAnomalyIncidentSummary,
  ConsoleAnomalyPolicy,
  ConsoleGatewayReadiness,
  ConsoleOperatorSummary,
  ConsolePersistedAnalysisExport,
  ConsolePromptCacheSummary,
  ConsoleRateLimitHotspotSummary,
  ConsoleRemediationEffectiveness,
  ConsoleRemediationQueue,
  ConsoleRemediationRun,
  ConsoleRequestAudit,
  ConsoleRequestAuditFullSummary,
  ConsoleRuntimePressure,
  ConsoleUsageAggregateSummary,
} from "../../api/contracts";

export type TranslateFn = (zh: string, en: string) => string;

/** Sections are collapsible so the page opens on 实时 instead of a wall of tables. */
export type OperationsSectionId = "live" | "requests" | "anomalies";

/**
 * Each panel loads independently and carries its own error, so a panel whose
 * endpoint answered 503 (`PostgreSQL 尚未配置`) still renders: it keeps the
 * placeholder dashes and states the reason, matching the credential-pool cards.
 */
export type OperationsPanelState<T> = {
  data: T | null;
  error: string | null;
  loading: boolean;
};

export type OperationsRequestFilterDraft = {
  status: string;
  projectId: string;
  providerAccountId: string;
  endpointKind: string;
  errorCode: string;
  limit: string;
};

export const OPERATIONS_DEFAULT_REQUEST_FILTERS: OperationsRequestFilterDraft = {
  status: "",
  projectId: "",
  providerAccountId: "",
  endpointKind: "",
  errorCode: "",
  limit: "100",
};

export type OperationsWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  /** Blocks the mutation buttons while a management-token refresh is in flight. */
  editorLocked: boolean;
  refreshing: boolean;
  onRefresh: () => void;
  pressure: OperationsPanelState<ConsoleRuntimePressure>;
  readiness: OperationsPanelState<ConsoleGatewayReadiness>;
  operatorSummary: OperationsPanelState<ConsoleOperatorSummary>;
  requests: OperationsPanelState<ConsoleRequestAudit[]>;
  requestSummary: OperationsPanelState<ConsoleRequestAuditFullSummary>;
  usageSummary: OperationsPanelState<ConsoleUsageAggregateSummary>;
  promptCache: OperationsPanelState<ConsolePromptCacheSummary>;
  incidents: OperationsPanelState<ConsoleAnomalyIncident[]>;
  incidentSummary: OperationsPanelState<ConsoleAnomalyIncidentSummary>;
  alertQueue: OperationsPanelState<ConsoleAlertQueue>;
  policies: OperationsPanelState<ConsoleAnomalyPolicy[]>;
  remediationQueue: OperationsPanelState<ConsoleRemediationQueue>;
  remediationRuns: OperationsPanelState<ConsoleRemediationRun[]>;
  remediationEffectiveness: OperationsPanelState<ConsoleRemediationEffectiveness>;
  hotspots: OperationsPanelState<ConsoleRateLimitHotspotSummary>;
  exports: OperationsPanelState<ConsolePersistedAnalysisExport[]>;
  exportInventory: OperationsPanelState<ConsoleAnalysisExportInventorySummary>;
  requestFilters: OperationsRequestFilterDraft;
  onRequestFiltersChange: (next: OperationsRequestFilterDraft) => void;
  onApplyRequestFilters: () => void;
  incidentBusyId: string | null;
  onAcknowledgeIncident: (incidentId: string) => void;
  onResolveIncident: (incidentId: string) => void;
};
