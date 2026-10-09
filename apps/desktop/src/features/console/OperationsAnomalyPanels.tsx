import { useMemo, useState } from "react";
import type { OperationsWorkspaceProps } from "./operations-contracts";
import { databaseOperationsAvailability } from "./operationsAvailability";
import { OperationsCurrentIssues } from "./OperationsCurrentIssues";
import { OperationsIncidentPanels } from "./OperationsIncidentPanels";
import { OperationsRemediationPanels } from "./OperationsRemediationPanels";
import { OperationsExportPanels } from "./OperationsExportPanels";
import "./OperationsAnomalyPanels.css";

type Props = Pick<
  OperationsWorkspaceProps,
  | "t"
  | "pressure"
  | "readiness"
  | "requests"
  | "operatorSummary"
  | "incidents"
  | "incidentSummary"
  | "alertQueue"
  | "policies"
  | "editorLocked"
  | "incidentBusyId"
  | "onAcknowledgeIncident"
  | "onResolveIncident"
  | "remediationEffectiveness"
  | "remediationQueue"
  | "remediationRuns"
  | "hotspots"
  | "exportInventory"
  | "exports"
> & { onOpenRequests: () => void };

export function OperationsAnomalyPanels(props: Props) {
  const { t, operatorSummary, incidents } = props;
  const [view, setView] = useState("incidents");
  const availability = databaseOperationsAvailability(operatorSummary);
  const sortedIncidents = useMemo(() => {
    const weight = (status: string) =>
      status.toLowerCase() === "open"
        ? 0
        : status.toLowerCase() === "acknowledged"
          ? 1
          : 2;
    return [...(incidents.data ?? [])].sort(
      (left, right) =>
        weight(left.status) - weight(right.status) ||
        right.lastSeenAt.localeCompare(left.lastSeenAt),
    );
  }, [incidents.data]);
  const tabs = [
    ["incidents", t("事件与告警", "Incidents & alerts")],
    ["remediation", t("自动处置", "Automated remediation")],
    ["exports", t("分析与导出", "Analysis & exports")],
  ];
  return (
    <div className="nt-operations-anomalies">
      <OperationsCurrentIssues {...props} />
      {availability === "unsupported" ? (
        <p className="nt-copy nt-operations-capability" role="status">
          {t(
            "服务端事件处置未启用（需要 PostgreSQL）",
            "Server incident management is not enabled (requires PostgreSQL)",
          )}
        </p>
      ) : availability === "unknown" ? (
        <div className="nt-alert nt-alert--warning" role="status">
          {operatorSummary.loading
            ? t(
                "正在检查事件处置能力…",
                "Checking incident management availability…",
              )
            : operatorSummary.error ||
              t(
                "无法确认事件处置能力",
                "Incident management availability is unknown",
              )}
        </div>
      ) : (
        <>
          <div
            className="nt-operations-view-switch"
            role="group"
            aria-label={t("服务端分析", "Server analysis")}
          >
            {tabs.map(([id, label]) => (
              <button
                key={id}
                type="button"
                aria-pressed={view === id}
                className={`nt-btn ${view === id ? "nt-btn--primary" : "nt-btn--outline"}`}
                onClick={() => setView(id)}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="nt-operations-server-panels">
            {view === "incidents" ? (
              <OperationsIncidentPanels
                {...props}
                sortedIncidents={sortedIncidents}
              />
            ) : view === "remediation" ? (
              <OperationsRemediationPanels {...props} />
            ) : (
              <OperationsExportPanels {...props} />
            )}
          </div>
        </>
      )}
    </div>
  );
}
