import type { UiLocale } from "../../i18n/UiLocaleProvider";
import type { AccountsLedgerPilotAccount } from "./accountCardTypes";
import type { ConsoleTelemetrySnapshot } from "./telemetry";
import { credentialStatusLabel, formatRelativeSince } from "./telemetryPresentation";
import { formatAggregateRate } from "./providerCardMetrics";

type PilotStatsMetric = {
  key: string;
  label: string;
  value: string;
  hint?: string | null;
};

type PilotStatsView = {
  headline: PilotStatsMetric[];
  credentialRows: PilotStatsMetric[];
  providerRows: PilotStatsMetric[];
  /** Route-document credentials that share this provider account's numbers. */
  sharedCredentialCount: number;
  hasCredentialHealth: boolean;
  hasProviderTelemetry: boolean;
};

function formatStatsCount(value: number | null | undefined): string {
  return typeof value === "number" && Number.isFinite(value) ? value.toLocaleString("en-US") : "—";
}

function formatStatsMoney(value: number | null | undefined): string {
  return typeof value === "number" && Number.isFinite(value) ? `$${value.toFixed(2)}` : "—";
}

function formatStatsTimestamp(timestamp: string | null | undefined, locale: UiLocale): string {
  if (!timestamp) {
    return "—";
  }
  const parsed = Date.parse(timestamp);
  return Number.isFinite(parsed) ? new Date(parsed).toLocaleString(locale) : timestamp;
}

/**
 * The stats dialog reads the same live endpoints the cards poll, split by the
 * granularity the gateway actually records: health, cooldown and quota exist per
 * credential, while requests, tokens, money and live concurrency exist only per
 * provider account, so they are labelled as shared by the whole pool instead of
 * being presented as this one credential's traffic.
 */
export function buildPilotStatsView(
  account: AccountsLedgerPilotAccount,
  providerAccountId: string,
  snapshot: ConsoleTelemetrySnapshot,
  locale: UiLocale,
): PilotStatsView {
  const label = (zh: string, en: string) => (locale === "zh-CN" ? zh : en);
  const credential = snapshot.credentials.get(account.accountId) ?? null;
  const provider = snapshot.providerAccounts.get(providerAccountId) ?? null;
  const completed = provider?.completedCount ?? null;
  const failed = provider?.failedCount ?? null;
  // Cancelled and still-running requests are excluded so the rate only divides
  // by requests the upstream actually decided.
  const decided = completed === null && failed === null ? null : (completed ?? 0) + (failed ?? 0);
  const successRate = decided !== null && decided > 0 ? (completed ?? 0) / decided : null;
  const lastFailureHint =
    credential?.lastError ??
    (typeof credential?.lastUpstreamStatus === "number"
      ? `HTTP ${credential.lastUpstreamStatus}`
      : null);

  return {
    headline: [
      {
        key: "requests",
        label: label("总请求", "Requests"),
        value: formatStatsCount(provider?.requestCount),
        hint: label("计费聚合口径", "Billing aggregate scope"),
      },
      {
        key: "completed",
        label: label("已完成", "Completed"),
        value: formatStatsCount(completed),
        hint: formatAggregateRate(successRate),
      },
      {
        key: "failed",
        label: label("失败", "Failed"),
        value: formatStatsCount(failed),
        hint: label("最近审计窗口", "Recent audit window"),
      },
      {
        key: "tokens",
        label: "Token",
        value: formatStatsCount(provider?.totalTokens),
        hint: provider
          ? `${formatStatsCount(provider.promptTokens)} / ${formatStatsCount(provider.completionTokens)}`
          : null,
      },
    ],
    credentialRows: [
      {
        key: "health",
        label: label("健康状态", "Health"),
        value:
          credentialStatusLabel(credential?.status ?? null, locale) ??
          label("待观测", "not observed"),
        hint: credential ? null : label("尚无调度记录", "never dispatched"),
      },
      {
        key: "models",
        label: label("已服务模型", "Served models"),
        value: String(credential?.models.length ?? 0),
        hint: credential?.models.join(", ") || null,
      },
      {
        key: "failures",
        label: label("累计失败", "Failure count"),
        value: formatStatsCount(credential?.failureCount),
      },
      {
        key: "cooldown",
        label: label("冷却至", "Cooling until"),
        value: formatStatsTimestamp(credential?.cooldownUntil, locale),
      },
      {
        key: "last-success",
        label: label("最近成功", "Last success"),
        value: formatRelativeSince(credential?.lastSuccessAt ?? null, locale) ?? "—",
        hint: credential?.lastSuccessAt
          ? formatStatsTimestamp(credential.lastSuccessAt, locale)
          : null,
      },
      {
        key: "last-failure",
        label: label("最近失败", "Last failure"),
        value: formatRelativeSince(credential?.lastFailureAt ?? null, locale) ?? "—",
        hint: lastFailureHint,
      },
      {
        key: "quota",
        label: label("可用余额", "Remaining balance"),
        value: formatStatsMoney(account.quotaRemainingUsd ?? credential?.quotaRemainingUsd),
      },
    ],
    providerRows: [
      {
        key: "concurrency",
        label: label("实时并发", "Live concurrency"),
        value: provider
          ? `${provider.activeConcurrency ?? 0} / ${provider.concurrencyLimit ?? "—"}`
          : "—",
        hint:
          typeof provider?.concurrencyAvailable === "number"
            ? label(`剩余 ${provider.concurrencyAvailable}`, `${provider.concurrencyAvailable} free`)
            : null,
      },
      {
        key: "breaker",
        label: label("熔断器", "Breaker"),
        value: provider
          ? provider.breakerOpen
            ? label("已打开", "open")
            : label("闭合", "closed")
          : "—",
        hint: provider?.runtimeStatus ?? null,
      },
      {
        key: "upstream-cost",
        label: label("上游费用", "Upstream cost"),
        value: formatStatsMoney(provider?.upstreamCostUsd),
      },
      {
        key: "last-request",
        label: label("最近调用", "Last request"),
        value: formatRelativeSince(provider?.lastRequestAt ?? null, locale) ?? "—",
        hint: provider?.lastRequestAt
          ? formatStatsTimestamp(provider.lastRequestAt, locale)
          : null,
      },
      {
        key: "traffic-models",
        label: label("有流量模型", "Models with traffic"),
        value: String(provider?.models.length ?? 0),
        hint: provider?.models.join(", ") || null,
      },
    ],
    sharedCredentialCount:
      snapshot.credentialRefsByProviderAccountId.get(providerAccountId)?.length ?? 0,
    hasCredentialHealth: credential !== null,
    hasProviderTelemetry: provider !== null,
  };
}
