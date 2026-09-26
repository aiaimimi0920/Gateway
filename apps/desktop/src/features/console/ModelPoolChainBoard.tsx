import {
  Activity,
  ArrowDown,
  ArrowUp,
  CircleDollarSign,
  Gauge,
  Send,
  TrendingUp,
  UsersRound,
} from "lucide-react";
import { useState } from "react";

import { ScopePager } from "./EntitlementGroupScopeBoard";
import type { ModelPoolProviderLink } from "./modelPoolViewModel";
import {
  buildProviderAvailabilityCells,
  formatAggregateMoney,
  formatAggregateRate,
} from "./providerCardMetrics";

type TranslateFn = (zh: string, en: string) => string;

/**
 * The chain pages instead of scrolling: the card face is a fixed height and the
 * standing rule is that no card ever grows a scrollbar, so a model served by a
 * dozen providers walks through pages at a stable height.
 */
const CHAIN_LINKS_PER_PAGE = 4;

export type ModelPoolChainBoardProps = {
  t: TranslateFn;
  /** Stable per-card prefix for React keys and DOM ids. */
  cardKey: string;
  model: string;
  chain: ModelPoolProviderLink[];
  editorLocked: boolean;
  /**
   * The complement of the provider selection, owned by the card so the expanded
   * account panel can filter by the very same choice.
   */
  deselectedProviderIds: readonly string[];
  onDeselectedProviderIdsChange: (next: string[]) => void;
  onMoveProvider: (providerId: string, direction: "up" | "down") => void;
};

/**
 * The model card back: the provider fallback chain, first link tried first. The
 * operator reorders links with the arrow buttons, and selecting links narrows
 * the account panel below the card the same way the entitlement group card's
 * provider tabs do.
 */
