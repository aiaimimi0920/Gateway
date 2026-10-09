import {
  Activity,
  CalendarClock,
  CircleDollarSign,
  ArrowLeftRight,
  Gauge,
  Pencil,
  Plus,
  Send,
  Trash2,
  TrendingUp,
} from "lucide-react";

import { ActionTooltip } from "../../components/ActionTooltip";
import { AccountBillingAction } from "./AccountBillingAction";
import { CardModelList } from "./CardModelList";
import { AccountRoutingPoolSelect } from "./AccountRoutingPoolSelect";
import { ProviderQuotaPanel, quotaWindowRemainingRatio } from "./accountCardQuota";
import type {
  AccountCardGroupOption,
  AccountsLedgerPilotAccount,
  ProviderAccountCardHandlers,
  ProviderAccountCardMenu,
  QuotaDisplayWindow,
  TranslateFn,
} from "./accountCardTypes";
import {
  aggregateProviderConcurrency,
  aggregateProviderCosts,
  aggregateProviderRequests,
  aggregateSuccessWindows,
  buildProviderAvailabilityCells,
  classifyAccountPoolState,
  formatAggregateMoney,
  formatAggregateRate,
} from "./providerCardMetrics";

export {
  ACCOUNT_LIBRARY_CARD_WIDTH,
  ACCOUNT_LIBRARY_GAP,
  ACCOUNT_LIBRARY_ROWS,
  AccountLibraryPager,
} from "./AccountLibraryPager";
export { CredentialRemoveDialog } from "./CredentialRemoveDialog";
export {
  formatQuotaReset,
  ProviderQuotaPanel,
  quotaWindowRemainingRatio,
} from "./accountCardQuota";
export type {
  AccountCardGroupOption,
  AccountCardMenuActionId,
  AccountCardMenuItem,
  AccountsLedgerPilotAccount,
  PendingCredentialRemoval,
  ProviderAccountCardHandlers,
  ProviderAccountCardMenu,
  QuotaDisplayWindow,
  TranslateFn,
} from "./accountCardTypes";
export { ACCOUNT_CARD_MENU_ITEMS, useAccountCardMenu } from "./useAccountCardMenu";

/**
 * The account card of the credential pool. Fixed geometry, front-only: identity
 * row on top, telemetry in the middle, the action row pinned to the bottom so a
 * row of neighbours stays even no matter how much telemetry each one carries.
 */
