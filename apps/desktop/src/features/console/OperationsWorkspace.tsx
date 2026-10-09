import { Gauge, ListOrdered, ShieldAlert } from "lucide-react";
import { useState } from "react";
import {
  type OperationsWorkspaceProps,
  type OperationsSectionId,
  type OperationsRequestFilterDraft,
} from "./operations-contracts";
import { Section } from "./OperationsPrimitives";
import { OperationsLivePanels } from "./OperationsLivePanels";
import { OperationsRequestPanels } from "./OperationsRequestPanels";
import { OperationsAnomalyPanels } from "./OperationsAnomalyPanels";
import "./OperationsWorkspace.css";

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
  const [openSections, setOpenSections] = useState<OperationsSectionId[]>([
    "live",
  ]);

  const toggleSection = (id: OperationsSectionId) => {
    setOpenSections((current) =>
      current.includes(id)
        ? current.filter((entry) => entry !== id)
        : [...current, id],
    );
  };

  const patchFilters = (patch: Partial<OperationsRequestFilterDraft>) => {
    onRequestFiltersChange({ ...requestFilters, ...patch });
  };

  return (
    <div className="nt-settings-page nt-console-page nt-console-table-page nt-operations">
      {notice}

      <div className="nt-settings-accordion">
        <Section
          icon={<Gauge size={18} />}
          id="live"
          onToggle={toggleSection}
          open={openSections.includes("live")}
          title={t("运行状态", "Live")}
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
            operatorSummary={operatorSummary}
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
          status={
            incidentSummary.data && incidentSummary.data.openIncidents > 0 ? (
              <span className="nt-badge nt-badge--danger">
                {t(
                  `${incidentSummary.data.openIncidents} 未处理`,
                  `${incidentSummary.data.openIncidents} open`,
                )}
              </span>
            ) : incidentSummary.error ? (
              <span className="nt-badge nt-badge--warning">
                {t("读取失败", "Read failed")}
              </span>
            ) : null
          }
        >
          <OperationsAnomalyPanels
            t={t}
            pressure={pressure}
            readiness={readiness}
            requests={requests}
            operatorSummary={operatorSummary}
            onOpenRequests={() => setOpenSections(["requests"])}
            incidentSummary={incidentSummary}
            alertQueue={alertQueue}
            incidents={incidents}
            policies={policies}
            editorLocked={editorLocked}
            incidentBusyId={incidentBusyId}
            onAcknowledgeIncident={onAcknowledgeIncident}
            onResolveIncident={onResolveIncident}
            remediationEffectiveness={remediationEffectiveness}
            remediationQueue={remediationQueue}
            remediationRuns={remediationRuns}
            hotspots={hotspots}
            exportInventory={exportInventory}
            exports={exports}
          />
        </Section>
      </div>
    </div>
  );
}