export function ModelPoolChainBoard({
  t,
  cardKey,
  model,
  chain,
  editorLocked,
  deselectedProviderIds,
  onDeselectedProviderIdsChange,
  onMoveProvider,
}: ModelPoolChainBoardProps) {
  const [chainPageState, setChainPageState] = useState(1);

  const selectedCount = chain.filter(
    (link) => !deselectedProviderIds.includes(link.providerId),
  ).length;
  const allSelected = chain.length > 0 && selectedCount === chain.length;
  const chainTotalPages = Math.max(1, Math.ceil(chain.length / CHAIN_LINKS_PER_PAGE));
  const chainPage = Math.min(chainPageState, chainTotalPages);
  const pageLinks = chain.slice(
    (chainPage - 1) * CHAIN_LINKS_PER_PAGE,
    chainPage * CHAIN_LINKS_PER_PAGE,
  );

  const toggleProvider = (providerId: string) => {
    onDeselectedProviderIdsChange(
      deselectedProviderIds.includes(providerId)
        ? deselectedProviderIds.filter((candidate) => candidate !== providerId)
        : [...deselectedProviderIds, providerId],
    );
  };

  return (
    <div className="nt-entitlement-scope" data-model-chain-board={cardKey}>
      <section
        className="nt-entitlement-scope__band nt-entitlement-scope__band--models"
        aria-label={t(`${model} 服务商优先级链`, `${model} provider priority chain`)}
      >
        <header className="nt-entitlement-scope__band-head">
          <strong>{t("优先级链", "Priority chain")}</strong>
          <span className="nt-entitlement-scope__band-tools">
            <span className="nt-entitlement-scope__count">
              {selectedCount}/{chain.length}
            </span>
            {chain.length > 1 ? (
              <button
                className="nt-entitlement-scope__band-action"
                type="button"
                onClick={() =>
                  onDeselectedProviderIdsChange(
                    allSelected ? chain.map((link) => link.providerId) : [],
                  )
                }
              >
                {allSelected ? t("清空", "Clear") : t("全选", "Select all")}
              </button>
            ) : null}
          </span>
        </header>
        {pageLinks.length > 0 ? (
          <>
            <ul className="nt-entitlement-scope__model-list nt-model-chain__list">
              {pageLinks.map((link) => {
                const chainIndex = chain.findIndex(
                  (candidate) => candidate.providerId === link.providerId,
                );
                const successWindows = link.metrics?.successWindows ?? [];
                const availabilityCells = buildProviderAvailabilityCells(successWindows);
                const successLabel = formatAggregateRate(link.metrics?.successRate ?? null);
                const selected = !deselectedProviderIds.includes(link.providerId);
                return (
                  <li
                    className={`nt-entitlement-scope__model-row nt-model-chain__row${link.detached ? " nt-model-chain__row--detached" : ""}`}
                    key={`${cardKey}:link:${link.providerId}`}
                    data-model-chain-provider={link.providerId}
                    data-model-chain-rank={chainIndex + 1}
                  >
                    <div className="nt-entitlement-scope__model-head">
                      <span
                        className="nt-model-chain__order"
                        aria-hidden="true"
                        title={
                          chainIndex === 0
                            ? t("首选服务商", "Primary provider")
                            : t(`第 ${chainIndex + 1} 顺位`, `Fallback ${chainIndex}`)
                        }
                      >
                        {chainIndex + 1}
                      </span>
                      <button
                        className="nt-model-chain__label"
                        type="button"
                        aria-pressed={selected}
                        title={t(
                          `${link.providerLabel}，${link.enabledAccountCount}/${link.accountCount} 个可用账号`,
                          `${link.providerLabel}, ${link.enabledAccountCount}/${link.accountCount} accounts available`,
                        )}
                        onClick={() => toggleProvider(link.providerId)}
                      >
                        <strong>{link.providerLabel}</strong>
                      </button>
                      {link.detached ? (
                        <span className="nt-model-chain__detached">
                          {t("无账号", "No accounts")}
                        </span>
                      ) : null}
                      <span
                        className="nt-entitlement-scope__model-concurrency"
                        title={t("并发数", "Concurrency")}
                      >
                        <Gauge size={13} aria-hidden="true" />
                        <span
                          aria-label={t("并发数", "Concurrency")}
                          data-model-chain-metric="concurrency"
                        >
                          {link.metrics?.concurrency
                            ? `${link.metrics.concurrency.used}/${link.metrics.concurrency.total ?? "—"}`
                            : "—"}
                        </span>
                      </span>
                      <span className="nt-model-chain__moves">
                        <button
                          className="nt-icon-action"
                          type="button"
                          disabled={editorLocked || chainIndex <= 0}
                          title={t("上移一位", "Move up one slot")}
                          aria-label={t(
                            `将 ${link.providerLabel} 上移一位`,
                            `Move ${link.providerLabel} up one slot`,
                          )}
                          onClick={() => onMoveProvider(link.providerId, "up")}
                        >
                          <ArrowUp size={13} aria-hidden="true" />
                        </button>
                        <button
                          className="nt-icon-action"
                          type="button"
                          disabled={editorLocked || chainIndex >= chain.length - 1}
                          title={t("下移一位", "Move down one slot")}
                          aria-label={t(
                            `将 ${link.providerLabel} 下移一位`,
                            `Move ${link.providerLabel} down one slot`,
                          )}
                          onClick={() => onMoveProvider(link.providerId, "down")}
                        >
                          <ArrowDown size={13} aria-hidden="true" />
                        </button>
                      </span>
                    </div>
                    <dl className="nt-entitlement-scope__model-metrics">
                      <div
                        title={t(
                          `可用账户数 ${link.enabledAccountCount}/${link.accountCount}`,
                          `Available accounts ${link.enabledAccountCount}/${link.accountCount}`,
                        )}
                      >
                        <dt aria-label={t("可用账户数", "Available accounts")}>
                          <UsersRound size={13} aria-hidden="true" />
                        </dt>
                        <dd data-model-chain-metric="accounts">
                          {link.enabledAccountCount}/{link.accountCount}
                        </dd>
                      </div>
                      <div title={t("费用", "Cost")}>
                        <dt aria-label={t("费用", "Cost")}>
                          <CircleDollarSign size={13} aria-hidden="true" />
                        </dt>
                        <dd data-model-chain-metric="upstream-cost">
                          {link.metrics?.upstreamCost == null
                            ? "—"
                            : `$${formatAggregateMoney(link.metrics.upstreamCost)}`}
                        </dd>
                      </div>
                      <div title={t("收入", "Revenue")}>
                        <dt aria-label={t("收入", "Revenue")}>
                          <TrendingUp size={13} aria-hidden="true" />
                        </dt>
                        <dd data-model-chain-metric="platform-revenue">
                          {link.metrics?.platformRevenue == null
                            ? "—"
                            : `$${formatAggregateMoney(link.metrics.platformRevenue)}`}
                        </dd>
                      </div>
                      <div title={t("请求数", "Requests")}>
                        <dt aria-label={t("请求数", "Requests")}>
                          <Send size={13} aria-hidden="true" />
                        </dt>
                        <dd data-model-chain-metric="requests">
                          {link.metrics?.requests == null
                            ? "—"
                            : link.metrics.requests.toLocaleString("en-US")}
                        </dd>
                      </div>
                    </dl>
                    <section
                      className="nt-entitlement-scope__model-success"
                      data-provider-availability-scope="model-chain"
                      role="img"
                      aria-label={t(
                        `${link.providerLabel} 调用 ${model} 的成功率${link.metrics?.successRate == null ? "，暂无数据" : `，${link.metrics.successSuccessCount}/${link.metrics.successRequestCount} 次成功，${successLabel}`}`,
                        `${link.providerLabel} success rate for ${model}${link.metrics?.successRate == null ? ", unavailable" : `, ${link.metrics.successSuccessCount}/${link.metrics.successRequestCount} successful, ${successLabel}`}`,
                      )}
                      title={t(
                        `${link.providerLabel} 调用成功率 ${successLabel}`,
                        `${link.providerLabel} success rate ${successLabel}`,
                      )}
                    >
                      <Activity size={13} aria-hidden="true" />
                      {successWindows.length > 0 ? (
                        <div className="nt-provider-card__availability-windows" aria-hidden="true">
                          {successWindows.map((window, windowIndex) => (
                            <div
                              className="nt-provider-card__availability-window"
                              key={`${cardKey}:${link.providerId}:window:${window.label}`}
                            >
                              {availabilityCells
                                .filter((cell) => cell.windowIndex === windowIndex)
                                .map((cell) => (
                                  <span
                                    className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
                                    key={`${cardKey}:${link.providerId}:cell:${cell.windowIndex}:${cell.position}`}
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
                        data-model-chain-metric="success-rate"
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
              page={chainPage}
              totalPages={chainTotalPages}
              navLabel={t("优先级链分页", "Priority chain pagination")}
              onPage={(next) => setChainPageState(next)}
            />
          </>
        ) : (
          <p className="nt-entitlement-scope__empty">
            {t("暂无服务商支持这个模型", "No provider serves this model yet")}
          </p>
        )}
      </section>
    </div>
  );
}
