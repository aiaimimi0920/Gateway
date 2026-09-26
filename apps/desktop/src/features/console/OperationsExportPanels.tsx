import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  formatCount,
  formatTimestamp,
  formatText,
  statusBadgeClass,
  StatCard,
  Panel,
  BucketList,
} from "./OperationsPrimitives";

type Props = Pick<OperationsWorkspaceProps,
  | "t"
  | "hotspots"
  | "exportInventory"
  | "exports"
>;

export function OperationsExportPanels({
  t,
  hotspots,
  exportInventory,
  exports,
}: Props) {
  return (
    <>
      <Panel state={hotspots} t={t} title={t("限流热点", "Rate-limit hotspots")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
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
            <div className="nt-console-bucket-grid">
              <BucketList buckets={data.byCode} t={t} title={t("按错误码", "By code")} />
              <BucketList buckets={data.byProject} t={t} title={t("按项目", "By project")} />
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

      <Panel state={exportInventory} t={t} title={t("导出台账", "Export inventory")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
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
            <dl className="nt-pilot-metric-list">
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
        emptyLabel={t("暂无导出记录", "No exports yet")}
        state={exports}
        t={t}
        title={t("导出记录", "Exports")}
      >
        {(rows) =>
          rows.length === 0 ? (
            <div className="nt-pilot-stats-state" role="status">
              {t("暂无导出记录", "No exports yet")}
            </div>
          ) : (
            <div className="nt-table nt-table--exports">
              <div className="nt-table__head">
                <span>{t("标签", "Label")}</span>
                <span>{t("状态", "Status")}</span>
                <span>{t("样本", "Samples")}</span>
                <span>{t("生成时间", "Created at")}</span>
              </div>
              {rows.slice(0, 20).map((entry) => (
                <div className="nt-table__row" key={entry.exportId}>
                  <span title={entry.exportId}>
                    {formatText(entry.label ?? entry.exportId)}
                  </span>
                  <span>
                    <span className={statusBadgeClass(entry.status)}>
                      {formatText(entry.status)}
                    </span>
                  </span>
                  <span>{formatCount(entry.sampleCount)}</span>
                  <span title={formatText(entry.retentionExpiresAt)}>
                    {formatTimestamp(entry.createdAt)}
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
