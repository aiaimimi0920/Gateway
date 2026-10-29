import type { TranslateFn } from "./accountCardTypes";
import { formatAvailabilityRange } from "./availabilityWindows";
import { formatAggregateRate, type ProviderAvailabilityCell } from "./providerCardMetrics";

/** Each focusable block is one real half-hour bucket, shared by cards and model rows. */
export function AvailabilityHeatmap({ cells, label, t, scope, compact = false }: {
  cells: readonly ProviderAvailabilityCell[];
  label: string;
  t: TranslateFn;
  scope?: "provider" | "account";
  compact?: boolean;
}) {
  return (
    <span className={compact ? "nt-card-models__cells" : "nt-provider-card__availability-windows"}
      role="group" aria-label={label}>
      {cells.map((cell) => {
        const range = formatAvailabilityRange(cell.windowLabel);
        const calls = cell.requests.toLocaleString("en-US");
        const rate = formatAggregateRate(cell.rate);
        const hint = t(`时间：${range}；调用 ${calls} 次；成功率 ${rate}`,
          `Time: ${range}; Calls: ${calls}; Success: ${rate}`);
        return <span key={cell.windowLabel} role="img" tabIndex={0} title={hint} aria-label={hint}
          className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
          data-provider-availability-window={scope === "provider" ? cell.windowLabel : undefined}
          data-provider-availability-cell={scope === "provider" ? cell.state : undefined}
          data-account-availability-window={scope === "account" ? cell.windowLabel : undefined}
          data-account-availability-cell={scope === "account" ? cell.state : undefined} />;
      })}
      {!cells.length && <span role="img" aria-label={t("暂无调用数据", "No call data")}
        className="nt-provider-card__availability-cell nt-provider-card__availability-cell--empty" />}
    </span>
  );
}
