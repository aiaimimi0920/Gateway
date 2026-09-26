import type { OperationsWorkspaceProps, OperationsRequestFilterDraft } from "./operations-contracts";
import { OPERATIONS_DEFAULT_REQUEST_FILTERS } from "./operations-contracts";
import {
  PLACEHOLDER,
  formatCount,
  formatUsd,
  formatRate,
  formatMs,
  formatTimestamp,
  formatText,
  statusBadgeClass,
  StatCard,
  Panel,
} from "./OperationsPrimitives";

type Props = Pick<OperationsWorkspaceProps,
  | "t"
  | "editorLocked"
  | "requests"
  | "requestSummary"
  | "usageSummary"
  | "promptCache"
  | "requestFilters"
  | "onRequestFiltersChange"
  | "onApplyRequestFilters"
> & {
  patchFilters: (patch: Partial<OperationsRequestFilterDraft>) => void;
};

export function OperationsRequestPanels({
  t,
  editorLocked,
  requests,
  requestSummary,
  usageSummary,
  promptCache,
  requestFilters,
  onRequestFiltersChange,
  onApplyRequestFilters,
  patchFilters,
}: Props) {
  return (
    <>
      <article className="nt-pilot-metric-card">
        <h3>{t("筛选", "Filters")}</h3>
        <div className="nt-console-field-grid">
          <label className="nt-field">
            <span>{t("状态", "Status")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchFilters({ status: event.target.value })}
              placeholder="completed / failed"
              value={requestFilters.status}
            />
          </label>
          <label className="nt-field">
            <span>{t("项目", "Project")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchFilters({ projectId: event.target.value })}
              value={requestFilters.projectId}
            />
          </label>
          <label className="nt-field">
            <span>{t("服务商账号", "Provider account")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchFilters({ providerAccountId: event.target.value })}
              value={requestFilters.providerAccountId}
            />
          </label>
          <label className="nt-field">
            <span>{t("端点类型", "Endpoint kind")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchFilters({ endpointKind: event.target.value })}
              placeholder="chat / embeddings"
              value={requestFilters.endpointKind}
            />
          </label>
          <label className="nt-field">
            <span>{t("错误码", "Error code")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchFilters({ errorCode: event.target.value })}
              value={requestFilters.errorCode}
            />
          </label>
          <label className="nt-field">
            <span>{t("条数上限", "Limit")}</span>
            <input
              className="nt-input"
              inputMode="numeric"
              onChange={(event) => patchFilters({ limit: event.target.value })}
              value={requestFilters.limit}
            />
          </label>
        </div>
        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={editorLocked || requests.loading}
            onClick={onApplyRequestFilters}
            type="button"
          >
            {requests.loading ? t("查询中…", "Querying…") : t("应用筛选", "Apply filters")}
          </button>
          <button
            className="nt-btn nt-btn--secondary"
            disabled={editorLocked}
            onClick={() => onRequestFiltersChange({ ...OPERATIONS_DEFAULT_REQUEST_FILTERS })}
            type="button"
          >
            {t("重置", "Reset")}
          </button>
        </div>
      </article>

      <Panel state={requestSummary} t={t} title={t("请求汇总", "Request summary")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("总请求", "Requests")}
                value={formatCount(data.totalRequests)}
              />
              <StatCard
                label={t("成功", "Completed")}
                tone="emerald"
                value={formatCount(data.completedCount)}
                hint={
                  data.totalRequests > 0
                    ? formatRate(data.completedCount / data.totalRequests)
                    : undefined
                }
              />
              <StatCard
                label={t("失败", "Failed")}
                tone={data.failedCount > 0 ? "danger" : "default"}
                value={formatCount(data.failedCount)}
              />
              <StatCard
                label={t("进行中", "Running")}
                tone="blue"
                value={formatCount(data.runningCount)}
              />
            </div>
            <dl className="nt-pilot-metric-list">
              <div>
                <dt>{t("已取消", "Cancelled")}</dt>
                <dd>{formatCount(data.cancelledCount)}</dd>
              </div>
              <div>
                <dt>{t("可回退失败", "Fallback-eligible failures")}</dt>
                <dd>{formatCount(data.fallbackEligibleFailures)}</dd>
              </div>
              <div>
                <dt>{t("回退耗尽失败", "Fallback-exhausted failures")}</dt>
                <dd>{formatCount(data.fallbackExhaustedFailures)}</dd>
              </div>
            </dl>
            {data.byErrorCode.length > 0 ? (
              <div className="nt-console-bucket-group">
                <h4>{t("错误码分布", "By error code")}</h4>
                <dl className="nt-pilot-metric-list">
                  {data.byErrorCode.slice(0, 8).map((bucket) => (
                    <div key={bucket.value}>
                      <dt title={bucket.value}>{formatText(bucket.value)}</dt>
                      <dd>{formatCount(bucket.count)}</dd>
                    </div>
                  ))}
                </dl>
              </div>
            ) : null}
          </>
        )}
      </Panel>

      <Panel state={usageSummary} t={t} title={t("用量归档", "Usage archive")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("队列深度", "Queue depth")}
                /* -1 means the queue could not be read, not an empty queue. */
                value={data.queueDepth < 0 ? PLACEHOLDER : formatCount(data.queueDepth)}
                hint={data.queueDepth < 0 ? t("Redis 不可用", "Redis unavailable") : undefined}
              />
              <StatCard
                label={t("近期请求", "Recent requests")}
                value={formatCount(data.recentRequestCount)}
              />
              <StatCard
                label={t("近期失败", "Recent failures")}
                tone={data.recentFailureCount > 0 ? "danger" : "default"}
                value={formatCount(data.recentFailureCount)}
              />
              <StatCard
                label={t("近期 Tokens", "Recent tokens")}
                tone="blue"
                value={formatCount(data.recentTotalTokens)}
              />
            </div>
            <dl className="nt-pilot-metric-list">
              <div>
                <dt>{t("归档失败数", "Archive failures")}</dt>
                <dd>{formatCount(data.archiveFailureCount)}</dd>
              </div>
            </dl>
            {data.alerts.length > 0 ? (
              <ul className="nt-validation-list nt-validation-list--warning">
                {data.alerts.map((alert) => (
                  <li key={`${alert.code}-${alert.message}`}>
                    <strong>{alert.severity}</strong> {alert.message}
                  </li>
                ))}
              </ul>
            ) : null}
          </>
        )}
      </Panel>

      <Panel state={promptCache} t={t} title={t("提示缓存", "Prompt cache")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("命中率", "Hit rate")}
                tone="emerald"
                value={formatRate(data.cacheHitRate)}
              />
              <StatCard
                label={t("命中请求", "Cache hits")}
                value={formatCount(data.cacheHitRequests)}
              />
              <StatCard
                label={t("节省 Tokens", "Tokens saved")}
                value={formatCount(data.totalTokensSaved)}
              />
              <StatCard
                label={t("节省成本", "Estimated savings")}
                tone="blue"
                value={formatUsd(data.estimatedCostSavedUsd)}
              />
            </div>
            <dl className="nt-pilot-metric-list">
              <div>
                <dt>{t("样本请求", "Sampled requests")}</dt>
                <dd>{formatCount(data.totalRequests)}</dd>
              </div>
              <div>
                <dt>{t("缓存写入请求", "Cache-creation requests")}</dt>
                <dd>{formatCount(data.cacheCreationRequests)}</dd>
              </div>
              <div>
                <dt>{t("客户端标注", "Client-marked")}</dt>
                <dd>{formatCount(data.clientMarkedRequests)}</dd>
              </div>
              <div>
                <dt>{t("网关自动标注", "Auto-applied")}</dt>
                <dd>{formatCount(data.autoAppliedRequests)}</dd>
              </div>
              <div>
                <dt>{t("缓存指令覆盖率", "Cache-control coverage")}</dt>
                <dd>{formatRate(data.cacheControlCoverageRate)}</dd>
              </div>
              <div>
                <dt>{t("输入价格 / 百万", "Input price / M")}</dt>
                <dd>{formatUsd(data.inputPricePerMillion)}</dd>
              </div>
            </dl>
          </>
        )}
      </Panel>

      <Panel
        emptyLabel={t("没有匹配的请求", "No matching requests")}
        state={requests}
        t={t}
        title={t("请求列表", "Requests")}
      >
        {(rows) =>
          rows.length === 0 ? (
            <div className="nt-pilot-stats-state" role="status">
              {t("没有匹配的请求", "No matching requests")}
            </div>
          ) : (
            <div className="nt-table nt-table--requests">
              <div className="nt-table__head">
                <span>{t("时间", "Time")}</span>
                <span>{t("模型", "Model")}</span>
                <span>{t("状态", "Status")}</span>
                <span>{t("耗时", "Duration")}</span>
                <span>{t("Tokens", "Tokens")}</span>
                <span>{t("上游", "Upstream")}</span>
              </div>
              {rows.map((row) => (
                <div className="nt-table__row" key={row.id}>
                  <span>{formatTimestamp(row.createdAt)}</span>
                  <span title={formatText(row.requestedModel)}>
                    {formatText(row.resolvedModel ?? row.requestedModel)}
                  </span>
                  <span>
                    <span className={statusBadgeClass(row.status)}>
                      {formatText(row.status)}
                    </span>
                    {row.stream ? (
                      <span className="nt-chip nt-chip--muted">{t("流式", "Stream")}</span>
                    ) : null}
                  </span>
                  <span>{formatMs(row.durationMs)}</span>
                  <span>{formatCount(row.totalTokens)}</span>
                  <span title={formatText(row.errorSummary)}>
                    {row.upstreamStatus === null ? PLACEHOLDER : row.upstreamStatus}
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
