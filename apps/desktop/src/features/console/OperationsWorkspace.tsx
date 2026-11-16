import { Button, Dialog } from "@neuro/ui";
import {
  type OperationsWorkspaceProps,
  type OperationsRequestFilterDraft,
} from "./operations-contracts";
import { OperationsRequestPanels } from "./OperationsRequestPanels";
import { OperationsAnomalyPanels } from "./OperationsAnomalyPanels";
import { DEFAULT_OPERATIONS_LIST_VIEW, requestRowMetadata, selectOperationsRows } from "./operationsListView";
import { databaseOperationsAvailability } from "./operationsAvailability";
import "./OperationsWorkspace.css";

export { OPERATIONS_DEFAULT_REQUEST_FILTERS } from "./operations-contracts";
export type {
  OperationsWorkspaceProps,
  OperationsSectionId,
  OperationsPanelState,
  OperationsRequestFilterDraft,
} from "./operations-contracts";

/**
 * Task-based views share the existing telemetry owner. Changing views never
 * applies a filter or performs an incident mutation.
 */
export function OperationsWorkspace({
  t,
  notice,
  editorLocked,
  listView = DEFAULT_OPERATIONS_LIST_VIEW,
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
  const patchFilters = (patch: Partial<OperationsRequestFilterDraft>) => {
    onRequestFiltersChange({ ...requestFilters, ...patch });
  };

  return (
    <div className="nt-operations">
      {notice}

      <div className="nt-operations-panel">
          <OperationsRequestPanels
            t={t}
            operatorSummary={operatorSummary}
            editorLocked={editorLocked}
            requests={{ ...requests, data: requests.data === null ? null : selectOperationsRows(requests.data, { ...listView, sort: listView.sort === "default" ? "newest" : listView.sort }, requestRowMetadata) }}
            requestSummary={requestSummary}
            usageSummary={usageSummary}
            promptCache={promptCache}
            requestFilters={requestFilters}
            onRequestFiltersChange={onRequestFiltersChange}
            onApplyRequestFilters={onApplyRequestFilters}
            patchFilters={patchFilters}
          />
      </div>
      {databaseOperationsAvailability(operatorSummary) === "available" && <Dialog className="nt-operations-drawer" title={t("事件记录", "Incident records")} trigger={<Button compact>{t("事件记录", "Incident records")}</Button>}>
          <OperationsAnomalyPanels
            t={t}
            listView={listView}
            operatorSummary={operatorSummary}
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
      </Dialog>}
    </div>
  );
}
