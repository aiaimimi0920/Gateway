import { Badge, DataTable, EmptyState } from "@neuro/ui";
import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  formatCount,
  formatTimestamp,
  formatText,
  shortId,
  statusTone,
  StatCard,
  Panel,
  BucketList,
} from "./OperationsPrimitives";

type Props = Pick<
  OperationsWorkspaceProps,
  "t" | "remediationEffectiveness" | "remediationQueue" | "remediationRuns"
>;

export function OperationsRemediationPanels({
  t,
  remediationEffectiveness,
  remediationQueue,
  remediationRuns,
}: Props) {
  return (
    <>
      <Panel
        collapsed
        state={remediationEffectiveness}
        t={t}
        title={t("处置成效", "Remediation impact")}
      >
        {(data) => (
          <>
            <div className="nt-operations-stats">
              <StatCard
                label={t("执行次数", "Runs")}
                value={formatCount(data.totalRuns)}
              />
              <StatCard
                label={t("已评估", "Measured")}
                tone="blue"
                value={formatCount(data.impactedRuns)}
              />
              <StatCard
                label={t("失败率改善", "Failure rate improved")}
                tone="emerald"
                value={formatCount(data.failureRate.improvedRuns)}
              />
              <StatCard
                label={t("失败率恶化", "Failure rate regressed")}
                tone={data.failureRate.regressedRuns > 0 ? "danger" : "default"}
                value={formatCount(data.failureRate.regressedRuns)}
              />
            </div>
            <dl className="nt-operations-metrics">
              <div>
                <dt>{t("统计窗口", "Window")}</dt>
                <dd>{`${data.windowMinutes} min`}</dd>
              </div>
              <div>
                <dt>{t("无法评估", "Unavailable")}</dt>
                <dd>{formatCount(data.unavailableRuns)}</dd>
              </div>
              <div>
                <dt>{t("完成率改善", "Completion improved")}</dt>
                <dd>{formatCount(data.completionRate.improvedRuns)}</dd>
              </div>
              <div>
                <dt>{t("首字延迟改善", "First-token improved")}</dt>
                <dd>{formatCount(data.firstTokenLatencyMsAvg.improvedRuns)}</dd>
              </div>
            </dl>
            <BucketList
              buckets={data.byActionKey}
              t={t}
              title={t("按动作", "By action")}
            />
            <BucketList
              buckets={data.byStatus}
              t={t}
              title={t("按状态", "By status")}
            />
          </>
        )}
      </Panel>

      <Panel
        collapsed={!remediationQueue.data?.itemCount}
        state={remediationQueue}
        t={t}
        title={t("处置队列", "Remediation queue")}
      >
        {(data) => (
          <>
            <div className="nt-operations-stats">
              <StatCard
                label={t("队列项", "Queued items")}
                value={formatCount(data.itemCount)}
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
              <div role="status"><EmptyState compact title={t("处置队列为空", "Remediation queue is empty")} /></div>
            ) : (
              <div className="nt-operations-table nt-operations-table--remediation-queue"><DataTable label={t("待处置动作", "Queued remediation")} rows={data.items} getRowKey={(item) => `${item.incident.id}-${item.action.actionKey}`} columns={[
                {
                  key: "col-0", header: <>{t("动作", "Action")}</>, width: "30%", rowHeader: true, cell: (item) => (<span title={item.action.description}>
                    {formatText(item.action.title || item.action.actionKey)}
                  </span>)
                },
                {
                  key: "col-1", header: <>{t("事件", "Incident")}</>, width: "22%", cell: (item) => (<span title={item.incident.id}>
                    {formatText(item.incident.code)}
                  </span>)
                },
                {
                  key: "col-2", header: <>{t("下次执行", "Next run")}</>, width: "24%", cell: (item) => (<span className="n-row">
                    {formatTimestamp(item.nextRunDueAt)}
                    {item.remediationDue ? (
                      <Badge tone="warning">
                        {t("到期", "Due")}
                      </Badge>
                    ) : null}
                  </span>)
                },
                {
                  key: "col-3", header: <>{t("阻塞原因", "Blocked by")}</>, width: "24%", cell: (item) => (<span className="n-muted">
                    {formatText(item.blockedReason)}
                  </span>)
                }
              ]} /></div>
            )}
          </>
        )}
      </Panel>

      <Panel
        collapsed
        emptyLabel={t("暂无处置记录", "No remediation runs yet")}
        state={remediationRuns}
        t={t}
        title={t("处置记录", "Remediation runs")}
      >
        {(rows) =>
          rows.length === 0 ? (
            <div role="status"><EmptyState compact title={t("暂无处置记录", "No remediation runs yet")} /></div>
          ) : (
            <div className="nt-operations-table nt-operations-table--remediation-runs"><DataTable label={t("处置记录", "Remediation runs")} rows={rows} getRowKey={(run) => run.id} columns={[
              {
                key: "col-0", header: <>{t("动作", "Action")}</>, width: "30%", rowHeader: true, cell: (run) => (<span title={run.actionKey}>
                  {formatText(run.title || run.actionKey)}
                </span>)
              },
              {
                key: "col-1", header: <>{t("状态", "Status")}</>, width: "24%", cell: (run) => (<span className="n-row">
                  <Badge tone={statusTone(run.status)}>
                    {formatText(run.status)}
                  </Badge>
                  {run.dryRun ? (
                    <Badge>
                      {t("演练", "Dry run")}
                    </Badge>
                  ) : null}
                </span>)
              },
              { key: "col-2", header: <>{t("事件", "Incident")}</>, width: "22%", cell: (run) => (<span title={run.incidentId}>{shortId(run.incidentId)}</span>) },
              {
                key: "col-3", header: <>{t("执行时间", "Executed at")}</>, width: "24%", cell: (run) => (<span title={formatText(run.errorSummary)}>
                  {formatTimestamp(run.completedAt ?? run.createdAt)}
                </span>)
              }
            ]} /></div>
          )
        }
      </Panel>
    </>
  );
}
