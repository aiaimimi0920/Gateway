import type { ConsoleProviderQuotaWindow } from "../../api/contracts";
import { formatAggregateMoney } from "./providerCardMetrics";
import type { QuotaDisplayWindow, TranslateFn } from "./accountCardTypes";

export function quotaWindowRemainingRatio(window: ConsoleProviderQuotaWindow): number | null {
  const value =
    typeof window.remainingRatio === "number"
      ? window.remainingRatio
      : typeof window.usedPercent === "number"
        ? 1 - window.usedPercent / 100
        : null;
  return value === null || !Number.isFinite(value) ? null : Math.min(1, Math.max(0, value));
}

export function formatQuotaReset(resetAt: string | null, t: TranslateFn): string {
  if (!resetAt) {
    return " ";
  }
  const timestamp = Date.parse(resetAt);
  if (!Number.isFinite(timestamp)) {
    return " ";
  }
  return t(
    `${new Intl.DateTimeFormat("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }).format(timestamp)} 重置`,
    `Resets ${new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(timestamp)}`,
  );
}

/** Renders only quota data explicitly reported by the upstream. */
export function ProviderQuotaPanel(props: {
  t: TranslateFn;
  scope: "provider" | "account";
  windows: readonly QuotaDisplayWindow[];
  remainingUsd: number | null;
}) {
  const { t, scope, windows, remainingUsd } = props;
  const visibleWindows = windows.slice(0, 2);
  if (visibleWindows.length === 0 && remainingUsd === null) {
    return null;
  }
  return (
    <section
      className={`nt-provider-quota nt-provider-quota--${scope}`}
      data-quota-scope={scope}
      aria-label={t("限时额度", "Time-limited quota")}
    >
      <header className="nt-provider-quota__head">
        <span>{t("限时额度", "Quota windows")}</span>
        <strong
          title={t(
            "仅在上游明确返回美元余额时显示，不使用不可靠的估算。",
            "Shown only when the upstream explicitly returns a USD balance; no unreliable estimate is fabricated.",
          )}
        >
          {remainingUsd === null ? "≈$—" : `≈$${formatAggregateMoney(remainingUsd)}`}
        </strong>
      </header>
      <div className="nt-provider-quota__windows">
        {visibleWindows.map((window) => {
          const percent =
            window.remainingRatio === null ? null : Math.round(window.remainingRatio * 100);
          return (
            <div
              className="nt-provider-quota__window"
              data-quota-window={window.key}
              key={window.key}
            >
              <div className="nt-provider-quota__window-head">
                <span title={window.label}>{window.label}</span>
                <strong>{percent === null ? "—" : `${percent}%`}</strong>
              </div>
              <div className="nt-provider-quota__bar" aria-hidden="true">
                <span style={{ width: `${percent ?? 0}%` }} />
              </div>
              <small>
                {`${formatQuotaReset(window.resetAt, t)}${window.accountCount ? t(` · ${window.accountCount} 个账号`, ` · ${window.accountCount} accounts`) : ""}`}
              </small>
            </div>
          );
        })}
      </div>
    </section>
  );
}
