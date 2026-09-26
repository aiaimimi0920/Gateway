import { Gauge, ListOrdered, RefreshCw, ShieldAlert } from "lucide-react";
import { useMemo, useState } from "react";
import {
  type OperationsWorkspaceProps,
  type OperationsSectionId,
  type OperationsRequestFilterDraft,
} from "./operations-contracts";
import { Section } from "./OperationsPrimitives";
import { OperationsLivePanels } from "./OperationsLivePanels";
import { OperationsRequestPanels } from "./OperationsRequestPanels";
import { OperationsIncidentPanels } from "./OperationsIncidentPanels";
import { OperationsRemediationPanels } from "./OperationsRemediationPanels";
import { OperationsExportPanels } from "./OperationsExportPanels";

export { OPERATIONS_DEFAULT_REQUEST_FILTERS } from "./operations-contracts";
export type {
  OperationsWorkspaceProps,
  OperationsSectionId,
  OperationsPanelState,
  OperationsRequestFilterDraft,
} from "./operations-contracts";

/**
 * 运维 workspace. Three collapsible sections read straight from the gateway's
 * internal telemetry endpoints: live pressure and readiness, the request-audit
 * detail table, and the anomaly/remediation surface.
 */
export function OperationsWorkspace({
  t,
  notice,
  editorLocked,
  refreshing,
  onRefresh,
  pressure,
  readiness,
  operatorSummary,
  requests,
  requestSummary,
  usageSummary,
  promptCache,
  incidents,
  incidentSummary,
  alertQueue,
  policies,
  remediationQueue,
  remediationRuns,
  remediationEffectiveness,
  hotspots,
  exports,
  exportInventory,
  requestFilters,
  onRequestFiltersChange,
  onApplyRequestFilters,
  incidentBusyId,
  onAcknowledgeIncident,
  onResolveIncident,
}: OperationsWorkspaceProps) {
  const [openSections, setOpenSections] = useState<OperationsSectionId[]>(["live"]);

  const toggleSection = (id: OperationsSectionId) => {
    setOpenSections((current) =>
      current.includes(id) ? current.filter((entry) => entry !== id) : [...current, id],
    );
  };

  const patchFilters = (patch: Partial<OperationsRequestFilterDraft>) => {
    onRequestFiltersChange({ ...requestFilters, ...patch });
  };

  /** Incidents still needing a human decision float to the top of the table. */
  const sortedIncidents = useMemo(() => {
    const rows = incidents.data ?? [];
    const weight = (status: string | null | undefined) => {
      const normalized = (status ?? "").toLowerCase();
      if (normalized === "open") {
        return 0;
      }
      return normalized === "acknowledged" ? 1 : 2;
    };
    return [...rows].sort((left, right) => {
      const byStatus = weight(left.status) - weight(right.status);
      return byStatus !== 0 ? byStatus : right.lastSeenAt.localeCompare(left.lastSeenAt);
    });
  }, [incidents.data]);

  return (
    <div className="nt-settings-page nt-console-page nt-console-table-page">
      {notice}
      <div className="nt-console-toolbar">
        <p className="nt-copy">
          {t(
            "运维数据直接来自网关内部接口；未配置 PostgreSQL 的面板会说明原因并保留占位符。",
            "Operations data comes straight from the gateway internal endpoints; panels without PostgreSQL state the reason and keep placeholders.",
          )}
        </p>
        <button
          className="nt-btn nt-btn--outline"
          disabled={refreshing}
          onClick={onRefresh}
          type="button"
        >
          <RefreshCw size={15} aria-hidden="true" />
          {refreshing ? t("刷新中…", "Refreshing…") : t("刷新", "Refresh")}
        </button>
      </div>

      <div className="nt-settings-accordion">
        <Section
          icon={<Gauge size={18} />}
          id="live"
          onToggle={toggleSection}
          open={openSections.includes("live")}
          title={t("实时", "Live")}
        >
          <OperationsLivePanels
            t={t}
            pressure={pressure}
            readiness={readiness}
            operatorSummary={operatorSummary}
          />
        </Section>

        <Section
          icon={<ListOrdered size={18} />}
          id="requests"
          onToggle={toggleSection}
          open={openSections.includes("requests")}
          title={t("请求明细", "Request log")}
        >
          <OperationsRequestPanels
            t={t}
            editorLocked={editorLocked}
            requests={requests}
            requestSummary={requestSummary}
            usageSummary={usageSummary}
            promptCache={promptCache}
            requestFilters={requestFilters}
            onRequestFiltersChange={onRequestFiltersChange}
            onApplyRequestFilters={onApplyRequestFilters}
            patchFilters={patchFilters}
          />
        </Section>

        <Section
          icon={<ShieldAlert size={18} />}
          id="anomalies"
          onToggle={toggleSection}
          open={openSections.includes("anomalies")}
          title={t("异常与处置", "Anomalies & remediation")}
        >
          <OperationsIncidentPanels
            t={t}
            incidentSummary={incidentSummary}
            alertQueue={alertQueue}
            incidents={incidents}
            policies={policies}
            editorLocked={editorLocked}
            incidentBusyId={incidentBusyId}
            onAcknowledgeIncident={onAcknowledgeIncident}
            onResolveIncident={onResolveIncident}
            sortedIncidents={sortedIncidents}
          />

          <OperationsRemediationPanels
            t={t}
            remediationEffectiveness={remediationEffectiveness}
            remediationQueue={remediationQueue}
            remediationRuns={remediationRuns}
          />

          <OperationsExportPanels
            t={t}
            hotspots={hotspots}
            exportInventory={exportInventory}
            exports={exports}
          />
        </Section>
      </div>
    </div>
  );
}
