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

type Props = Pick<OperationsWorkspaceProps,
  | "t"
  | "pressure"
  | "readiness"
  | "operatorSummary"
>;

export function OperationsLivePanels({
  t,
  pressure,
  readiness,
  operatorSummary,
}: Props) {
  return (
    <>
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
                label={t("项目并发", "Project concurrency")}
                value={formatCount(data.totalProjectConcurrency)}
              />
              <StatCard
                label={t("服务商并发", "Provider concurrency")}
                value={formatCount(data.totalProviderConcurrency)}
              />
              <StatCard
                label={t("熔断账号", "Open breakers")}
                tone={
                  data.providers.some((provider) => provider.breakerOpen) ? "danger" : "default"
                }
                value={formatCount(
                  data.providers.filter((provider) => provider.breakerOpen).length,
                )}
              />
            </div>
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
                  <div className="nt-table__row" key={provider.providerAccountId}>
                    <span title={provider.providerAccountId}>{formatText(provider.label)}</span>
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
                    <span title={project.projectId}>{formatText(project.displayName)}</span>
                    <span>{formatCount(project.activeConcurrency)}</span>
                    <span>{formatCount(project.runningRequestCount)}</span>
                  </div>
                ))}
              </div>
            ) : null}
          </>
        )}
      </Panel>

      <Panel state={readiness} t={t} title={t("依赖与就绪", "Dependencies & readiness")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                label={t("整体状态", "Overall")}
                tone={data.ok ? "emerald" : "danger"}
                value={data.ok ? t("就绪", "Ready") : t("未就绪", "Not ready")}
              />
              <StatCard
                label={t("排空中", "Draining")}
                tone={data.draining ? "danger" : "default"}
                value={data.draining ? t("是", "Yes") : t("否", "No")}
                hint={data.drainReason ?? undefined}
              />
              <StatCard
                label={t("活跃请求", "Active requests")}
                value={formatCount(data.activeRequests)}
              />
              <StatCard
                label={t("排空开始", "Drain started")}
                value={formatTimestamp(data.drainStartedAt)}
              />
            </div>
            <div className="nt-table nt-table--readiness">
              <div className="nt-table__head">
                <span>{t("依赖", "Dependency")}</span>
                <span>{t("状态", "State")}</span>
                <span>{t("说明", "Detail")}</span>
              </div>
              <DependencyRow dependency={data.dependencies.postgresql} name="PostgreSQL" t={t} />
              <DependencyRow dependency={data.dependencies.redis} name="Redis" t={t} />
              <DependencyRow
                dependency={data.dependencies.objectStorage}
                name={t("对象存储", "Object storage")}
                t={t}
              />
            </div>
            <dl className="nt-pilot-metric-list">
              <div>
                <dt>{t("API 密钥密文", "API key secret")}</dt>
                <dd>{data.checks.apiKeySecret ? t("已配置", "Set") : t("缺失", "Missing")}</dd>
              </div>
              <div>
                <dt>{t("公开访问地址", "Public base URL")}</dt>
                <dd>{data.checks.publicBaseUrl ? t("已配置", "Set") : t("缺失", "Missing")}</dd>
              </div>
            </dl>
          </>
        )}
      </Panel>

      <Panel state={operatorSummary} t={t} title={t("运行概况", "Operator summary")}>
        {(data) => (
          <>
            <div className="nt-pilot-stats-overview-grid">
              <StatCard
                hint={`${data.build.target.os}/${data.build.target.arch}`}
                label={t("版本", "Version")}
                value={formatText(data.build.version)}
              />
              <StatCard
                hint={data.lifecycle.draining ? t("排空中", "Draining") : undefined}
                label={t("生命周期", "Lifecycle")}
                tone={data.lifecycle.draining ? "danger" : "emerald"}
                value={formatText(data.lifecycle.state)}
              />
              <StatCard
                label={t("已发布模型", "Published models")}
                value={formatCount(data.routing.publishedModelCount)}
              />
              <StatCard
                label={t("服务商账号", "Provider accounts")}
                value={formatCount(data.routing.providerCount)}
              />
            </div>
            <dl className="nt-pilot-metric-list">
              <div>
                <dt>{t("采集时间", "Generated at")}</dt>
                <dd>{formatTimestamp(data.generatedAt)}</dd>
              </div>
              <div>
                <dt>{t("角色 / 端口", "Role / port")}</dt>
                <dd>{`${formatText(data.runtime.role)} : ${data.runtime.port}`}</dd>
              </div>
              <div>
                <dt>{t("累计请求", "Requests total")}</dt>
                <dd>{formatCount(data.requestMetrics.requestsTotal)}</dd>
              </div>
              <div>
                <dt>{t("累计错误", "Errors total")}</dt>
                <dd>{formatCount(data.requestMetrics.requestErrorsTotal)}</dd>
              </div>
              <div>
                <dt>{t("处理中", "In flight")}</dt>
                <dd>{formatCount(data.requestMetrics.requestInFlight)}</dd>
              </div>
              <div>
                <dt>{t("平均耗时", "Average duration")}</dt>
                <dd>
                  {formatMs(
                    averageDurationMs(
                      data.requestMetrics.requestDurationMsSum,
                      data.requestMetrics.requestDurationMsCount,
                    ),
                  )}
                </dd>
              </div>
              <div>
                <dt>{t("限流拒绝", "Rate-limit rejections")}</dt>
                <dd>{formatCount(data.requestMetrics.rateLimitRejectionsTotal)}</dd>
              </div>
              <div>
                <dt>{t("排空拒绝", "Drain rejections")}</dt>
                <dd>{formatCount(data.requestMetrics.requestDrainRejectionsTotal)}</dd>
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
          </>
        )}
      </Panel>
    </>
  );
}
