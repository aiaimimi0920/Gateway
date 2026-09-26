import {
  Activity,
  CircleDollarSign,
  Gauge,
  Send,
  TrendingUp,
  UsersRound,
} from "lucide-react";

import type { ProviderAggregateMetrics } from "./providerCardMetrics";
import {
  buildProviderAvailabilityCells,
  formatAggregateMoney,
  formatAggregateRate,
} from "./providerCardMetrics";
import { ScopePager } from "./EntitlementScopePager";

type TranslateFn = (zh: string, en: string) => string;

export type EntitlementModelScopeRow = {
  model: string;
  accountsTotal: number;
  accountsEnabled: number;
  providerCount: number;
  metrics: ProviderAggregateMetrics;
};

export type EntitlementGroupModelScopeProps = {
  t: TranslateFn;
  cardKey: string;
  label: string;
  selectedProviderIds: ReadonlySet<string>;
  visibleModels: readonly EntitlementModelScopeRow[];
  modelPage: number;
  modelTotalPages: number;
  modelPageRows: readonly EntitlementModelScopeRow[];
  onPage: (page: number) => void;
};

/** Render the model scope band from the board's already selected and paged rows. */
export function EntitlementGroupModelScope({
  t,
  cardKey,
  label,
  selectedProviderIds,
  visibleModels,
  modelPage,
  modelTotalPages,
  modelPageRows,
  onPage,
}: EntitlementGroupModelScopeProps) {
  return (
    <section
      className="nt-entitlement-scope__band nt-entitlement-scope__band--models"
      aria-label={t(`${label} 模型标签`, `${label} model scope`)}
    >
      <header className="nt-entitlement-scope__band-head">
        <strong>{t("模型标签", "Model scope")}</strong>
        <span className="nt-entitlement-scope__count">{visibleModels.length}</span>
      </header>
      {modelPageRows.length > 0 ? (
        <>
          <ul className="nt-entitlement-scope__model-list">
            {modelPageRows.map((row) => {
              const successWindows = row.metrics.successWindows;
              const availabilityCells = buildProviderAvailabilityCells(successWindows);
              const successLabel = formatAggregateRate(row.metrics.successRate);
              return (
                <li
                  className="nt-entitlement-scope__model-row"
                  key={`${cardKey}:model:${row.model}`}
                  data-entitlement-scope-model={row.model}
                >
                  <div className="nt-entitlement-scope__model-head">
                    <strong title={row.model}>{row.model}</strong>
                    {row.providerCount > 1 ? (
                      <span className="nt-entitlement-scope__model-providers">
                        {t(`${row.providerCount} 家服务商`, `${row.providerCount} providers`)}
                      </span>
                    ) : null}
                    {/* Concurrency rides the title line, right aligned, so the
                        metric row below keeps the provider card's four slots. */}
                    <span
                      className="nt-entitlement-scope__model-concurrency"
                      title={t("并发数", "Concurrency")}
                    >
                      <Gauge size={13} aria-hidden="true" />
                      <span
                        aria-label={t("并发数", "Concurrency")}
                        data-entitlement-model-metric="concurrency"
                      >
                        {row.metrics.concurrency
                          ? `${row.metrics.concurrency.used}/${row.metrics.concurrency.total ?? "—"}`
                          : "—"}
                      </span>
                    </span>
                  </div>
                  <dl className="nt-entitlement-scope__model-metrics">
                    <div
                      title={t(
                        `可用账户数 ${row.accountsEnabled}/${row.accountsTotal}`,
                        `Available accounts ${row.accountsEnabled}/${row.accountsTotal}`,
                      )}
                    >
                      <dt aria-label={t("可用账户数", "Available accounts")}>
                        <UsersRound size={13} aria-hidden="true" />
                      </dt>
                      <dd data-entitlement-model-metric="accounts">
                        {row.accountsEnabled}/{row.accountsTotal}
                      </dd>
                    </div>
                    <div title={t("费用", "Cost")}>
                      <dt aria-label={t("费用", "Cost")}>
                        <CircleDollarSign size={13} aria-hidden="true" />
                      </dt>
                      <dd data-entitlement-model-metric="upstream-cost">
                        {row.metrics.upstreamCost == null
                          ? "—"
                          : `$${formatAggregateMoney(row.metrics.upstreamCost)}`}
                      </dd>
                    </div>
                    <div title={t("收入", "Revenue")}>
                      <dt aria-label={t("收入", "Revenue")}>
                        <TrendingUp size={13} aria-hidden="true" />
                      </dt>
                      <dd data-entitlement-model-metric="platform-revenue">
                        {row.metrics.platformRevenue == null
                          ? "—"
                          : `$${formatAggregateMoney(row.metrics.platformRevenue)}`}
                      </dd>
                    </div>
                    <div title={t("请求数", "Requests")}>
                      <dt aria-label={t("请求数", "Requests")}>
                        <Send size={13} aria-hidden="true" />
                      </dt>
                      <dd data-entitlement-model-metric="requests">
                        {row.metrics.requests == null
                          ? "—"
                          : row.metrics.requests.toLocaleString("en-US")}
                      </dd>
                    </div>
                  </dl>
                  {/* The real window strip, same shape as the provider cards, so
                      the rate reads as a trend and not just a number. */}
                  <section
                    className="nt-entitlement-scope__model-success"
                    data-provider-availability-scope="entitlement-model"
                    role="img"
                    aria-label={t(
                      `${row.model} 调用成功率${row.metrics.successRate == null ? "，暂无数据" : `，${row.metrics.successSuccessCount}/${row.metrics.successRequestCount} 次成功，${successLabel}`}`,
                      `${row.model} success rate${row.metrics.successRate == null ? ", unavailable" : `, ${row.metrics.successSuccessCount}/${row.metrics.successRequestCount} successful, ${successLabel}`}`,
                    )}
                    title={t(
                      `${row.model} 调用成功率 ${successLabel}`,
                      `${row.model} success rate ${successLabel}`,
                    )}
                  >
                    <Activity size={13} aria-hidden="true" />
                    {successWindows.length > 0 ? (
                      <div className="nt-provider-card__availability-windows" aria-hidden="true">
                        {successWindows.map((window, windowIndex) => (
                          <div
                            className="nt-provider-card__availability-window"
                            key={`${cardKey}:${row.model}:window:${window.label}`}
                          >
                            {availabilityCells
                              .filter((cell) => cell.windowIndex === windowIndex)
                              .map((cell) => (
                                <span
                                  className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
                                  key={`${cardKey}:${row.model}:cell:${cell.windowIndex}:${cell.position}`}
                                  title={`${cell.windowLabel}: ${cell.success}/${cell.requests} (${formatAggregateRate(cell.rate)})`}
                                />
                              ))}
                          </div>
                        ))}
                      </div>
                    ) : (
                      <span
                        className="nt-entitlement-scope__model-success-empty"
                        aria-hidden="true"
                      >
                        {t("暂无调用数据", "No dispatch data yet")}
                      </span>
                    )}
                    <strong
                      className="nt-provider-card__success-rate"
                      data-entitlement-model-metric="success-rate"
                    >
                      {successLabel}
                    </strong>
                  </section>
                </li>
              );
            })}
          </ul>
          <ScopePager
            t={t}
            page={modelPage}
            totalPages={modelTotalPages}
            navLabel={t("模型标签分页", "Model scope pagination")}
            onPage={onPage}
          />
        </>
      ) : (
        <p className="nt-entitlement-scope__empty">
          {selectedProviderIds.size === 0
            ? t("选择服务商标签以查看模型", "Select a provider tab to list models")
            : t("所选服务商暂无模型", "Selected providers expose no models")}
        </p>
      )}
    </section>
  );
}
