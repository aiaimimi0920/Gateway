import { Activity, CircleDollarSign, Gauge, Send, TrendingUp, UsersRound } from "lucide-react";

import { ProviderQuotaPanel } from "./ProviderAccountCard";
import {
  formatAggregateMoney,
  formatAggregateRate,
  type AggregatedProviderQuotaWindow,
  type AggregatedSuccessWindow,
  type ProviderAvailabilityCell,
  type ProviderConcurrency,
  type ProviderCosts,
  type ProviderPoolSegments,
} from "./providerCardMetrics";

type TranslateFn = (zh: string, en: string) => string;

export type ProviderCardFrontBodyProps = {
  providerLabel: string;
  poolSegments: ProviderPoolSegments;
  providerConcurrency: ProviderConcurrency;
  providerCosts: ProviderCosts;
  providerRequestCount: number | null;
  providerSuccessWindows: AggregatedSuccessWindow[];
  providerSuccessTotals: { success: number; requests: number };
  providerSuccessRate: number | null;
  providerAvailabilityCells: ProviderAvailabilityCell[];
  providerQuotaWindows: AggregatedProviderQuotaWindow[];
  providerQuotaRemainingUsd: number | null;
  t: TranslateFn;
};

function providerPoolSegmentLabel(
  state: "available" | "rate-limited" | "invalid" | "remaining",
  count: number,
  t: TranslateFn,
): string {
  switch (state) {
    case "available":
      return t(`可用 ${count}`, `Available ${count}`);
    case "rate-limited":
      return t(`待恢复 ${count}`, `Recovering ${count}`);
    case "invalid":
      return t(`失效 ${count}`, `Invalid ${count}`);
    case "remaining":
      return t(`剩余 ${count}`, `Remaining ${count}`);
  }
}

