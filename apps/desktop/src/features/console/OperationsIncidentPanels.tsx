import { Badge, Button, DataTable, EmptyState } from "@neuro/ui";
import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  PLACEHOLDER,
  formatCount,
  formatTimestamp,
  formatText,
  severityTone,
  statusTone,
  StatCard,
  Panel,
  BucketList,
} from "./OperationsPrimitives";

type Props = Pick<
  OperationsWorkspaceProps,
  | "t"
  | "incidentSummary"
  | "alertQueue"
  | "incidents"
  | "policies"
  | "editorLocked"
  | "incidentBusyId"
  | "onAcknowledgeIncident"
  | "onResolveIncident"
> & {
  sortedIncidents: NonNullable<OperationsWorkspaceProps["incidents"]["data"]>;
};

export function OperationsIncidentPanels({
  t,
  incidentSummary,
  alertQueue,
  incidents,
  policies,
  editorLocked,
  incidentBusyId,
  onAcknowledgeIncident,
  onResolveIncident,
  sortedIncidents,
}: Props) {
  return (
    <>
      <Panel
        state={incidentSummary}
        t={t}
        title={t("异常概况", "Incident summary")}
      >
        {(data) => (
          <>
            <div className="nt-operations-stats">
              <StatCard
                label={t("总事件", "Incidents")}
                value={formatCount(data.totalIncidents)}
              />
              <StatCard
                label={t("未处理", "Open")}
                tone={data.openIncidents > 0 ? "danger" : "default"}
                value={formatCount(data.openIncidents)}
              />
              <StatCard
                label={t("已确认", "Acknowledged")}
                value={formatCount(data.acknowledgedIncidents)}
              />
              <StatCard
                label={t("已解决", "Resolved")}
                tone="emerald"
                value={formatCount(data.resolvedIncidents)}
              />
            </div>
            <dl className="nt-operations-metrics">
              <div>
                <dt>{t("已升级", "Escalated")}</dt>
                <dd>{formatCount(data.escalatedIncidents)}</dd>
              </div>
            </dl>
            <BucketList
              buckets={data.bySeverity}
              t={t}
              title={t("按等级", "By severity")}
            />
            <BucketList
              buckets={data.byCode}
              t={t}
              title={t("按代码", "By code")}
            />
          </>
        )}
      </Panel>

      <Panel
        collapsed={!alertQueue.data?.incidentCount}
        state={alertQueue}
        t={t}
        title={t("待告警队列", "Alert queue")}
      >
        {(data) => (
          <>
            <div className="nt-operations-stats">
              <StatCard
                label={t("队列事件", "Queued incidents")}
                value={formatCount(data.incidentCount)}
              />
              <StatCard
                label={t("已到期", "Due now")}
                tone={data.dueCount > 0 ? "danger" : "default"}
                value={formatCount(data.dueCount)}
              />
              <StatCard
                label={t("仅到期", "Due only")}
                value={data.dueOnly ? t("是", "Yes") : t("否", "No")}
              />
              <StatCard
                label={t("采集时间", "Generated at")}
                value={formatTimestamp(data.generatedAt)}
              />
            </div>
            {data.items.length === 0 ? (
              <div role="status"><EmptyState compact title={t("没有待告警事件", "No incidents awaiting alerts")} /></div>
            ) : (
              <div className="nt-operations-table nt-operations-table--alert-queue"><DataTable label={t("待告警事件", "Queued alerts")} rows={data.items} getRowKey={(item) => item.incident.id} columns={[
                {
                  key: "col-0", header: <>{t("事件", "Incident")}</>, width: "30%", rowHeader: true, cell: (item) => (<span title={item.incident.summary}>
                    {formatText(item.incident.code)}
                  </span>)
                },
                {
                  key: "col-1", header: <>{t("等级", "Severity")}</>, width: "24%", cell: (item) => (<span className="n-row">
                    <Badge tone={severityTone(item.webhookSeverity)}>
                      {formatText(item.webhookSeverity)}
                    </Badge>
                    {item.alertDue ? (
                      <Badge tone="warning">
                        {t("到期", "Due")}
                      </Badge>
                    ) : null}
                  </span>)
                },
                { key: "col-2", header: <>{t("下次告警", "Next alert")}</>, width: "25%", cell: (item) => (<span>{formatTimestamp(item.nextAlertDueAt)}</span>) },
                {
                  key: "col-3", header: <>{t("处置动作", "Actions")}</>, width: "21%", cell: (item) => (<span title={item.remediationActionKeys.join(", ")}>
                    {item.remediationActionKeys.length === 0
                      ? PLACEHOLDER
                      : formatCount(item.remediationActionKeys.length)}
                  </span>)
                }
              ]} /></div>
            )}
          </>
        )}
      </Panel>

      <Panel
        emptyLabel={t("暂无异常事件", "No incidents yet")}
        state={incidents}
        t={t}
        title={t("异常事件", "Incidents")}
      >
        {() =>
          sortedIncidents.length === 0 ? (
            <div role="status"><EmptyState compact title={t("暂无异常事件", "No incidents yet")} /></div>
          ) : (
            <div className="nt-operations-table nt-operations-table--incidents"><DataTable label={t("异常事件", "Incidents")} rows={sortedIncidents} getRowKey={(incident) => incident.id} columns={[
              {
                key: "col-0", header: <>{t("代码", "Code")}</>, width: "24%", rowHeader: true, cell: (incident) => (<span title={incident.summary}>
                  {formatText(incident.code)}
                </span>)
              },
              {
                key: "col-1", header: <>{t("等级", "Severity")}</>, width: "14%", cell: (incident) => (<span>
                  <Badge tone={severityTone(incident.severity)}>
                    {formatText(incident.severity)}
                  </Badge>
                </span>)
              },
              {
                key: "col-2", header: <>{t("状态", "Status")}</>, width: "19%", cell: (incident) => (<span className="n-row">
                  <Badge tone={statusTone(incident.status)}>
                    {formatText(incident.status)}
                  </Badge>
                  {incident.escalationStatus !== "none" ? (
                    <Badge>
                      {formatText(incident.escalationStatus)}
                    </Badge>
                  ) : null}
                </span>)
              },
              { key: "col-3", header: <>{t("最近出现", "Last seen")}</>, width: "20%", cell: (incident) => (<span>{formatTimestamp(incident.lastSeenAt)}</span>) },
              {
                key: "col-4", header: <>{t("操作", "Actions")}</>, width: "23%", cell: (incident) => {
                  const busy = incidentBusyId === incident.id;
                  const normalized = incident.status.toLowerCase();
                  const resolved = normalized === "resolved";
                  return (<span className="n-row">
                    <Button compact disabled={editorLocked || busy || normalized !== "open"} onClick={() => onAcknowledgeIncident(incident.id)} type="button">
                      {t("确认", "Acknowledge")}
                    </Button>
                    <Button compact disabled={editorLocked || busy || resolved} onClick={() => onResolveIncident(incident.id)} type="button">
                      {t("解决", "Resolve")}
                    </Button>
                  </span>);
                }
              }
            ]} /></div>
          )
        }
      </Panel>

      <Panel
        collapsed
        emptyLabel={t("暂无异常策略", "No policies yet")}
        state={policies}
        t={t}
        title={t("异常策略", "Anomaly policies")}
      >
        {(rows) =>
          rows.length === 0 ? (
            <div role="status"><EmptyState compact title={t("暂无异常策略", "No policies yet")} /></div>
          ) : (
            <div className="nt-operations-table nt-operations-table--policies"><DataTable label={t("异常策略", "Anomaly policies")} rows={rows} getRowKey={(policy) => policy.id} columns={[
              { key: "col-0", header: <>{t("策略", "Policy")}</>, width: "28%", rowHeader: true, cell: (policy) => (<span title={policy.id}>{formatText(policy.name)}</span>) },
              {
                key: "col-1", header: <>{t("状态", "Status")}</>, width: "18%", cell: (policy) => (<span>
                  <Badge tone={statusTone(policy.status)}>
                    {formatText(policy.status)}
                  </Badge>
                </span>)
              },
              {
                key: "col-2", header: <>{t("自动化", "Automation")}</>, width: "30%", cell: (policy) => (<span className="n-row">
                  {policy.autoSyncEnabled ? (
                    <Badge>
                      {t("同步", "Sync")}
                    </Badge>
                  ) : null}
                  {policy.alertingEnabled ? (
                    <Badge>
                      {t("告警", "Alerts")}
                    </Badge>
                  ) : null}
                  {policy.autoEscalateEnabled ? (
                    <Badge>
                      {t("升级", "Escalate")}
                    </Badge>
                  ) : null}
                  {policy.autoRemediationEnabled ? (
                    <Badge>
                      {t("处置", "Remediate")}
                    </Badge>
                  ) : null}
                  {!policy.autoSyncEnabled &&
                    !policy.alertingEnabled &&
                    !policy.autoEscalateEnabled &&
                    !policy.autoRemediationEnabled
                    ? PLACEHOLDER
                    : null}
                </span>)
              },
              {
                key: "col-3", header: <>{t("下次同步", "Next sync")}</>, width: "24%", cell: (policy) => (<span className="n-row">
                  {formatTimestamp(policy.nextSyncDueAt)}
                  {policy.syncDue ? (
                    <Badge tone="warning">
                      {t("到期", "Due")}
                    </Badge>
                  ) : null}
                </span>)
              }
            ]} /></div>
          )
        }
      </Panel>
    </>
  );
}
