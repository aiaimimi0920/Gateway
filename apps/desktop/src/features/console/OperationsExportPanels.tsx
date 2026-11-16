import { Badge, DataTable, EmptyState } from "@neuro/ui";
import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  formatCount,
  formatTimestamp,
  formatText,
  statusTone,
  StatCard,
  Panel,
  BucketList,
} from "./OperationsPrimitives";

type Props = Pick<
  OperationsWorkspaceProps,
  "t" | "hotspots" | "exportInventory" | "exports"
>;

export function OperationsExportPanels({
  t,
  hotspots,
  exportInventory,
  exports,
}: Props) {
  return (
    <>
      <Panel
        collapsed={!hotspots.data?.totalRateLimitedRequests}
        state={hotspots}
        t={t}
        title={t("限流热点", "Rate-limit hotspots")}
      >
        {(data) => (
          <>
            <div className="nt-operations-stats">
              <StatCard
                label={t("限流请求", "Rate-limited")}
                tone={data.totalRateLimitedRequests > 0 ? "danger" : "emerald"}
                value={formatCount(data.totalRateLimitedRequests)}
              />
              <StatCard
                label={t("命中错误码", "Error codes")}
                value={formatCount(data.byCode.length)}
              />
              <StatCard
                label={t("受影响项目", "Projects")}
                value={formatCount(data.byProject.length)}
              />
              <StatCard
                label={t("受影响模型", "Models")}
                tone="blue"
                value={formatCount(data.byResolvedModel.length)}
              />
            </div>
            <div className="nt-operations-buckets">
              <BucketList
                buckets={data.byCode}
                t={t}
                title={t("按错误码", "By code")}
              />
              <BucketList
                buckets={data.byProject}
                t={t}
                title={t("按项目", "By project")}
              />
              <BucketList
                buckets={data.byResolvedModel}
                t={t}
                title={t("按模型", "By model")}
              />
              <BucketList
                buckets={data.byRoutePolicyId}
                t={t}
                title={t("按路由策略", "By route policy")}
              />
            </div>
          </>
        )}
      </Panel>

      <Panel
        collapsed
        state={exportInventory}
        t={t}
        title={t("导出台账", "Export inventory")}
      >
        {(data) => (
          <>
            <div className="nt-operations-stats">
              <StatCard
                label={t("导出数", "Exports")}
                value={formatCount(data.totalExports)}
              />
              <StatCard
                label={t("有效", "Active")}
                tone="emerald"
                value={formatCount(data.activeExports)}
              />
              <StatCard
                label={t("24 小时内过期", "Expiring in 24h")}
                tone={data.expiringWithin24Hours > 0 ? "danger" : "default"}
                value={formatCount(data.expiringWithin24Hours)}
              />
              <StatCard
                label={t("样本总数", "Samples")}
                tone="blue"
                value={formatCount(data.totalSampleCount)}
              />
            </div>
            <dl className="nt-operations-metrics">
              <div>
                <dt>{t("已删除", "Deleted")}</dt>
                <dd>{formatCount(data.deletedExports)}</dd>
              </div>
              <div>
                <dt>{t("已固定", "Pinned")}</dt>
                <dd>{formatCount(data.pinnedExports)}</dd>
              </div>
              <div>
                <dt>{t("过期未清理", "Expired but active")}</dt>
                <dd>{formatCount(data.expiredActiveExports)}</dd>
              </div>
              <div>
                <dt>{t("请求工件", "Request artifacts")}</dt>
                <dd>{formatCount(data.totalRequestArtifactCount)}</dd>
              </div>
              <div>
                <dt>{t("响应工件", "Response artifacts")}</dt>
                <dd>{formatCount(data.totalResponseArtifactCount)}</dd>
              </div>
            </dl>
          </>
        )}
      </Panel>

      <Panel
        collapsed
        emptyLabel={t("暂无导出记录", "No exports yet")}
        state={exports}
        t={t}
        title={t("导出记录", "Exports")}
      >
        {(rows) =>
          rows.length === 0 ? (
            <div role="status"><EmptyState compact title={t("暂无导出记录", "No exports yet")} /></div>
          ) : (
            <div className="nt-operations-table nt-operations-table--exports"><DataTable label={t("导出记录", "Exports")} rows={rows.slice(0, 20)} getRowKey={(entry) => entry.exportId} columns={[
              {
                key: "col-0", header: <>{t("标签", "Label")}</>, width: "34%", rowHeader: true, cell: (entry) => (<span title={entry.exportId}>
                  {formatText(entry.label ?? entry.exportId)}
                </span>)
              },
              {
                key: "col-1", header: <>{t("状态", "Status")}</>, width: "20%", cell: (entry) => (<span>
                  <Badge tone={statusTone(entry.status)}>
                    {formatText(entry.status)}
                  </Badge>
                </span>)
              },
              { key: "col-2", header: <>{t("样本", "Samples")}</>, width: "16%", cell: (entry) => (<span>{formatCount(entry.sampleCount)}</span>) },
              {
                key: "col-3", header: <>{t("生成时间", "Created at")}</>, width: "30%", cell: (entry) => (<span title={formatText(entry.retentionExpiresAt)}>
                  {formatTimestamp(entry.createdAt)}
                </span>)
              }
            ]} /></div>
          )
        }
      </Panel>
    </>
  );
}
