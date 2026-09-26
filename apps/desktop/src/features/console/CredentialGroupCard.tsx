import {
  Activity,
  CircleDollarSign,
  Database,
  Ellipsis,
  GalleryHorizontalEnd,
  Gauge,
  Pencil,
  Send,
  Trash2,
  TrendingUp,
  UsersRound,
} from "lucide-react";
import type { Dispatch, SetStateAction } from "react";

import type { CredentialGroupsWorkspaceProps, TranslateFn } from "./credentialGroupsWorkspaceTypes";
import type { CredentialGroupCardSnapshot } from "./credentialGroupCardSnapshot";
import { EntitlementGroupScopeBoard } from "./EntitlementGroupScopeBoard";
import { formatAggregateMoney, formatAggregateRate } from "./providerCardMetrics";

type CredentialGroupCardProps = Pick<
  CredentialGroupsWorkspaceProps,
  "editorLocked" | "onSelectGroup" | "onToggleEnabled" | "onRemoveGroup"
> & {
  t: TranslateFn;
  snapshot: CredentialGroupCardSnapshot;
  expanded: boolean;
  flipped: boolean;
  activeMenuKey: string | null;
  onActiveMenuKeyChange: Dispatch<SetStateAction<string | null>>;
  registerMenuTrigger: (key: string, node: HTMLButtonElement | null) => void;
  registerFlipButton: (
    rowId: string,
    side: "front" | "back",
    node: HTMLButtonElement | null,
  ) => void;
  onToggleCard: (rowId: string) => void;
  onFlip: (rowId: string, label: string, side: "front" | "back") => void;
  onDeselectedProviderIdsChange: (rowId: string, providerIds: string[]) => void;
};

