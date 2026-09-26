import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  PLACEHOLDER,
  formatCount,
  formatTimestamp,
  formatText,
  severityBadgeClass,
  statusBadgeClass,
  StatCard,
  Panel,
  BucketList,
} from "./OperationsPrimitives";

type Props = Pick<OperationsWorkspaceProps,
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
      <Panel state={incidentSummary} t={t} title={t("异常概况", "Incident summary")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
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
            <dl className="nt-pilot-metric-list">
              <div>
                <dt>{t("已升级", "Escalated")}</dt>
                <dd>{formatCount(data.escalatedIncidents)}</dd>
              </div>
            </dl>
            <BucketList buckets={data.bySeverity} t={t} title={t("按等级", "By severity")} />
            <BucketList buckets={data.byCode} t={t} title={t("按代码", "By code")} />
          </>
        )}
      </Panel>

      <Panel state={alertQueue} t={t} title={t("待告警队列", "Alert queue")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
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
              <div className="nt-pilot-stats-state" role="status">
                {t("没有待告警事件", "No incidents awaiting alerts")}
              </div>
            ) : (
              <div className="nt-table nt-table--alert-queue">
                <div className="nt-table__head">
                  <span>{t("事件", "Incident")}</span>
                  <span>{t("等级", "Severity")}</span>
                  <span>{t("下次告警", "Next alert")}</span>
                  <span>{t("处置动作", "Actions")}</span>
                </div>
                {data.items.map((item) => (
                  <div className="nt-table__row" key={item.incident.id}>
                    <span title={item.incident.summary}>{formatText(item.incident.code)}</span>
                    <span>
                      <span className={severityBadgeClass(item.webhookSeverity)}>
                        {formatText(item.webhookSeverity)}
                      </span>
                      {item.alertDue ? (
                        <span className="nt-badge nt-badge--warning">
                          {t("到期", "Due")}
                        </span>
                      ) : null}
                    </span>
                    <span>{formatTimestamp(item.nextAlertDueAt)}</span>
                    <span title={item.remediationActionKeys.join(", ")}>
                      {item.remediationActionKeys.length === 0
                        ? PLACEHOLDER
                        : formatCount(item.remediationActionKeys.length)}
                    </span>
                  </div>
                ))}
              </div>
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
            <div className="nt-pilot-stats-state" role="status">
              {t("暂无异常事件", "No incidents yet")}
            </div>
          ) : (
            <div className="nt-table nt-table--incidents">
              <div className="nt-table__head">
                <span>{t("代码", "Code")}</span>
                <span>{t("等级", "Severity")}</span>
                <span>{t("状态", "Status")}</span>
                <span>{t("最近出现", "Last seen")}</span>
                <span>{t("操作", "Actions")}</span>
              </div>
              {sortedIncidents.map((incident) => {
                const busy = incidentBusyId === incident.id;
                const normalized = incident.status.toLowerCase();
                const resolved = normalized === "resolved";
                return (
                  <div className="nt-table__row" key={incident.id}>
                    <span title={incident.summary}>{formatText(incident.code)}</span>
                    <span>
                      <span className={severityBadgeClass(incident.severity)}>
                        {formatText(incident.severity)}
                      </span>
                    </span>
                    <span>
                      <span className={statusBadgeClass(incident.status)}>
                        {formatText(incident.status)}
                      </span>
                      {incident.escalationStatus !== "none" ? (
                        <span className="nt-chip nt-chip--muted">
                          {formatText(incident.escalationStatus)}
                        </span>
                      ) : null}
                    </span>
                    <span>{formatTimestamp(incident.lastSeenAt)}</span>
                    <span className="nt-console-row-actions">
                      <button
                        className="nt-btn nt-btn--secondary"
                        disabled={editorLocked || busy || normalized !== "open"}
                        onClick={() => onAcknowledgeIncident(incident.id)}
                        type="button"
                      >
                        {t("确认", "Acknowledge")}
                      </button>
                      <button
                        className="nt-btn nt-btn--outline"
                        disabled={editorLocked || busy || resolved}
                        onClick={() => onResolveIncident(incident.id)}
                        type="button"
                      >
                        {t("解决", "Resolve")}
                      </button>
                    </span>
                  </div>
                );
              })}
            </div>
          )
        }
      </Panel>

      <Panel
        emptyLabel={t("暂无异常策略", "No policies yet")}
        state={policies}
        t={t}
        title={t("异常策略", "Anomaly policies")}
      >
        {(rows) =>
          rows.length === 0 ? (
            <div className="nt-pilot-stats-state" role="status">
              {t("暂无异常策略", "No policies yet")}
            </div>
          ) : (
            <div className="nt-table nt-table--policies">
              <div className="nt-table__head">
                <span>{t("策略", "Policy")}</span>
                <span>{t("状态", "Status")}</span>
                <span>{t("自动化", "Automation")}</span>
                <span>{t("下次同步", "Next sync")}</span>
              </div>
              {rows.map((policy) => (
                <div className="nt-table__row" key={policy.id}>
                  <span title={policy.id}>{formatText(policy.name)}</span>
                  <span>
                    <span className={statusBadgeClass(policy.status)}>
                      {formatText(policy.status)}
                    </span>
                  </span>
                  <span className="nt-console-chip-row">
                    {policy.autoSyncEnabled ? (
                      <span className="nt-chip nt-chip--muted">{t("同步", "Sync")}</span>
                    ) : null}
                    {policy.alertingEnabled ? (
                      <span className="nt-chip nt-chip--muted">{t("告警", "Alerts")}</span>
                    ) : null}
                    {policy.autoEscalateEnabled ? (
                      <span className="nt-chip nt-chip--muted">{t("升级", "Escalate")}</span>
                    ) : null}
                    {policy.autoRemediationEnabled ? (
                      <span className="nt-chip nt-chip--muted">{t("处置", "Remediate")}</span>
                    ) : null}
                    {!policy.autoSyncEnabled &&
                    !policy.alertingEnabled &&
                    !policy.autoEscalateEnabled &&
                    !policy.autoRemediationEnabled
                      ? PLACEHOLDER
                      : null}
                  </span>
                  <span>
                    {formatTimestamp(policy.nextSyncDueAt)}
                    {policy.syncDue ? (
                      <span className="nt-badge nt-badge--warning">{t("到期", "Due")}</span>
                    ) : null}
                  </span>
                </div>
              ))}
            </div>
          )
        }
      </Panel>
    </>
  );
}
