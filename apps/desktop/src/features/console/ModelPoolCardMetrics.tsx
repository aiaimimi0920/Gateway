import { Activity, CircleDollarSign, Gauge, Send, TrendingUp } from "lucide-react";

import type { ModelPoolCard } from "./modelPoolViewModel";
import {
  buildProviderAvailabilityCells,
  formatAggregateMoney,
  formatAggregateRate,
} from "./providerCardMetrics";

type ModelPoolCardMetricsProps = {
  card: ModelPoolCard;
  t: (zh: string, en: string) => string;
};

/** Front-face usage and availability; the workspace owns all interaction state. */
export function ModelPoolCardMetrics({ card, t }: ModelPoolCardMetricsProps) {
  const metrics = card.metrics;
  const successWindows = metrics?.successWindows ?? [];
  const availabilityCells = buildProviderAvailabilityCells(successWindows);
  const primaryLabel = card.chain[0]?.providerLabel ?? "—";
  return (
    <div className="nt-entitlement-group-card__front-body">
      <dl className="nt-entitlement-group-card__metrics">
        <div>
          <dt>{t("服务商", "Providers")}</dt>
          <dd data-model-pool-metric="providers">{card.chain.length}</dd>
        </div>
        <div>
          <dt>{t("账号数", "Accounts")}</dt>
          <dd data-model-pool-metric="accounts">{card.accountCount}</dd>
        </div>
        <div title={t(`首选服务商 ${primaryLabel}`, `Primary provider ${primaryLabel}`)}>
          <dt>{t("首选", "Primary")}</dt>
          <dd data-model-pool-metric="primary">{primaryLabel}</dd>
        </div>
      </dl>

      {/* Usage rolled up over every account that serves this
          model, in the same order and with the same icons as
          the provider card front on the credential pool page. */}
      <dl className="nt-entitlement-group-card__metrics nt-entitlement-group-card__usage">
        <div title={t(`${card.model} 聚合并发`, `${card.model} aggregated concurrency`)}>
          <dt aria-label={t("并发", "Concurrency")}>
            <Gauge size={15} aria-hidden="true" />
          </dt>
          <dd data-model-pool-metric="concurrency">
            {metrics?.concurrency
              ? `${metrics.concurrency.used}/${metrics.concurrency.total ?? "—"}`
              : "—"}
          </dd>
        </div>
        <div title={t(`${card.model} 聚合上游费用`, `${card.model} aggregated upstream cost`)}>
          <dt aria-label={t("上游费用", "Upstream cost")}>
            <CircleDollarSign size={15} aria-hidden="true" />
          </dt>
          <dd data-model-pool-metric="upstream-cost">
            {metrics?.upstreamCost == null
              ? "—"
              : `$${formatAggregateMoney(metrics.upstreamCost)}`}
          </dd>
        </div>
        <div title={t(`${card.model} 聚合收费`, `${card.model} aggregated revenue`)}>
          <dt aria-label={t("收费", "Revenue")}>
            <TrendingUp size={15} aria-hidden="true" />
          </dt>
          <dd data-model-pool-metric="platform-revenue">
            {metrics?.platformRevenue == null
              ? "—"
              : `$${formatAggregateMoney(metrics.platformRevenue)}`}
          </dd>
        </div>
        <div
          title={t(
            `${card.model} 聚合总请求数${metrics?.requests == null ? "暂无数据" : ` ${metrics.requests}`}`,
            `${card.model} aggregated requests${metrics?.requests == null ? " unavailable" : ` ${metrics.requests}`}`,
          )}
        >
          <dt aria-label={t("请求数", "Requests")}>
            <Send size={15} aria-hidden="true" />
          </dt>
          <dd data-model-pool-metric="requests">
            {metrics?.requests == null
              ? "—"
              : metrics.requests.toLocaleString("en-US")}
          </dd>
        </div>
      </dl>

      {/* Always on, like the metric row above it: a model that
          never dispatched still owns a success rate slot so the
          control does not appear and disappear between cards. */}
      <section
        className="nt-entitlement-group-card__success"
        data-provider-availability-scope="model-pool"
        role="img"
        aria-label={t(
          `${card.model} 最近窗口的调用成功率，链上账号聚合${metrics?.successRate == null ? "，暂无数据" : `，${metrics.successSuccessCount}/${metrics.successRequestCount} 次成功，${formatAggregateRate(metrics.successRate)}`}`,
          `${card.model} recent window success rate, chain accounts aggregated${metrics?.successRate == null ? ", unavailable" : `, ${metrics.successSuccessCount}/${metrics.successRequestCount} successful, ${formatAggregateRate(metrics.successRate)}`}`,
        )}
        title={t(
          `${card.model} 调用成功率 ${formatAggregateRate(metrics?.successRate ?? null)}`,
          `${card.model} success rate ${formatAggregateRate(metrics?.successRate ?? null)}`,
        )}
      >
        <Activity size={15} aria-hidden="true" />
        {successWindows.length > 0 ? (
          <div
            className="nt-provider-card__availability-windows"
            aria-hidden="true"
          >
            {successWindows.map((window, windowIndex) => (
              <div
                className="nt-provider-card__availability-window"
                key={`${card.rowId}:success:${window.label}`}
              >
                {availabilityCells
                  .filter((cell) => cell.windowIndex === windowIndex)
                  .map((cell) => (
                    <span
                      className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
                      key={`${card.rowId}:cell:${cell.windowIndex}:${cell.position}`}
                      title={`${cell.windowLabel}: ${cell.success}/${cell.requests} (${formatAggregateRate(cell.rate)})`}
                    />
                  ))}
              </div>
            ))}
          </div>
        ) : (
          <span
            className="nt-entitlement-group-card__success-empty"
            aria-hidden="true"
          >
            {t("暂无调用数据", "No dispatch data yet")}
          </span>
        )}
        <strong
          className="nt-provider-card__success-rate"
          data-model-pool-metric="success-rate"
        >
          {formatAggregateRate(metrics?.successRate ?? null)}
        </strong>
      </section>
    </div>
  );
}