export function CredentialGroupCard({
  t,
  snapshot,
  expanded,
  flipped,
  editorLocked,
  activeMenuKey,
  onActiveMenuKeyChange,
  registerMenuTrigger,
  registerFlipButton,
  onToggleCard,
  onFlip,
  onDeselectedProviderIdsChange,
  onSelectGroup,
  onToggleEnabled,
  onRemoveGroup,
}: CredentialGroupCardProps) {
  const {
    group,
    label,
    accountPanelId,
    cardMenuKey,
    deselectedProviderIds,
    successWindows,
    availabilityCells,
  } = snapshot;
  const metrics = group.metrics;

  return (
    <article
      className={`nt-entitlement-group-card${flipped ? " nt-entitlement-group-card--flipped" : ""}${expanded ? " nt-entitlement-group-card--expanded" : ""}`}
      data-entitlement-group-card={group.groupId || group.rowId}
      data-entitlement-group-card-side={flipped ? "back" : "front"}
      aria-label={t(`${label} 权益组卡牌`, `${label} entitlement group card`)}
    >
      <div className="nt-entitlement-group-card__inner">
        <div
          className="nt-entitlement-group-card__face nt-entitlement-group-card__front"
          aria-hidden={flipped}
          inert={flipped ? true : undefined}
        >
          <header className="nt-entitlement-group-card__head">
            <span className="nt-entitlement-group-card__icon" aria-hidden="true">
              <UsersRound size={17} />
            </span>
            <button
              className="nt-entitlement-group-card__toggle"
              type="button"
              aria-label={label}
              aria-expanded={expanded}
              aria-controls={accountPanelId}
              onClick={() => onToggleCard(group.rowId)}
            >
              <strong>{label}</strong>
            </button>
            <div className="nt-entitlement-group-card__head-actions">
              <button
                className="nt-icon-action nt-entitlement-group-card__library-toggle"
                type="button"
                title={
                  expanded
                    ? t("收起组内账号", "Hide group accounts")
                    : t("显示组内账号", "Show group accounts")
                }
                aria-label={
                  expanded
                    ? t(`收起 ${label} 组内账号`, `Hide ${label} group accounts`)
                    : t(`显示 ${label} 组内账号`, `Show ${label} group accounts`)
                }
                aria-expanded={expanded}
                aria-controls={accountPanelId}
                onClick={() => onToggleCard(group.rowId)}
              >
                <Database size={15} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-entitlement-group-card__flip"
                type="button"
                title={t(`翻面查看 ${label} 详情`, `Flip ${label} for details`)}
                aria-label={t(`翻面查看 ${label} 详情`, `Flip ${label} for details`)}
                aria-pressed={false}
                ref={(node) => registerFlipButton(group.rowId, "front", node)}
                onClick={() => onFlip(group.rowId, label, "back")}
              >
                <GalleryHorizontalEnd size={16} aria-hidden="true" />
              </button>
            </div>
          </header>

          <div className="nt-entitlement-group-card__front-body">
            <dl className="nt-entitlement-group-card__metrics">
              <div>
                <dt>{t("服务商", "Providers")}</dt>
                <dd>{group.providerLabels.length}</dd>
              </div>
              <div>
                <dt>{t("账号数", "Accounts")}</dt>
                <dd data-entitlement-group-metric="accounts">{group.memberCount}</dd>
              </div>
              <div>
                <dt>{t("计费倍率", "Billing")}</dt>
                <dd>{group.billingMultiplier.trim() || "1.0"}</dd>
              </div>
            </dl>

            <dl className="nt-entitlement-group-card__metrics nt-entitlement-group-card__usage">
              <div title={t(`${label} 聚合并发`, `${label} aggregated concurrency`)}>
                <dt aria-label={t("并发", "Concurrency")}>
                  <Gauge size={15} aria-hidden="true" />
                </dt>
                <dd data-entitlement-group-metric="concurrency">
                  {metrics?.concurrency
                    ? `${metrics.concurrency.used}/${metrics.concurrency.total ?? "—"}`
                    : "—"}
                </dd>
              </div>
              <div title={t(`${label} 聚合上游费用`, `${label} aggregated upstream cost`)}>
                <dt aria-label={t("上游费用", "Upstream cost")}>
                  <CircleDollarSign size={15} aria-hidden="true" />
                </dt>
                <dd data-entitlement-group-metric="upstream-cost">
                  {metrics?.upstreamCost == null
                    ? "—"
                    : `$${formatAggregateMoney(metrics.upstreamCost)}`}
                </dd>
              </div>
              <div title={t(`${label} 聚合收费`, `${label} aggregated revenue`)}>
                <dt aria-label={t("收费", "Revenue")}>
                  <TrendingUp size={15} aria-hidden="true" />
                </dt>
                <dd data-entitlement-group-metric="platform-revenue">
                  {metrics?.platformRevenue == null
                    ? "—"
                    : `$${formatAggregateMoney(metrics.platformRevenue)}`}
                </dd>
              </div>
              <div
                title={t(
                  `${label} 聚合总请求数${metrics?.requests == null ? "暂无数据" : ` ${metrics.requests}`}`,
                  `${label} aggregated requests${metrics?.requests == null ? " unavailable" : ` ${metrics.requests}`}`,
                )}
              >
                <dt aria-label={t("请求数", "Requests")}>
                  <Send size={15} aria-hidden="true" />
                </dt>
                <dd data-entitlement-group-metric="requests">
                  {metrics?.requests == null ? "—" : metrics.requests.toLocaleString("en-US")}
                </dd>
              </div>
            </dl>

            <section
              className="nt-entitlement-group-card__success"
              data-provider-availability-scope="entitlement-group"
              role="img"
              aria-label={t(
                `${label} 最近窗口的调用成功率，组内账号聚合${metrics?.successRate == null ? "，暂无数据" : `，${metrics.successSuccessCount}/${metrics.successRequestCount} 次成功，${formatAggregateRate(metrics.successRate)}`}`,
                `${label} recent window success rate, member accounts aggregated${metrics?.successRate == null ? ", unavailable" : `, ${metrics.successSuccessCount}/${metrics.successRequestCount} successful, ${formatAggregateRate(metrics.successRate)}`}`,
              )}
              title={t(
                `${label} 调用成功率 ${formatAggregateRate(metrics?.successRate ?? null)}`,
                `${label} success rate ${formatAggregateRate(metrics?.successRate ?? null)}`,
              )}
            >
              <Activity size={15} aria-hidden="true" />
              {successWindows.length > 0 ? (
                <div className="nt-provider-card__availability-windows" aria-hidden="true">
                  {successWindows.map((window, windowIndex) => (
                    <div
                      className="nt-provider-card__availability-window"
                      key={`${group.rowId}:success:${window.label}`}
                    >
                      {availabilityCells
                        .filter((cell) => cell.windowIndex === windowIndex)
                        .map((cell) => (
                          <span
                            className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`}
                            key={`${group.rowId}:cell:${cell.windowIndex}:${cell.position}`}
                            title={`${cell.windowLabel}: ${cell.success}/${cell.requests} (${formatAggregateRate(cell.rate)})`}
                          />
                        ))}
                    </div>
                  ))}
                </div>
              ) : (
                <span className="nt-entitlement-group-card__success-empty" aria-hidden="true">
                  {t("暂无调用数据", "No dispatch data yet")}
                </span>
              )}
              <strong
                className="nt-provider-card__success-rate"
                data-entitlement-group-metric="success-rate"
              >
                {formatAggregateRate(metrics?.successRate ?? null)}
              </strong>
            </section>
          </div>

          <footer className="nt-entitlement-group-card__actions">
            <button
              className={
                group.enabled
                  ? "nt-entitlement-group-card__action nt-entitlement-group-card__action--icon nt-entitlement-group-card__action--active"
                  : "nt-entitlement-group-card__action nt-entitlement-group-card__action--icon"
              }
              type="button"
              role="switch"
              aria-checked={group.enabled}
              disabled={editorLocked}
              aria-label={t(`${label} 启用状态`, `${label} enabled state`)}
              title={t("启用", "Enabled")}
              onClick={() => onToggleEnabled(group.rowId, !group.enabled)}
            >
              <span className="nt-switch__track" aria-hidden="true">
                <span className="nt-switch__thumb" />
              </span>
            </button>
            <button
              className="nt-entitlement-group-card__action nt-entitlement-group-card__action--icon"
              type="button"
              disabled={editorLocked}
              aria-label={t("编辑", "Edit")}
              title={t("编辑权益组", "Edit entitlement group")}
              onClick={() => onSelectGroup(group.rowId)}
            >
              <Pencil size={14} aria-hidden="true" />
            </button>
            <button
              className="nt-entitlement-group-card__action nt-entitlement-group-card__action--icon nt-entitlement-group-card__action--danger"
              type="button"
              disabled={editorLocked}
              aria-label={t("删除", "Delete")}
              title={t("移除权益组", "Remove entitlement group")}
              onClick={() => onRemoveGroup(group.rowId)}
            >
              <Trash2 size={14} aria-hidden="true" />
            </button>
            <div className="nt-pilot-menu" data-pilot-menu-key={cardMenuKey}>
              <button
                className="nt-entitlement-group-card__action nt-entitlement-group-card__action--icon"
                type="button"
                ref={(node) => registerMenuTrigger(cardMenuKey, node)}
                aria-haspopup="menu"
                aria-expanded={activeMenuKey === cardMenuKey}
                aria-label={t(`${label} 更多操作`, `${label} more actions`)}
                title={t("更多", "More")}
                onClick={() =>
                  onActiveMenuKeyChange((current) => (current === cardMenuKey ? null : cardMenuKey))
                }
              >
                <Ellipsis size={14} aria-hidden="true" />
              </button>
              {activeMenuKey === cardMenuKey ? (
                <div className="nt-pilot-menu__panel" role="menu">
                  <button
                    className="nt-pilot-menu__item"
                    role="menuitem"
                    type="button"
                    aria-expanded={expanded}
                    aria-controls={accountPanelId}
                    onClick={() => {
                      onActiveMenuKeyChange(null);
                      onToggleCard(group.rowId);
                    }}
                  >
                    <Database size={15} aria-hidden="true" />
                    <span>
                      {expanded
                        ? t("收起组内账号", "Hide member accounts")
                        : t("显示组内账号", "Show member accounts")}
                    </span>
                  </button>
                  <button
                    className="nt-pilot-menu__item"
                    role="menuitem"
                    type="button"
                    onClick={() => {
                      onActiveMenuKeyChange(null);
                      onFlip(group.rowId, label, "back");
                    }}
                  >
                    <GalleryHorizontalEnd size={15} aria-hidden="true" />
                    <span>{t("服务商与模型范围", "Provider and model scope")}</span>
                  </button>
                </div>
              ) : null}
            </div>
          </footer>
        </div>

        <div
          className="nt-entitlement-group-card__face nt-entitlement-group-card__back"
          aria-hidden={!flipped}
          inert={!flipped ? true : undefined}
        >
          <header className="nt-entitlement-group-card__back-head">
            <div className="nt-entitlement-group-card__back-title">
              <strong>{label}</strong>
            </div>
            <div className="nt-entitlement-group-card__head-actions">
              <button
                className="nt-icon-action nt-entitlement-group-card__library-toggle"
                type="button"
                title={
                  expanded
                    ? t("收起组内账号", "Hide group accounts")
                    : t("显示组内账号", "Show group accounts")
                }
                aria-label={
                  expanded
                    ? t(`收起 ${label} 组内账号`, `Hide ${label} group accounts`)
                    : t(`显示 ${label} 组内账号`, `Show ${label} group accounts`)
                }
                aria-expanded={expanded}
                aria-controls={accountPanelId}
                onClick={() => onToggleCard(group.rowId)}
              >
                <Database size={15} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-entitlement-group-card__flip"
                type="button"
                title={t(`翻回 ${label} 卡牌正面`, `Flip ${label} back to the front`)}
                aria-label={t(`翻回 ${label} 卡牌正面`, `Flip ${label} back to the front`)}
                aria-pressed={true}
                ref={(node) => registerFlipButton(group.rowId, "back", node)}
                onClick={() => onFlip(group.rowId, label, "front")}
              >
                <GalleryHorizontalEnd size={16} aria-hidden="true" />
              </button>
            </div>
          </header>

          <section
            className="nt-entitlement-group-card__back-body"
            aria-label={t(`${label} 权益范围`, `${label} entitlement scope`)}
          >
            <EntitlementGroupScopeBoard
              t={t}
              cardKey={group.rowId}
              label={label}
              providerScopes={group.providerScopes}
              modelScopes={group.modelScopes}
              deselectedProviderIds={deselectedProviderIds}
              onDeselectedProviderIdsChange={(next) =>
                onDeselectedProviderIdsChange(group.rowId, next)
              }
            />
          </section>
        </div>
      </div>
    </article>
  );
}
