import {
  Activity,
  ArrowLeftRight,
  CalendarClock,
  Database,
  Ellipsis,
  GalleryHorizontalEnd,
  Pencil,
  Trash2,
} from "lucide-react";
import type { ReactNode, RefObject } from "react";

import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import { ProviderCardFrontBody } from "./ProviderCardFrontBody";
import { providerCardVisual } from "./providerCardVisual";
import type { ProviderCardSnapshot } from "./providerCardSnapshot";
import type { useAccountCardMenu } from "./useAccountCardMenu";

type ProviderLedgerCardProps = {
  section: AccountsLedgerPilotSection;
  snapshot: ProviderCardSnapshot;
  t: AccountsLedgerWorkspaceProps["t"];
  editorLocked: boolean;
  accountLibraryExpanded: boolean;
  flipped: boolean;
  providerModelMappingCount: number;
  toggleProviderAccountLibrary: (section: AccountsLedgerPilotSection) => void;
  toggleProviderFlip: (section: AccountsLedgerPilotSection, side: "front" | "back") => void;
  providerFlipButtonRefs: RefObject<Map<string, HTMLButtonElement>>;
  menu: ReturnType<typeof useAccountCardMenu>;
  handlers: Pick<AccountsLedgerWorkspaceProps,
    "onToggleDispatch" | "onEdit" | "onOpenProviderProbe" |
    "onOpenProviderSchedule" | "onOpenModelMapping"
  > & { requestCredentialRemoval: AccountsLedgerWorkspaceProps["onRemove"] };
  children: ReactNode;
};