export function ProviderCardFrontBody({
  providerLabel,
  poolSegments,
  providerConcurrency,
  providerCosts,
  providerRequestCount,
  providerSuccessWindows,
  providerSuccessTotals,
  providerSuccessRate,
  providerAvailabilityCells,
  providerQuotaWindows,
  providerQuotaRemainingUsd,
  t,
}: ProviderCardFrontBodyProps) {
  return (
    <div className="nt-provider-card__front-body">
      <section
        className="nt-provider-card__pool"
        title={t(
          `${providerLabel}：${poolSegments.available} 个可用，${poolSegments.rateLimited} 个待恢复，${poolSegments.invalid} 个失效，${poolSegments.remaining} 个剩余${poolSegments.unknown > 0 ? `，${poolSegments.unknown} 个状态未知` : ""}`,
          `${providerLabel}: ${poolSegments.available} available, ${poolSegments.rateLimited} recovering, ${poolSegments.invalid} invalid, ${poolSegments.remaining} remaining${poolSegments.unknown > 0 ? `, ${poolSegments.unknown} unknown` : ""}`,
        )}
        aria-label={t(`${providerLabel} 目标账号池`, `${providerLabel} target account pool`)}
      >
        <div className="nt-provider-card__pool-value">
          <UsersRound size={15} aria-hidden="true" />
          <strong>{`${poolSegments.available}/${poolSegments.target}`}</strong>
        </div>
        <div
          className="nt-provider-card__pool-bar"
          role="img"
          aria-label={t(
            `可用 ${poolSegments.available}，待恢复 ${poolSegments.rateLimited}，失效 ${poolSegments.invalid}，剩余 ${poolSegments.remaining}`,
            `Available ${poolSegments.available}, recovering ${poolSegments.rateLimited}, invalid ${poolSegments.invalid}, remaining ${poolSegments.remaining}`,
          )}
        >
          {poolSegments.segments.map((segment) => (
            <span
              className={`nt-provider-card__pool-segment nt-provider-card__pool-segment--${segment.state}`}
              data-pool-segment={segment.state}
              key={segment.state}
              style={{ width: `${segment.percent}%` }}
              title={providerPoolSegmentLabel(segment.state, segment.count, t)}
            />
          ))}
        </div>
      </section>

      <dl className="nt-provider-card__metrics">
        <div title={t(`${providerLabel} 并发`, `${providerLabel} concurrency`)}>
          <dt aria-label={t("并发", "Concurrency")}>
            <Gauge size={15} aria-hidden="true" />
          </dt>
          <dd data-provider-metric="concurrency">
            {providerConcurrency ? `${providerConcurrency.used}/${providerConcurrency.total ?? "—"}` : "—"}
          </dd>
        </div>
        <div title={t(`${providerLabel} 上游费用`, `${providerLabel} upstream cost`)}>
          <dt aria-label={t("上游费用", "Upstream cost")}>
            <CircleDollarSign size={15} aria-hidden="true" />
          </dt>
          <dd data-provider-metric="upstream-cost">
            {providerCosts.upstream === null ? "—" : `$${formatAggregateMoney(providerCosts.upstream)}`}
          </dd>
        </div>
        <div title={t(`${providerLabel} 平台收入`, `${providerLabel} platform revenue`)}>
          <dt aria-label={t("平台收入", "Platform revenue")}>
            <TrendingUp size={15} aria-hidden="true" />
          </dt>
          <dd data-provider-metric="platform-revenue">
            {providerCosts.user === null ? "—" : `$${formatAggregateMoney(providerCosts.user)}`}
          </dd>
        </div>
        <div
          title={t(
            `${providerLabel} 总请求数${providerRequestCount === null ? "暂无数据" : ` ${providerRequestCount}`}`,
            `${providerLabel} total requests${providerRequestCount === null ? " unavailable" : ` ${providerRequestCount}`}`,
          )}
        >
          <dt aria-label={t("请求数", "Requests")}>
            <Send size={15} aria-hidden="true" />
          </dt>
          <dd data-provider-metric="requests">
            {providerRequestCount === null ? "—" : providerRequestCount.toLocaleString("en-US")}
          </dd>
        </div>
      </dl>

      {providerSuccessWindows.length > 0 ? (
        <section
          className="nt-provider-card__success"
          data-provider-availability-scope="provider"
          role="img"
          aria-label={t(
            `${providerLabel} 最近窗口的调用成功率，所有模型聚合${providerSuccessRate === null ? "，暂无数据" : `，${providerSuccessTotals.success}/${providerSuccessTotals.requests} 次成功，${formatAggregateRate(providerSuccessRate)}`}`,
            `${providerLabel} recent window success rate, all models aggregated${providerSuccessRate === null ? ", unavailable" : `, ${providerSuccessTotals.success}/${providerSuccessTotals.requests} successful, ${formatAggregateRate(providerSuccessRate)}`}`,
          )}
          title={t(
            `${providerLabel} 最近窗口的调用成功率（所有模型聚合，每 5 分钟）`,
            `${providerLabel} recent window success rate (all models aggregated, 5 min windows)`,
          )}
        >
          <Activity size={15} aria-hidden="true" />
          <div className="nt-provider-card__availability-windows" aria-hidden="true">
            {providerSuccessWindows.map((window, windowIndex) => {
              const windowTitle = t(
                `${window.label}：${window.success}/${window.requests} 次成功（${Math.round((window.rate ?? 0) * 100)}%）`,
                `${window.label}: ${window.success}/${window.requests} successful (${Math.round((window.rate ?? 0) * 100)}%)`,
              );
              return (
                <div
                  className="nt-provider-card__availability-window"
                  data-provider-availability-window={window.label}
                  key={window.label}
                  title={windowTitle}
                >
                  {providerAvailabilityCells
                    .filter((cell) => cell.windowIndex === windowIndex)
                    .map((cell) => (
                      <span
                        className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
                        data-provider-availability-cell={cell.state}
                        key={`${cell.windowIndex}:${cell.position}`}
                        title={windowTitle}
                      />
                    ))}
                </div>
              );
            })}
          </div>
          <strong className="nt-provider-card__success-rate" data-provider-metric="success-rate">
            {formatAggregateRate(providerSuccessRate)}
          </strong>
        </section>
      ) : null}

      <ProviderQuotaPanel
        t={t}
        scope="provider"
        windows={providerQuotaWindows}
        remainingUsd={providerQuotaRemainingUsd}
      />
    </div>
  );
}
