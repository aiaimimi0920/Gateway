import { Badge, DataTable, Disclosure, EmptyState } from "@neuro/ui";
import type { ConsoleDependencyReadiness } from "../../api/contracts";
import type { OperationsWorkspaceProps } from "./operations-contracts";
import { displayLabel } from "./displayLabel";
import { DEFAULT_OPERATIONS_LIST_VIEW, selectOperationsRows } from "./operationsListView";
import {
  formatCount,
  formatMs,
  formatTimestamp,
  formatText,
  averageDurationMs,
  statusTone,
  StatCard,
  Panel,
  DiagnosticDisclosure,
  PLACEHOLDER,
} from "./OperationsPrimitives";

type Props = Pick<
  OperationsWorkspaceProps,
  "t" | "pressure" | "readiness" | "operatorSummary" | "listView"
>;

export function OperationsLivePanels({
  t,
  pressure,
  readiness,
  operatorSummary,
  listView = DEFAULT_OPERATIONS_LIST_VIEW,
}: Props) {
  const providers = selectOperationsRows(pressure.data?.providers ?? [], listView, (row) => ({
    name: displayLabel(row.label), text: [row.providerAccountId, row.status],
  }));
  const projects = selectOperationsRows(pressure.data?.projects ?? [], listView, (row) => ({
    name: row.displayName, text: [row.projectId],
  }));
  return (
    <div className="nt-operations-live">
      <Panel state={readiness} t={t} title={t("服务状态", "Service status")}>
        {(data) => (
          <>
            <div className="nt-operations-stats">
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
            <DiagnosticDisclosure
              title={t("依赖与配置检查", "Dependency & configuration checks")}
              active={!data.ok || data.draining}
            >
              <div className="nt-operations-detail__body">
                <div className="nt-operations-table nt-operations-table--readiness">
                  <DataTable label={t("依赖状态", "Dependency state")}
                    rows={dependencyRows(data.dependencies, t)} getRowKey={row => row.id}
                    columns={[
                      { key: "name", header: t("依赖", "Dependency"), rowHeader: true, width: "30%", cell: row => row.name },
                      {
                        key: "state", header: t("状态", "State"), width: "30%", cell: ({ dependency }) =>
                          <Badge tone={dependency.ready ? "success" : dependency.configured ? "danger" : "neutral"}>
                            {dependency.ready ? t("就绪", "Ready") : dependency.configured ? t("异常", "Failing") : t("未配置", "Unconfigured")}
                          </Badge>
                      },
                      {
                        key: "notes", header: t("说明", "Detail"), width: "40%", cell: ({ dependency }) =>
                          [dependency.required ? t("必需", "Required") : null, dependency.timedOut ? t("检查超时", "Check timed out") : null, dependency.driver].filter(Boolean).join(" · ") || PLACEHOLDER
                      },
                    ]} />
                </div>
                <dl className="nt-operations-metrics">
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
            </DiagnosticDisclosure>
          </>
        )}
      </Panel>

      <Panel state={pressure} t={t} title={t("实时压力", "Runtime pressure")}>
        {(data) => (
          <>
            <div className="nt-operations-stats">
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
            <DiagnosticDisclosure
              title={t("账号与项目负载", "Account & project load")}
              active={data.providers.some((provider) => provider.breakerOpen)}
            >
              <div className="nt-operations-detail__body">
                <dl className="nt-operations-metrics">
                  <div>
                    <dt>{t("项目并发", "Project concurrency")}</dt>
                    <dd>{formatCount(data.totalProjectConcurrency)}</dd>
                  </div>
                  <div>
                    <dt>{t("服务商并发", "Provider concurrency")}</dt>
                    <dd>{formatCount(data.totalProviderConcurrency)}</dd>
                  </div>
                </dl>
                {providers.length === 0 ? (
                  <div role="status"><EmptyState compact title={data.providers.length ? t("没有匹配的服务商账号", "No matching provider accounts") : t("暂无活跃服务商账号", "No active provider accounts")} /></div>
                ) : (
                  <div className="nt-operations-table nt-operations-table--pressure"><DataTable label={t("服务商账号负载", "Provider account load")} rows={providers} getRowKey={(provider) => provider.providerAccountId} columns={[
                    {
                      key: "col-0", header: <>{t("服务商账号", "Provider account")}</>, width: "30%", rowHeader: true, cell: (provider) => (<span title={provider.providerAccountId}>
                        {displayLabel(formatText(provider.label))}
                      </span>)
                    },
                    {
                      key: "col-1", header: <>{t("状态", "Status")}</>, width: "30%", cell: (provider) => (<span className="n-row">
                        <Badge tone={statusTone(provider.status)}>
                          {formatText(provider.status)}
                        </Badge>
                        {provider.breakerOpen ? (
                          <Badge tone="danger">
                            {t("熔断", "Breaker")}
                          </Badge>
                        ) : null}
                      </span>)
                    },
                    {
                      key: "col-2", header: <>{t("并发", "Concurrency")}</>, width: "24%", cell: (provider) => (<span>
                        {provider.concurrencyLimit === null
                          ? formatCount(provider.activeConcurrency)
                          : `${provider.activeConcurrency} / ${provider.concurrencyLimit}`}
                      </span>)
                    },
                    { key: "col-3", header: <>{t("进行中", "Running")}</>, width: "16%", cell: (provider) => (<span>{formatCount(provider.runningRequestCount)}</span>) }
                  ]} /></div>
                )}
                {projects.length > 0 ? (
                  <div className="nt-operations-table nt-operations-table--pressure-projects"><DataTable label={t("项目负载", "Project load")} rows={projects} getRowKey={(project) => project.projectId} columns={[
                    {
                      key: "col-0", header: <>{t("项目", "Project")}</>, width: "50%", rowHeader: true, cell: (project) => (<span title={project.projectId}>
                        {formatText(project.displayName)}
                      </span>)
                    },
                    { key: "col-1", header: <>{t("并发", "Concurrency")}</>, width: "25%", cell: (project) => (<span>{formatCount(project.activeConcurrency)}</span>) },
                    { key: "col-2", header: <>{t("进行中", "Running")}</>, width: "25%", cell: (project) => (<span>{formatCount(project.runningRequestCount)}</span>) }
                  ]} /></div>
                ) : null}
              </div>
            </DiagnosticDisclosure>
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
            <div className="nt-operations-stats">
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
            <Disclosure className="nt-operations-detail" title={t("运行诊断", "Runtime diagnostics")}>
              <dl className="nt-operations-metrics">
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
            </Disclosure>
          </>
        )}
      </Panel>
    </div>
  );
}
function dependencyRows(dependencies: NonNullable<OperationsWorkspaceProps["readiness"]["data"]>["dependencies"], t: OperationsWorkspaceProps["t"]) {
  const rows: { id: string; name: string; dependency: ConsoleDependencyReadiness }[] = [];
  if (dependencies.sqlite) rows.push({ id: "sqlite", name: "SQLite", dependency: dependencies.sqlite });
  for (const [id, name] of [["postgresql", "PostgreSQL"], ["redis", "Redis"]] as const) {
    const dependency = dependencies[id];
    if (dependency.configured || dependency.required) rows.push({ id, name, dependency });
  }
  rows.push({ id: "object-storage", name: t("对象存储", "Object storage"), dependency: dependencies.objectStorage });
  return rows;
}
