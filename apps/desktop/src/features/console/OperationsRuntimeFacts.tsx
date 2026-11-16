import { Badge } from "@neuro/ui";
import type { OperationsWorkspaceProps } from "./operations-contracts";
import { formatCount, formatText, formatTimestamp } from "./OperationsPrimitives";

/** Small diagnostics are always visible; unknown or stale observations retain their parent error. */
export function OperationsRuntimeFacts({ pressure, readiness, operatorSummary, t }: Pick<OperationsWorkspaceProps, "pressure" | "readiness" | "operatorSummary" | "t">) {
  const ready = readiness.data;
  const summary = operatorSummary.data;
  const load = pressure.data;
  const dependencies = ready ? [
    ["SQLite", ready.dependencies.sqlite],
    ["PostgreSQL", ready.dependencies.postgresql],
    ["Redis", ready.dependencies.redis],
    [t("对象存储", "Object storage"), ready.dependencies.objectStorage],
  ] as const : [];
  return <div className="nt-operations-inline-facts">
    <div className="nt-operations-dependencies">
      {dependencies.map(([name, dependency]) => dependency && (dependency.configured || dependency.required) ? <span key={name}>
        {name} <Badge tone={dependency.ready ? "success" : "danger"}>{dependency.ready ? t("就绪", "Ready") : dependency.timedOut ? t("超时", "Timed out") : t("异常", "Failing")}</Badge>
        {dependency.driver && <small className="n-muted">{dependency.driver}</small>}
      </span> : null)}
    </div>
    <dl className="nt-operations-inline-metrics">
      {[
        ...(ready ? [
          [t("API 密钥密文", "API key secret"), ready.checks.apiKeySecret ? t("已配置", "Set") : t("缺失", "Missing")],
          [t("公开访问地址", "Public base URL"), ready.checks.publicBaseUrl ? t("已配置", "Set") : t("缺失", "Missing")],
          ...(ready.draining ? [[t("排空", "Draining"), [ready.drainReason, formatTimestamp(ready.drainStartedAt), formatCount(ready.activeRequests)].filter(Boolean).join(" · ")]] : []),
        ] : []),
        ...(load ? [[t("项目并发", "Project concurrency"), formatCount(load.totalProjectConcurrency)], [t("服务商并发", "Provider concurrency"), formatCount(load.totalProviderConcurrency)]] : []),
        ...(summary ? [
          [t("版本", "Version"), summary.build.version + " · " + summary.build.target.os + "/" + summary.build.target.arch],
          [t("生命周期", "Lifecycle"), formatText(summary.lifecycle.state)],
          [t("角色 / 端口", "Role / port"), summary.runtime.role + " : " + summary.runtime.port],
          [t("模型 / 服务商", "Models / providers"), formatCount(summary.routing.publishedModelCount) + " / " + formatCount(summary.routing.providerCount)],
          [t("限流拒绝", "Rate-limit rejections"), formatCount(summary.requestMetrics.rateLimitRejectionsTotal)],
          [t("排空拒绝", "Drain rejections"), formatCount(summary.requestMetrics.requestDrainRejectionsTotal)],
          [t("凭据缓存 / 路由", "Credential cache / routes"), formatCount(summary.credentialCache.entryCount) + " / " + formatCount(summary.routing.routeCount)],
          [t("处理中", "In flight"), formatCount(summary.requestMetrics.requestInFlight)],
          [t("采集时间", "Generated at"), formatTimestamp(summary.generatedAt)],
        ] : []),
      ].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}
    </dl>
    {!!load?.providers.length && <ul className="nt-operations-load-list">{load.providers.map(provider => <li key={provider.providerAccountId}>
      <span title={provider.providerAccountId}>{formatText(provider.label)}</span>
      <Badge tone={provider.breakerOpen ? "danger" : "neutral"}>{provider.breakerOpen ? t("熔断", "Breaker open") : provider.status}</Badge>
      <span>{t("并发", "Concurrency")} {provider.activeConcurrency}{provider.concurrencyLimit !== null ? " / " + provider.concurrencyLimit : ""}</span>
      <span>{t("进行中", "Running")} {provider.runningRequestCount}</span>
    </li>)}</ul>}
    {!!load?.projects.length && <ul className="nt-operations-load-list">{load.projects.map(project => <li key={project.projectId}>
      <span>{project.displayName}</span><span>{t("并发", "Concurrency")} {project.activeConcurrency}</span><span>{t("进行中", "Running")} {project.runningRequestCount}</span>
    </li>)}</ul>}
  </div>;
}
