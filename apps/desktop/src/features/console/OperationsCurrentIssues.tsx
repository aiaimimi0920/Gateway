import type { OperationsWorkspaceProps } from "./operations-contracts";
import {
  formatCount,
  formatMs,
  formatText,
  formatTimestamp,
  Panel,
} from "./OperationsPrimitives";

type Props = Pick<
  OperationsWorkspaceProps,
  "t" | "pressure" | "readiness" | "requests"
> & {
  onOpenRequests: () => void;
};

export function OperationsCurrentIssues({
  t,
  pressure,
  readiness,
  requests,
  onOpenRequests,
}: Props) {
  const issues: { id: string; title: string; detail: string }[] = [];
  const ready = readiness.data;
  if (ready) {
    const dependencies = [
      ["SQLite", ready.dependencies.sqlite],
      ["PostgreSQL", ready.dependencies.postgresql],
      ["Redis", ready.dependencies.redis],
      [t("对象存储", "Object storage"), ready.dependencies.objectStorage],
    ] as const;
    for (const [name, dependency] of dependencies) {
      if (
        dependency &&
        !dependency.ready &&
        (dependency.configured || dependency.required)
      ) {
        issues.push({
          id: name,
          title: `${name} ${t("不可用", "unavailable")}`,
          detail: dependency.timedOut
            ? t("连接检查超时", "Connection check timed out")
            : t(
                "检查服务连接与存储配置",
                "Check the service connection and storage configuration",
              ),
        });
      }
    }
    if (ready.draining)
      issues.push({
        id: "draining",
        title: t("网关正在排空", "Gateway draining"),
        detail:
          ready.drainReason ||
          t(
            `剩余 ${formatCount(ready.activeRequests)} 个请求`,
            `${formatCount(ready.activeRequests)} requests remaining`,
          ),
      });
    if (!ready.ok && issues.length === 0)
      issues.push({
        id: "not-ready",
        title: t("网关未就绪", "Gateway not ready"),
        detail: t(
          "查看运行状态中的依赖与配置检查",
          "Check dependencies and configuration under Live",
        ),
      });
  }
  for (const provider of pressure.data?.providers ?? []) {
    if (provider.breakerOpen)
      issues.push({
        id: `provider-${provider.providerAccountId}`,
        title: `${formatText(provider.label)} · ${t("熔断", "Breaker open")}`,
        detail: t(
          "检查凭据状态、上游错误和并发限制",
          "Check credential status, upstream errors and concurrency limits",
        ),
      });
  }
  const complete = Boolean(
    ready &&
    pressure.data &&
    !readiness.error &&
    !pressure.error &&
    !readiness.loading &&
    !pressure.loading,
  );

  return (
    <div className="nt-operations-issues">
      <article className="nt-pilot-metric-card">
        <h3>{t("当前运行异常", "Current runtime issues")}</h3>
        {[readiness.error, pressure.error]
          .filter(Boolean)
          .map((error, index) => (
            <div
              key={index}
              className="nt-alert nt-alert--warning"
              role="status"
            >
              {error}
            </div>
          ))}
        {issues.length ? (
          <ul className="nt-operations-issue-list">
            {issues.map((issue) => (
              <li key={issue.id}>
                <strong>{issue.title}</strong>
                <span>{issue.detail}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="nt-copy" role="status">
            {complete
              ? t("未发现运行异常", "No runtime issues detected")
              : readiness.loading || pressure.loading
                ? t("正在检查运行状态…", "Checking runtime status…")
                : t(
                    "运行状态不完整，暂时无法判断",
                    "Runtime status is incomplete",
                  )}
          </p>
        )}
      </article>
      <Panel
        t={t}
        state={requests}
        title={t(
          "近期失败请求 · 当前筛选",
          "Recent failed requests · current filters",
        )}
      >
        {(rows) => {
          const failures = rows.filter(
            (row) => row.status.toLowerCase() === "failed",
          );
          return (
            <>
              <div className="nt-operations-issues__heading">
                <span className="nt-copy">
                  {t(
                    `本次查询 ${rows.length} 条，失败 ${failures.length} 条`,
                    `${failures.length} failed out of ${rows.length} queried`,
                  )}
                </span>
                <button
                  type="button"
                  className="nt-btn nt-btn--outline"
                  onClick={onOpenRequests}
                >
                  {t("查看请求", "View requests")}
                </button>
              </div>
              {failures.length ? (
                <ul className="nt-operations-failure-list">
                  {failures.slice(0, 10).map((row) => (
                    <li key={row.id}>
                      <div>
                        <strong>
                          {formatText(row.resolvedModel ?? row.requestedModel)}
                        </strong>
                        <span>
                          {formatTimestamp(row.createdAt)} ·{" "}
                          {formatMs(row.durationMs)}
                        </span>
                      </div>
                      <p>
                        {formatText(
                          row.errorSummary ??
                            (row.upstreamStatus === null
                              ? null
                              : `HTTP ${row.upstreamStatus}`),
                        )}
                      </p>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="nt-copy">
                  {t(
                    "本次查询没有失败请求",
                    "No failed requests in this query",
                  )}
                </p>
              )}
              {failures.length > 10 ? (
                <span className="nt-copy">
                  {t(
                    "显示最近 10 条，完整记录见请求明细",
                    "Showing the latest 10; see Request log for all results",
                  )}
                </span>
              ) : null}
            </>
          );
        }}
      </Panel>
    </div>
  );
}