export function ProviderAccountCard(props: {
  t: TranslateFn;
  account: AccountsLedgerPilotAccount;
  editorLocked: boolean;
  groupOptions: readonly AccountCardGroupOption[];
  handlers: ProviderAccountCardHandlers;
  menu: ProviderAccountCardMenu;
}) {
  const { t, account, editorLocked, groupOptions, handlers, menu } = props;
  const accountPoolState = classifyAccountPoolState(account);
  const accountConcurrency = aggregateProviderConcurrency([account]);
  const accountCosts = aggregateProviderCosts([account]);
  const accountRequestCount = aggregateProviderRequests([account]);
  const accountSuccessWindows = aggregateSuccessWindows([account]).slice(-4);
  const accountAvailabilityCells = buildProviderAvailabilityCells(accountSuccessWindows);
  const accountSuccessTotals = accountSuccessWindows.reduce(
    (totals, window) => ({
      success: totals.success + window.success,
      requests: totals.requests + window.requests,
    }),
    { success: 0, requests: 0 },
  );
  const accountSuccessRate =
    accountSuccessTotals.requests > 0
      ? accountSuccessTotals.success / accountSuccessTotals.requests
      : null;
  const accountQuotaWindows: QuotaDisplayWindow[] = (account.quota?.windows ?? [])
    .slice(0, 2)
    .map((window) => ({
      key: window.key,
      label: window.label,
      remainingRatio: quotaWindowRemainingRatio(window),
      resetAt: window.resetAt,
    }));

  return (
    <article
      className={`nt-provider-account-card nt-provider-account-card--${accountPoolState}`}
      data-account-card={account.accountId}
      data-account-provider={account.providerId}
      aria-label={t(`${account.displayName} 账号`, `${account.displayName} account`)}
    >
      <header className="nt-provider-account-card__head">
        <div className="nt-provider-account-card__identity">
          <strong title={account.displayName}>{account.displayName}</strong>
        </div>
        {account.mode !== "provider-default" ? <ActionTooltip label={t(`编辑账号 ${account.displayName}`, `Edit account ${account.displayName}`)}>
          <button className="nt-icon-action" type="button"
            disabled={editorLocked || account.previewOnly}
            aria-label={t(`编辑账号 ${account.displayName}`, `Edit account ${account.displayName}`)}
            onClick={() => handlers.onEdit(account.providerId, account.accountId)}>
            <Pencil size={14} aria-hidden="true" />
          </button>
        </ActionTooltip> : null}
        <AccountBillingAction providerId={account.providerId} accountId={account.accountId}
          name={account.displayName} locked={editorLocked || Boolean(account.previewOnly)} t={t} />
      </header>

      <div className="nt-provider-account-card__body">
        <dl className="nt-provider-account-card__metrics">
          <div title={t("并发", "Concurrency")}>
            <dt aria-label={t("并发", "Concurrency")}>
              <Gauge size={13} aria-hidden="true" />
            </dt>
            <dd data-account-metric="concurrency">
              {accountConcurrency
                ? `${accountConcurrency.used}/${accountConcurrency.total ?? "—"}`
                : "—"}
            </dd>
          </div>
          <div title={t("请求数", "Requests")}>
            <dt aria-label={t("请求数", "Requests")}>
              <Send size={13} aria-hidden="true" />
            </dt>
            <dd data-account-metric="requests">
              {accountRequestCount === null
                ? "—"
                : accountRequestCount.toLocaleString("en-US")}
            </dd>
          </div>
          <div title={t("上游费用", "Upstream cost")}>
            <dt aria-label={t("上游费用", "Upstream cost")}>
              <CircleDollarSign size={13} aria-hidden="true" />
            </dt>
            <dd data-account-metric="upstream-cost">
              {accountCosts.upstream === null
                ? "—"
                : `$${formatAggregateMoney(accountCosts.upstream)}`}
            </dd>
          </div>
          <div title={t("平台收入", "Platform revenue")}>
            <dt aria-label={t("平台收入", "Platform revenue")}>
              <TrendingUp size={13} aria-hidden="true" />
            </dt>
            <dd data-account-metric="platform-revenue">
              {accountCosts.user === null ? "—" : `$${formatAggregateMoney(accountCosts.user)}`}
            </dd>
          </div>
        </dl>

        {/* Success strip only exists once the account has real windows. An account
            that has never been dispatched used to render 4 x 12 empty cells plus a
            "—" rate, which read as a broken chart instead of "no data yet". */}
        {accountSuccessWindows.length > 0 ? (
          <section
            className="nt-provider-card__success nt-provider-account-card__success"
            data-account-availability-scope="account"
            role="img"
            aria-label={t(
              `${account.displayName} 最近窗口调用成功率${accountSuccessRate === null ? "暂无数据" : ` ${accountSuccessTotals.success}/${accountSuccessTotals.requests}，${formatAggregateRate(accountSuccessRate)}`}`,
              `${account.displayName} recent success rate${accountSuccessRate === null ? " unavailable" : ` ${accountSuccessTotals.success}/${accountSuccessTotals.requests}, ${formatAggregateRate(accountSuccessRate)}`}`,
            )}
          >
            <Activity size={13} aria-hidden="true" />
            <div className="nt-provider-card__availability-windows" aria-hidden="true">
              {accountSuccessWindows.map((window, windowIndex) => {
                const windowTitle = `${window.label}: ${window.success}/${window.requests} (${formatAggregateRate(window.rate)})`;
                return (
                  <div
                    className="nt-provider-card__availability-window"
                    data-account-availability-window={window.label}
                    key={window.label}
                    title={windowTitle}
                  >
                    {accountAvailabilityCells
                      .filter((cell) => cell.windowIndex === windowIndex)
                      .map((cell) => (
                        <span
                          className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
                          data-account-availability-cell={cell.state}
                          key={`${cell.windowIndex}:${cell.position}`}
                        />
                      ))}
                  </div>
                );
              })}
            </div>
            <strong className="nt-provider-card__success-rate" data-account-metric="success-rate">
              {formatAggregateRate(accountSuccessRate)}
            </strong>
          </section>
        ) : null}

        <ProviderQuotaPanel
          t={t}
          scope="account"
          windows={accountQuotaWindows}
          remainingUsd={account.quotaRemainingUsd ?? null}
        />

        <CardModelList models={account.supportedModels ?? []} label={account.displayName} traffic={account.modelTraffic} t={t} />

        <div className="nt-provider-account-card__meta">
          <div className="nt-provider-account-card__groups">
            <span className="nt-provider-account-card__group-label">
              {t("分组池", "Routing pool")}
            </span>
            <AccountRoutingPoolSelect
              groupId={account.logicalGroupIds?.[0] ?? ""}
              options={groupOptions}
              ungroupedLabel={t("未分组", "Ungrouped")}
              disabled={editorLocked || account.previewOnly}
              label={t(
                `调整 ${account.displayName} 分组池`,
                `Change routing pool for ${account.displayName}`,
              )}
              onValueChange={(groupId) => handlers.onSetAccountGroup(account.accountId, groupId)}
            />
          </div>
          <span className="nt-provider-account-card__recent" title={account.recentUseLabel}>
            <CalendarClock size={13} aria-hidden="true" />
            <span>{account.recentUseLabel}</span>
          </span>
        </div>
      </div>

      <footer className="nt-provider-account-card__actions nt-provider-card__actions">
        {account.mode === "provider-default" ? (
          <button
            className="nt-btn nt-btn--secondary nt-btn--compact"
            type="button"
            disabled={editorLocked}
            onClick={() => handlers.onAddExplicit(account.providerId)}
          >
            <Plus size={14} aria-hidden="true" />
            {t("添加显式账号", "Add explicit account")}
          </button>
        ) : (
          <>
            <button className={`nt-provider-card__action${account.enabled ? " nt-provider-card__action--active" : ""}`}
              type="button" role="switch" aria-checked={account.enabled}
              aria-label={t(`调度 ${account.accountId}`, `Dispatch ${account.accountId}`)}
              disabled={editorLocked || !account.dispatchEditable || account.previewOnly}
              onClick={() => handlers.onToggleDispatch(account.providerId, account.accountId, !account.enabled)}>
              <span className="nt-switch__track" aria-hidden="true"><span className="nt-switch__thumb" /></span>
            </button>
            <button className="nt-provider-card__action nt-provider-card__action--icon" type="button"
              disabled={editorLocked || account.previewOnly}
              aria-label={t(`测试账号 ${account.displayName}`, `Test account ${account.displayName}`)}
              onClick={() => menu.onAction("probe", account.providerId, account)}>
              <Activity size={14} aria-hidden="true" />
            </button>
            <button className="nt-provider-card__action nt-provider-card__action--icon" type="button"
              disabled={editorLocked || account.previewOnly}
              aria-label={t(`模型映射 ${account.displayName}`, `Model mapping ${account.displayName}`)}
              title={t("编辑所属凭据池的模型映射", "Edit model mapping for this credential pool")}
              onClick={() => menu.onAction("model-mapping", account.providerId, account)}>
              <ArrowLeftRight size={14} aria-hidden="true" />
            </button>
            <button className="nt-provider-card__action nt-provider-card__action--icon nt-provider-card__action--danger" type="button"
              disabled={editorLocked || account.previewOnly}
              aria-label={t(`删除账号 ${account.displayName}`, `Delete account ${account.displayName}`)}
              onClick={() => handlers.onRequestRemoval(account.providerId, account.accountId, account.displayName)}>
              <Trash2 size={14} aria-hidden="true" />
            </button>
          </>
        )}
      </footer>
    </article>
  );
}