// Both faces remain mounted; the ledger owns shared menu and flip-focus state.
export function ProviderLedgerCard({
  section,
  snapshot,
  t,
  editorLocked,
  accountLibraryExpanded,
  flipped,
  providerModelMappingCount,
  toggleProviderAccountLibrary,
  toggleProviderFlip,
  providerFlipButtonRefs,
  menu,
  handlers,
  children,
}: ProviderLedgerCardProps) {
  const {
    activeMenuKey: activePilotActionMenuKey,
    setActiveMenuKey: setActivePilotActionMenuKey,
    menuTriggerRefs: pilotMenuTriggerRefs,
  } = menu;
  const {
    onToggleDispatch,
    onEdit,
    onOpenProviderProbe,
    onOpenProviderSchedule,
    onOpenModelMapping,
    requestCredentialRemoval,
  } = handlers;
  const poolSegments = snapshot.poolSegments;
  const providerConcurrency = snapshot.concurrency;
  const providerCosts = snapshot.costs;
  const providerRequestCount = snapshot.requestCount;
  const providerSuccessWindows = snapshot.successWindows;
  const providerSuccessTotals = snapshot.successTotals;
  const providerSuccessRate = snapshot.successRate;
  const providerAvailabilityCells = snapshot.availabilityCells;
  const providerQuotaWindows = snapshot.quotaWindows;
  const providerQuotaRemainingUsd = snapshot.quotaRemainingUsd;
  const actionableAccounts = snapshot.actionableAccounts;
  const hasProbeTargets = snapshot.hasProbeTargets;
  const singleActionableAccount =
    actionableAccounts.length === 1 ? actionableAccounts[0] : null;
  const providerDispatchEnabled =
    actionableAccounts.length > 0 &&
    actionableAccounts.every((account) => account.dispatchEnabled);
  const providerMenuKey = `provider:${section.providerId}`;
  const visual = providerCardVisual({
    providerId: section.providerId,
    providerLabel: section.providerLabel,
    providerPreset: section.providerPreset,
    adapter: section.adapter,
    protocolProfile: section.protocolProfile,
  });
  const accountLibraryId = `provider-account-library-${section.providerId.replace(/[^a-zA-Z0-9_-]/g, "-")}`;

  return (
    <div
      className="nt-provider-card-stack"
      data-provider-card-stack={section.providerId}
      data-provider-category={visual.category}
    >
    <article
      className={
        flipped
          ? "nt-provider-card nt-provider-card--flipped nt-provider-pilot"
          : "nt-provider-card nt-provider-pilot"
      }
      data-provider-card={section.providerId}
      data-provider-category={visual.category}
      aria-label={t(
        `${section.providerLabel} 卡牌`,
        `${section.providerLabel} card`,
      )}
      data-provider-card-side={flipped ? "back" : "front"}
    >
      <div className="nt-provider-card__inner">
        <div
          className="nt-provider-card__face nt-provider-card__front"
          aria-hidden={flipped}
          inert={flipped ? true : undefined}
        >
        <header className="nt-provider-card__front-head">
          <span
            className={`nt-provider-card__icon nt-provider-card__icon--${visual.iconKey}`}
            data-provider-icon={visual.iconKey}
            title={section.vendorLabel}
            aria-hidden="true"
          >
            {visual.iconLabel}
          </span>
          <button
            className="nt-provider-card__title nt-provider-tree-item nt-provider-tree-item--level-0"
            type="button"
            aria-label={section.providerLabel}
            aria-expanded={accountLibraryExpanded}
            aria-controls={accountLibraryId}
            onClick={() => toggleProviderAccountLibrary(section)}
          >
            <div className="nt-provider-tree-item__main">
              <strong>{section.providerLabel}</strong>
            </div>
          </button>
          <div className="nt-provider-card__head-actions">
            <button
              className="nt-icon-action nt-provider-card__library-toggle"
              type="button"
              title={
                accountLibraryExpanded
                  ? t("收起账号库", "Hide account library")
                  : t("显示账号库", "Show account library")
              }
              aria-label={
                accountLibraryExpanded
                  ? t(`收起 ${section.providerLabel} 账号库`, `Hide ${section.providerLabel} account library`)
                  : t(`显示 ${section.providerLabel} 账号库`, `Show ${section.providerLabel} account library`)
              }
              aria-expanded={accountLibraryExpanded}
              aria-controls={accountLibraryId}
              onClick={() => toggleProviderAccountLibrary(section)}
            >
              <Database size={15} aria-hidden="true" />
            </button>
            <button
              className="nt-icon-action nt-provider-card__flip nt-provider-card__flip--icon"
              type="button"
              title={t(
                `翻面查看 ${section.providerLabel} 详情`,
                `Flip ${section.providerLabel} for details`,
              )}
              aria-label={t(
                `翻面查看 ${section.providerLabel} 详情`,
                `Flip ${section.providerLabel} for details`,
              )}
              aria-pressed={false}
              ref={(node) => {
                const refKey = `${section.providerId}:front`;
                if (node) {
                  providerFlipButtonRefs.current.set(refKey, node);
                } else {
                  providerFlipButtonRefs.current.delete(refKey);
                }
              }}
              onClick={() => toggleProviderFlip(section, "back")}
            >
              <GalleryHorizontalEnd size={16} aria-hidden="true" />
            </button>
          </div>
        </header>

        <ProviderCardFrontBody
          providerLabel={section.providerLabel}
          poolSegments={poolSegments}
          providerConcurrency={providerConcurrency}
          providerCosts={providerCosts}
          providerRequestCount={providerRequestCount}
          providerSuccessWindows={providerSuccessWindows}
          providerSuccessTotals={providerSuccessTotals}
          providerSuccessRate={providerSuccessRate}
          providerAvailabilityCells={providerAvailabilityCells}
          providerQuotaWindows={providerQuotaWindows}
          providerQuotaRemainingUsd={providerQuotaRemainingUsd}
          t={t}
        />

        <footer className="nt-provider-card__actions">
          <button
            className={
              providerDispatchEnabled
                ? "nt-provider-card__action nt-provider-card__action--icon nt-provider-card__action--active"
                : "nt-provider-card__action nt-provider-card__action--icon"
            }
            type="button"
            role="switch"
            aria-checked={providerDispatchEnabled}
            disabled={editorLocked || actionableAccounts.length === 0}
            aria-label={t(
              `${section.providerLabel} 调度开关`,
              `${section.providerLabel} dispatch`,
            )}
            title={t("调度", "Dispatch")}
            onClick={() => {
              const nextEnabled = !providerDispatchEnabled;
              actionableAccounts.forEach((account) =>
                onToggleDispatch(account.providerId, account.accountId, nextEnabled),
              );
            }}
          >
            <span className="nt-switch__track" aria-hidden="true">
              <span className="nt-switch__thumb" />
            </span>
          </button>
          <button
            className="nt-provider-card__action nt-provider-card__action--icon"
            type="button"
            disabled={editorLocked || !singleActionableAccount}
            aria-label={t("编辑", "Edit")}
            title={
              singleActionableAccount
                ? t("编辑账号", "Edit account")
                : t("多账号请在明细中编辑", "Edit multiple accounts in details")
            }
            onClick={() =>
              singleActionableAccount &&
              onEdit(singleActionableAccount.providerId, singleActionableAccount.accountId)
            }
          >
            <Pencil size={14} aria-hidden="true" />
          </button>
          <button
            className="nt-provider-card__action nt-provider-card__action--icon nt-provider-card__action--danger"
            type="button"
            disabled={editorLocked || !singleActionableAccount}
            aria-label={t("删除", "Delete")}
            title={
              singleActionableAccount
                ? t("删除账号", "Delete account")
                : t("多账号请在明细中删除", "Delete multiple accounts in details")
            }
            onClick={() =>
              singleActionableAccount &&
              requestCredentialRemoval(
                singleActionableAccount.providerId,
                singleActionableAccount.accountId,
                singleActionableAccount.displayName,
              )
            }
          >
            <Trash2 size={14} aria-hidden="true" />
          </button>
          {/* Always the fourth slot, even with nothing probeable in
              the section: every card in the console carries the same
              four actions in the same place, so the menu greys its
              items out rather than disappearing. */}
          <div className="nt-pilot-menu" data-pilot-menu-key={providerMenuKey}>
            <button
              className="nt-provider-card__action nt-provider-card__action--icon"
              type="button"
              ref={(node) => {
                if (node) {
                  pilotMenuTriggerRefs.current.set(providerMenuKey, node);
                } else {
                  pilotMenuTriggerRefs.current.delete(providerMenuKey);
                }
              }}
              aria-haspopup="menu"
              aria-expanded={activePilotActionMenuKey === providerMenuKey}
              aria-label={t(
                `${section.providerLabel} 更多操作`,
                `${section.providerLabel} more actions`,
              )}
              title={t("更多", "More")}
              onClick={() =>
                setActivePilotActionMenuKey((current) =>
                  current === providerMenuKey ? null : providerMenuKey,
                )
              }
            >
              <Ellipsis size={14} aria-hidden="true" />
            </button>
            {activePilotActionMenuKey === providerMenuKey ? (
              <div className="nt-pilot-menu__panel" role="menu">
                <button
                  className="nt-pilot-menu__item"
                  role="menuitem"
                  type="button"
                  disabled={!hasProbeTargets}
                  title={
                    hasProbeTargets
                      ? undefined
                      : t("没有可测试的账号", "No testable account in this section")
                  }
                  onClick={() => {
                    setActivePilotActionMenuKey(null);
                    onOpenProviderProbe(section);
                  }}
                >
                  <Activity size={15} aria-hidden="true" />
                  <span>{t("服务商测试", "Test provider")}</span>
                </button>
                <button
                  className="nt-pilot-menu__item"
                  role="menuitem"
                  type="button"
                  disabled={editorLocked || !hasProbeTargets}
                  title={
                    hasProbeTargets
                      ? undefined
                      : t("没有可测试的账号", "No testable account in this section")
                  }
                  onClick={() => {
                    setActivePilotActionMenuKey(null);
                    onOpenProviderSchedule(section);
                  }}
                >
                  <CalendarClock size={15} aria-hidden="true" />
                  <span>{t("自动定时测试", "Automatic scheduled tests")}</span>
                </button>
                <button
                  className="nt-pilot-menu__item"
                  role="menuitem"
                  type="button"
                  disabled={editorLocked}
                  onClick={() => {
                    setActivePilotActionMenuKey(null);
                    onOpenModelMapping(section);
                  }}
                >
                  <ArrowLeftRight size={15} aria-hidden="true" />
                  <span>
                    {providerModelMappingCount > 0
                      ? t(
                          `模型映射 · ${providerModelMappingCount}`,
                          `Model mapping · ${providerModelMappingCount}`,
                        )
                      : t("模型映射", "Model mapping")}
                  </span>
                </button>
              </div>
            ) : null}
          </div>
        </footer>
        </div>
      <div
        className="nt-provider-card__face nt-provider-card__back"
        aria-hidden={!flipped}
        inert={!flipped ? true : undefined}
      >
        <header className="nt-provider-card__back-head">
          <div className="nt-provider-card__back-title">
            <strong>{section.providerLabel}</strong>
          </div>
          <div className="nt-provider-card__head-actions">
            <button
              className="nt-icon-action nt-provider-card__library-toggle"
              type="button"
              title={
                accountLibraryExpanded
                  ? t("收起账号库", "Hide account library")
                  : t("显示账号库", "Show account library")
              }
              aria-label={
                accountLibraryExpanded
                  ? t(`收起 ${section.providerLabel} 账号库`, `Hide ${section.providerLabel} account library`)
                  : t(`显示 ${section.providerLabel} 账号库`, `Show ${section.providerLabel} account library`)
              }
              aria-expanded={accountLibraryExpanded}
              aria-controls={accountLibraryId}
              onClick={() => toggleProviderAccountLibrary(section)}
            >
              <Database size={15} aria-hidden="true" />
            </button>
            <button
              className="nt-icon-action nt-provider-card__flip nt-provider-card__flip--icon"
              type="button"
              title={t(
                `翻回 ${section.providerLabel} 卡牌正面`,
                `Flip ${section.providerLabel} back to the front`,
              )}
              aria-label={t(
                `翻回 ${section.providerLabel} 卡牌正面`,
                `Flip ${section.providerLabel} back to the front`,
              )}
              aria-pressed={true}
              ref={(node) => {
                const refKey = `${section.providerId}:back`;
                if (node) {
                  providerFlipButtonRefs.current.set(refKey, node);
                } else {
                  providerFlipButtonRefs.current.delete(refKey);
                }
              }}
              onClick={() => toggleProviderFlip(section, "front")}
            >
              <GalleryHorizontalEnd size={16} aria-hidden="true" />
            </button>
          </div>
        </header>
        <section
          className="nt-provider-card__back-body"
          aria-label={t(
            `${section.providerLabel} 账号生命周期`,
            `${section.providerLabel} credential lifecycle`,
          )}
        >
          {children}
        </section>
      </div>
      </div>
      </article>
    </div>
  );
}
