import * as Tabs from "@radix-ui/react-tabs";
import { useRef, useState } from "react";
import {
  type OperationsWorkspaceProps,
  type OperationsSectionId,
  type OperationsRequestFilterDraft,
} from "./operations-contracts";
import { OperationsNavigation } from "./OperationsNavigation";
import { OperationsCurrentIssues } from "./OperationsCurrentIssues";
import { OperationsLivePanels } from "./OperationsLivePanels";
import { OperationsRequestPanels } from "./OperationsRequestPanels";
import { OperationsAnomalyPanels } from "./OperationsAnomalyPanels";
import { DEFAULT_OPERATIONS_LIST_VIEW, requestRowMetadata, selectOperationsRows } from "./operationsListView";
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
  const [section, setSection] = useState<OperationsSectionId>("live");
  const requestsTabRef = useRef<HTMLButtonElement>(null);
  const openRequests = () => {
    setSection("requests");
    // The source button unmounts; keep keyboard focus at the destination.
    requestsTabRef.current?.focus();
  };

  const patchFilters = (patch: Partial<OperationsRequestFilterDraft>) => {
    onRequestFiltersChange({ ...requestFilters, ...patch });
  };

  return (
    <div className="nt-settings-page nt-console-page nt-console-table-page nt-operations">
      {notice}

      <Tabs.Root
        value={section}
        onValueChange={(value) => setSection(value as OperationsSectionId)}
      >
        <OperationsNavigation
          t={t}
          operatorSummary={operatorSummary}
          incidentSummary={incidentSummary}
          requestsTabRef={requestsTabRef}
        />
        <Tabs.Content className="nt-operations-panel" value="live">
          <OperationsLivePanels
            t={t}
            pressure={pressure}
            readiness={readiness}
            operatorSummary={operatorSummary}
            listView={listView}
          />
          <OperationsCurrentIssues
            t={t}
            pressure={pressure}
            readiness={readiness}
            requests={requests}
            listView={listView}
            onOpenRequests={openRequests}
          />
        </Tabs.Content>

        <Tabs.Content className="nt-operations-panel" value="requests">
          <OperationsRequestPanels
            t={t}
            operatorSummary={operatorSummary}
            editorLocked={editorLocked}
            requests={{ ...requests, data: requests.data === null ? null : selectOperationsRows(requests.data, listView, requestRowMetadata) }}
            requestSummary={requestSummary}
            usageSummary={usageSummary}
            promptCache={promptCache}
            requestFilters={requestFilters}
            onRequestFiltersChange={onRequestFiltersChange}
            onApplyRequestFilters={onApplyRequestFilters}
            patchFilters={patchFilters}
          />
        </Tabs.Content>

        <Tabs.Content className="nt-operations-panel" value="anomalies">
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
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}
