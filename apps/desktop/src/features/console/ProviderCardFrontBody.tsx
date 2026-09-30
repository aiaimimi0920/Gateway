import { Activity, CircleDollarSign, Gauge, Send, TrendingUp, UsersRound } from "lucide-react";

import { ProviderQuotaPanel } from "./ProviderAccountCard";
import { CardModelList } from "./CardModelList";
import type { CardModelTraffic } from "./cardModelTraffic";
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
  supportedModels?: readonly string[];
  modelTraffic?: CardModelTraffic;
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
  state: "available" | "rate-limited" | "invalid" | "unknown" | "remaining",
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
    case "unknown":
      return t(`待观测 ${count}`, `Not observed ${count}`);
  }
}

export function ProviderCardFrontBody({
  providerLabel,
  supportedModels,
  modelTraffic,
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
        <div
          className="nt-provider-card__pool-bar"
          role="img"
          aria-label={t(
            `可用 ${poolSegments.available}，待恢复 ${poolSegments.rateLimited}，失效 ${poolSegments.invalid}，待观测 ${poolSegments.unknown}，剩余 ${poolSegments.remaining}`,
            `Available ${poolSegments.available}, recovering ${poolSegments.rateLimited}, invalid ${poolSegments.invalid}, not observed ${poolSegments.unknown}, remaining ${poolSegments.remaining}`,
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
        <div className="nt-provider-card__pool-value" aria-label={t("已配置账号 / 目标账号", "Configured accounts / target accounts")}>
          <UsersRound size={15} aria-hidden="true" />
          <strong>{`${poolSegments.available + poolSegments.rateLimited + poolSegments.invalid + poolSegments.unknown}/${poolSegments.target}`}</strong>
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
        <div title={providerCosts.upstream === null
          ? t(`${providerLabel} 上游费用：暂无完整的 USD 用量计价数据`, `${providerLabel} upstream cost: complete USD usage pricing unavailable`)
          : t(`${providerLabel} 上游费用估算：实际 token 用量 × 配置价格，非上游账单`, `${providerLabel} estimated upstream cost: recorded token usage × configured prices, not an upstream invoice`)}>
          <dt aria-label={t("上游费用", "Upstream cost")}>
            <CircleDollarSign size={15} aria-hidden="true" />
          </dt>
          <dd data-provider-metric="upstream-cost">
            {providerCosts.upstream === null ? "—" : `≈$${formatAggregateMoney(providerCosts.upstream)}`}
          </dd>
        </div>
        <div title={providerCosts.user === null
          ? t(`${providerLabel} 平台收入：暂无已结算收入数据`, `${providerLabel} platform revenue: settled revenue unavailable`)
          : t(`${providerLabel} 平台收入`, `${providerLabel} platform revenue`)}>
          <dt aria-label={t("平台收入", "Platform revenue")}>
            <TrendingUp size={15} aria-hidden="true" />
          </dt>
          <dd data-provider-metric="platform-revenue">
            {providerCosts.user === null ? "—" : `$${formatAggregateMoney(providerCosts.user)}`}
          </dd>
        </div>
        <div
          title={t(
            `${providerLabel} 已保留调用记录${providerRequestCount === null ? "暂无数据" : ` ${providerRequestCount} 次（含成功、失败和运行中请求）`}`,
            `${providerLabel} retained request history${providerRequestCount === null ? " unavailable" : `: ${providerRequestCount} calls, including completed, failed and running requests`}`,
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
            `${providerLabel} 最近窗口的调用成功率（所有模型聚合，按小时）`,
            `${providerLabel} recent window success rate (all models aggregated, hourly windows)`,
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
      ) : (
        <section className="nt-provider-card__success" data-provider-call-state="empty">
          <Activity size={15} aria-hidden="true" />
          <span>{providerRequestCount === null
            ? t("调用统计暂不可用", "Call statistics unavailable")
            : t("当前统计窗口暂无调用记录", "No calls in the current statistics window")}</span>
        </section>
      )}

      {supportedModels && <CardModelList models={supportedModels} label={providerLabel} traffic={modelTraffic} t={t} />}

      <ProviderQuotaPanel
        t={t}
        scope="provider"
        windows={providerQuotaWindows}
        remainingUsd={providerQuotaRemainingUsd}
      />
    </div>
  );
}
