import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  formatCount,
  formatMs,
  formatTimestamp,
  formatText,
  averageDurationMs,
  statusBadgeClass,
  StatCard,
  Panel,
  DependencyRow,
} from "./OperationsPrimitives";

type Props = Pick<
  OperationsWorkspaceProps,
  "t" | "pressure" | "readiness" | "operatorSummary"
>;

export function OperationsLivePanels({
  t,
  pressure,
  readiness,
  operatorSummary,
}: Props) {
  return (
    <div className="nt-operations-live">
      <Panel state={pressure} t={t} title={t("实时压力", "Runtime pressure")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("进行中请求", "Running requests")}
                tone="blue"
                value={formatCount(data.totalRunningRequests)}
              />
              <StatCard
                label={t("熔断账号", "Open breakers")}
                tone={
                  data.providers.some((provider) => provider.breakerOpen)
                    ? "danger"
                    : "default"
                }
                value={formatCount(
                  data.providers.filter((provider) => provider.breakerOpen)
                    .length,
                )}
              />
            </div>
            <details
              className="nt-operations-detail"
              open={data.providers.some((provider) => provider.breakerOpen)}
            >
              <summary>{t("账号与项目负载", "Account & project load")}</summary>
              <div className="nt-operations-detail__body">
                <dl className="nt-pilot-metric-list">
                  <div>
                    <dt>{t("项目并发", "Project concurrency")}</dt>
                    <dd>{formatCount(data.totalProjectConcurrency)}</dd>
                  </div>
                  <div>
                    <dt>{t("服务商并发", "Provider concurrency")}</dt>
                    <dd>{formatCount(data.totalProviderConcurrency)}</dd>
                  </div>
                </dl>
                {data.providers.length === 0 ? (
                  <div className="nt-pilot-stats-state" role="status">
                    {t("暂无活跃服务商账号", "No active provider accounts")}
                  </div>
                ) : (
                  <div className="nt-table nt-table--pressure">
                    <div className="nt-table__head">
                      <span>{t("服务商账号", "Provider account")}</span>
                      <span>{t("状态", "Status")}</span>
                      <span>{t("并发", "Concurrency")}</span>
                      <span>{t("进行中", "Running")}</span>
                    </div>
                    {data.providers.map((provider) => (
                      <div
                        className="nt-table__row"
                        key={provider.providerAccountId}
                      >
                        <span title={provider.providerAccountId}>
                          {formatText(provider.label)}
                        </span>
                        <span>
                          <span className={statusBadgeClass(provider.status)}>
                            {formatText(provider.status)}
                          </span>
                          {provider.breakerOpen ? (
                            <span className="nt-badge nt-badge--danger">
                              {t("熔断", "Breaker")}
                            </span>
                          ) : null}
                        </span>
                        <span>
                          {provider.concurrencyLimit === null
                            ? formatCount(provider.activeConcurrency)
                            : `${provider.activeConcurrency} / ${provider.concurrencyLimit}`}
                        </span>
                        <span>{formatCount(provider.runningRequestCount)}</span>
                      </div>
                    ))}
                  </div>
                )}
                {data.projects.length > 0 ? (
                  <div className="nt-table nt-table--pressure-projects">
                    <div className="nt-table__head">
                      <span>{t("项目", "Project")}</span>
                      <span>{t("并发", "Concurrency")}</span>
                      <span>{t("进行中", "Running")}</span>
                    </div>
                    {data.projects.map((project) => (
                      <div className="nt-table__row" key={project.projectId}>
                        <span title={project.projectId}>
                          {formatText(project.displayName)}
                        </span>
                        <span>{formatCount(project.activeConcurrency)}</span>
                        <span>{formatCount(project.runningRequestCount)}</span>
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
            </details>
          </>
        )}
      </Panel>

      <Panel state={readiness} t={t} title={t("服务状态", "Service status")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("整体状态", "Overall")}
                tone={data.ok ? "emerald" : "danger"}
                value={data.ok ? t("就绪", "Ready") : t("未就绪", "Not ready")}
              />
              {data.draining ? (
                <StatCard
                  label={t("排空中", "Draining")}
                  tone="danger"
                  value={formatCount(data.activeRequests)}
                  hint={[data.drainReason, formatTimestamp(data.drainStartedAt)]
                    .filter(Boolean)
                    .join(" · ")}
                />
              ) : null}
            </div>
            <details
              className="nt-operations-detail"
              open={!data.ok || data.draining}
            >
              <summary>
                {t("依赖与配置检查", "Dependency & configuration checks")}
              </summary>
              <div className="nt-operations-detail__body">
                <div className="nt-table nt-table--readiness">
                  <div className="nt-table__head">
                    <span>{t("依赖", "Dependency")}</span>
                    <span>{t("状态", "State")}</span>
                    <span>{t("说明", "Detail")}</span>
                  </div>
                  {data.dependencies.sqlite ? (
                    <DependencyRow
                      dependency={data.dependencies.sqlite}
                      name="SQLite"
                      t={t}
                    />
                  ) : null}
                  {data.dependencies.postgresql.configured ||
                  data.dependencies.postgresql.required ? (
                    <DependencyRow
                      dependency={data.dependencies.postgresql}
                      name="PostgreSQL"
                      t={t}
                    />
                  ) : null}
                  {data.dependencies.redis.configured ||
                  data.dependencies.redis.required ? (
                    <DependencyRow
                      dependency={data.dependencies.redis}
                      name="Redis"
                      t={t}
                    />
                  ) : null}
                  <DependencyRow
                    dependency={data.dependencies.objectStorage}
                    name={t("对象存储", "Object storage")}
                    t={t}
                  />
                </div>
                <dl className="nt-pilot-metric-list">
                  <div>
                    <dt>{t("API 密钥密文", "API key secret")}</dt>
                    <dd>
                      {data.checks.apiKeySecret
                        ? t("已配置", "Set")
                        : t("缺失", "Missing")}
                    </dd>
                  </div>
                  <div>
                    <dt>{t("公开访问地址", "Public base URL")}</dt>
                    <dd>
                      {data.checks.publicBaseUrl
                        ? t("已配置", "Set")
                        : t("缺失", "Missing")}
                    </dd>
                  </div>
                </dl>
              </div>
            </details>
          </>
        )}
      </Panel>

      <Panel
        state={operatorSummary}
        t={t}
        title={t("运行统计", "Runtime statistics")}
      >
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("累计请求", "Requests total")}
                value={formatCount(data.requestMetrics.requestsTotal)}
              />
              <StatCard
                label={t("累计错误", "Errors total")}
                tone={
                  data.requestMetrics.requestErrorsTotal > 0
                    ? "danger"
                    : "default"
                }
                value={formatCount(data.requestMetrics.requestErrorsTotal)}
              />
              <StatCard
                label={t("平均耗时", "Average duration")}
                value={formatMs(
                  averageDurationMs(
                    data.requestMetrics.requestDurationMsSum,
                    data.requestMetrics.requestDurationMsCount,
                  ),
                )}
              />
            </div>
            <details className="nt-operations-detail">
              <summary>{t("运行诊断", "Runtime diagnostics")}</summary>
              <dl className="nt-pilot-metric-list">
                <div>
                  <dt>{t("版本", "Version")}</dt>
                  <dd>
                    {formatText(data.build.version)} · {data.build.target.os}/
                    {data.build.target.arch}
                  </dd>
                </div>
                <div>
                  <dt>{t("生命周期", "Lifecycle")}</dt>
                  <dd>{formatText(data.lifecycle.state)}</dd>
                </div>
                <div>
                  <dt>{t("已发布模型", "Published models")}</dt>
                  <dd>{formatCount(data.routing.publishedModelCount)}</dd>
                </div>
                <div>
                  <dt>{t("服务商账号", "Provider accounts")}</dt>
                  <dd>{formatCount(data.routing.providerCount)}</dd>
                </div>
                <div>
                  <dt>{t("采集时间", "Generated at")}</dt>
                  <dd>{formatTimestamp(data.generatedAt)}</dd>
                </div>
                <div>
                  <dt>{t("角色 / 端口", "Role / port")}</dt>
                  <dd>{`${formatText(data.runtime.role)} : ${data.runtime.port}`}</dd>
                </div>
                <div>
                  <dt>{t("处理中", "In flight")}</dt>
                  <dd>{formatCount(data.requestMetrics.requestInFlight)}</dd>
                </div>
                <div>
                  <dt>{t("限流拒绝", "Rate-limit rejections")}</dt>
                  <dd>
                    {formatCount(data.requestMetrics.rateLimitRejectionsTotal)}
                  </dd>
                </div>
                <div>
                  <dt>{t("排空拒绝", "Drain rejections")}</dt>
                  <dd>
                    {formatCount(
                      data.requestMetrics.requestDrainRejectionsTotal,
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{t("凭据缓存", "Credential cache")}</dt>
                  <dd>{formatCount(data.credentialCache.entryCount)}</dd>
                </div>
                <div>
                  <dt>{t("路由条目", "Routes")}</dt>
                  <dd>{formatCount(data.routing.routeCount)}</dd>
                </div>
              </dl>
            </details>
          </>
        )}
      </Panel>
    </div>
  );
}
