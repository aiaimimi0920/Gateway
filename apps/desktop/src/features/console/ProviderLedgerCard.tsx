import {
  Activity,
  ArrowLeftRight,
  Database,
  GalleryHorizontalEnd,
  Trash2,
} from "lucide-react";
import type { ReactNode, RefObject } from "react";

import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import { ProviderCardFrontBody } from "./ProviderCardFrontBody";
import { NvidiaBrandIcon } from "./NvidiaBrandIcon";

import { providerCardVisual } from "./providerCardVisual";
import type { ProviderCardSnapshot } from "./providerCardSnapshot";

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

  handlers: Pick<AccountsLedgerWorkspaceProps,
    "onToggleDispatch" |
    "onOpenProviderSchedule" | "onOpenModelMapping"
  > & { onRequestProviderRemoval: (section: AccountsLedgerPilotSection) => void };
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
  handlers,
  children,
}: ProviderLedgerCardProps) {
  const {
    onToggleDispatch,
    onOpenProviderSchedule,
    onOpenModelMapping,
    onRequestProviderRemoval,
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
  const providerDispatchEnabled =
    actionableAccounts.length > 0 &&
    actionableAccounts.every((account) => account.enabled);
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
            {visual.iconKey === "nvidia" ? <NvidiaBrandIcon /> : visual.iconLabel}
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
          supportedModels={snapshot.supportedModels}
          modelTraffic={section.modelTraffic}
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
          <button className="nt-provider-card__action nt-provider-card__action--icon" type="button"
            disabled={!hasProbeTargets} aria-label={t("测试", "Test")} title={t("测试", "Test")}
            onClick={() => onOpenProviderSchedule(section)}>
            <Activity size={14} aria-hidden="true" />
          </button>
          <button className="nt-provider-card__action nt-provider-card__action--icon" type="button"
            disabled={editorLocked} aria-label={t("模型映射", "Model mapping")}
            title={t(`模型映射 · ${providerModelMappingCount}`, `Model mapping · ${providerModelMappingCount}`)}
            onClick={() => onOpenModelMapping(section)}>
            <ArrowLeftRight size={14} aria-hidden="true" />
          </button>
          <button className="nt-provider-card__action nt-provider-card__action--icon nt-provider-card__action--danger" type="button"
            disabled={editorLocked} aria-label={t("删除凭据池", "Delete pool")} title={t("删除凭据池", "Delete pool")}
            onClick={() => onRequestProviderRemoval(section)}>
            <Trash2 size={14} aria-hidden="true" />
          </button>
        </footer>
        </div>
      <div
        className="nt-provider-card__face nt-provider-card__back"
        aria-hidden={!flipped}
        inert={!flipped ? true : undefined}
      >
        <header className="nt-provider-card__back-head">
          <span className={`nt-provider-card__icon nt-provider-card__icon--${visual.iconKey}`}
            data-provider-icon={visual.iconKey} title={section.vendorLabel} aria-hidden="true">
            {visual.iconKey === "nvidia" ? <NvidiaBrandIcon /> : visual.iconLabel}
          </span>
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
