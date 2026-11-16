import { AvailabilityHeatmap } from "./AvailabilityHeatmap";
import { ProviderQuotaPanel } from "./accountCardQuota";
import { formatAggregateMoney, formatAggregateRate } from "./providerCardMetrics";
import type { ProviderCardSnapshot } from "./providerCardSnapshot";
import type { TranslateFn } from "./accountsLedgerTypes";

/** Provider snapshots retain authoritative totals; account snapshots never inherit them. */
export function CredentialPoolOverview({ snapshot: s, account = false, name, t }: {
  snapshot: ProviderCardSnapshot; account?: boolean; name: string; t: TranslateFn;
}) {
  const metrics = [
    [t("并发", "Concurrency"), s.concurrency ? `${s.concurrency.used}/${s.concurrency.total ?? "—"}` : "—"],
    [t("请求数", "Requests"), s.requestCount === null ? "—" : s.requestCount.toLocaleString()],
    [t("上游费用估算", "Estimated upstream cost"), s.costs.upstream === null ? "—" : `≈$${formatAggregateMoney(s.costs.upstream)}`],
    [t("平台收入", "Platform revenue"), s.costs.user === null ? "—" : `$${formatAggregateMoney(s.costs.user)}`],
  ];
  const labels = { available: t("可用", "Available"), "rate-limited": t("待恢复", "Recovering"), invalid: t("失效", "Invalid"), unknown: t("待观测", "Not observed"), remaining: t("剩余容量", "Remaining capacity") };
  return <section className="nt-credential-overview" aria-label={`${name} ${t("统计", "statistics")}`}>
    {!account && <div className="nt-credential-overview__pool"><span>{t("账号池", "Account pool")}</span><strong>{s.accounts.length} / {s.poolSegments.target}</strong>
      <div className="nt-provider-card__pool-bar" aria-label={t("账号池状态", "Account pool state")} role="img">
        {s.poolSegments.segments.map(segment => <span key={segment.state} className={`nt-provider-card__pool-segment nt-provider-card__pool-segment--${segment.state}`} style={{ width: `${segment.percent}%` }} title={`${labels[segment.state]} ${segment.count}`} />)}
      </div><span>{t("可用", "Available")} {s.availablePoolCount} · {t("待恢复", "Recovering")} {s.poolSegments.rateLimited} · {t("失效", "Invalid")} {s.poolSegments.invalid} · {t("待观测", "Not observed")} {s.poolSegments.unknown}</span>
    </div>}
    <dl className="nt-credential-overview__metrics">{metrics.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
    <div className="nt-credential-overview__activity"><span>{t("近期调用", "Recent calls")}</span>
      {s.successWindows.length ? <AvailabilityHeatmap cells={s.availabilityCells} scope={account ? "account" : "provider"} label={`${name} ${t("热度图", "heatmap")}`} t={t} /> : <span className="n-muted">{t("当前统计窗口暂无调用记录", "No calls in the current window")}</span>}
      <span>{t("成功率", "Success rate")} {formatAggregateRate(s.successRate)}</span>
    </div>
    <ProviderQuotaPanel t={t} scope={account ? "account" : "provider"} windows={s.quotaWindows} remainingUsd={s.quotaRemainingUsd} />
  </section>;
}
