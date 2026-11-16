import {
  Activity,
  ArrowLeftRight,
  Database,
  GalleryHorizontalEnd,
  Trash2,
} from "lucide-react";
import { useState, type ReactNode, type RefObject } from "react";
import { Button, IconButton, Switch } from "@neuro/ui";
import "../../styles/account-workspace-controls.css";

import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import { ProviderCardFrontBody } from "./ProviderCardFrontBody";
import { ProviderBrandIcon } from "./ProviderBrandIcon";

import { providerCardVisual } from "./providerCardVisual";
import { providerAccessLabel } from "./providerAccessKind";
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
  children: () => ReactNode;
};

// Keep face shells for flip focus; mount lifecycle/settings only on first use.
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
  const [backMounted, setBackMounted] = useState(flipped);
  if (flipped && !backMounted) setBackMounted(true);
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
              <ProviderBrandIcon iconKey={visual.iconKey} fallback={visual.iconLabel} title={section.vendorLabel} />
              <Button variant="quiet" compact className="nt-pool-card-title"
                aria-label={providerAccessLabel(section)}
                aria-expanded={accountLibraryExpanded}
                aria-controls={accountLibraryId}
                onClick={() => toggleProviderAccountLibrary(section)}
              >
                <span className="nt-pool-card-title__text">
                  <strong>{providerAccessLabel(section)}</strong>
                </span>
              </Button>
              <div className="nt-provider-card__head-actions">
                <IconButton variant="quiet" compact
                  title={
                    accountLibraryExpanded
                      ? t("收起账号库", "Hide account library")
                      : t("显示账号库", "Show account library")
                  }
                  label={
                    accountLibraryExpanded
                      ? t(`收起 ${section.providerLabel} 账号库`, `Hide ${section.providerLabel} account library`)
                      : t(`显示 ${section.providerLabel} 账号库`, `Show ${section.providerLabel} account library`)
                  }
                  aria-expanded={accountLibraryExpanded}
                  aria-controls={accountLibraryId}
                  onClick={() => toggleProviderAccountLibrary(section)}
                >
                  <Database size={15} aria-hidden="true" />
                </IconButton>
                <IconButton variant="quiet" compact className="nt-provider-card__flip"
                  title={t(
                    `翻面查看 ${section.providerLabel} 详情`,
                    `Flip ${section.providerLabel} for details`,
                  )}
                  label={t(
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
                </IconButton>
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


            <footer className="nt-provider-card__actions nt-pool-card-actions">
              <Switch checked={providerDispatchEnabled}
                disabled={editorLocked || actionableAccounts.length === 0}
                aria-label={t(
                  `${section.providerLabel} 调度开关`,
                  `${section.providerLabel} dispatch`,
                )}
                title={t("调度", "Dispatch")}
                onChange={() => {
                  const nextEnabled = !providerDispatchEnabled;
                  actionableAccounts.forEach((account) =>
                    onToggleDispatch(account.providerId, account.accountId, nextEnabled),
                  );
                }}
              />
              <IconButton
                disabled={!hasProbeTargets} label={t("测试", "Test")} title={t("测试", "Test")}
                onClick={() => onOpenProviderSchedule(section)}>
                <Activity size={14} aria-hidden="true" />
              </IconButton>
              <IconButton
                disabled={editorLocked} label={t("模型映射", "Model mapping")}
                title={t(`模型映射 · ${providerModelMappingCount}`, `Model mapping · ${providerModelMappingCount}`)}
                onClick={() => onOpenModelMapping(section)}>
                <ArrowLeftRight size={14} aria-hidden="true" />
              </IconButton>
              <IconButton variant="danger"
                disabled={editorLocked} label={t("删除凭据池", "Delete pool")} title={t("删除凭据池", "Delete pool")}
                onClick={() => onRequestProviderRemoval(section)}>
                <Trash2 size={14} aria-hidden="true" />
              </IconButton>
            </footer>
          </div>
          <div
            className="nt-provider-card__face nt-provider-card__back"
            aria-hidden={!flipped}
            inert={!flipped ? true : undefined}
          >
            <header className="nt-provider-card__back-head">
              <ProviderBrandIcon iconKey={visual.iconKey} fallback={visual.iconLabel} title={section.vendorLabel} />
              <div className="nt-provider-card__back-title">
                <strong>{providerAccessLabel(section)}</strong>
              </div>
              <div className="nt-provider-card__head-actions">
                <IconButton variant="quiet" compact
                  title={
                    accountLibraryExpanded
                      ? t("收起账号库", "Hide account library")
                      : t("显示账号库", "Show account library")
                  }
                  label={
                    accountLibraryExpanded
                      ? t(`收起 ${section.providerLabel} 账号库`, `Hide ${section.providerLabel} account library`)
                      : t(`显示 ${section.providerLabel} 账号库`, `Show ${section.providerLabel} account library`)
                  }
                  aria-expanded={accountLibraryExpanded}
                  aria-controls={accountLibraryId}
                  onClick={() => toggleProviderAccountLibrary(section)}
                >
                  <Database size={15} aria-hidden="true" />
                </IconButton>
                <IconButton variant="quiet" compact className="nt-provider-card__flip"
                  title={t(
                    `翻回 ${section.providerLabel} 卡牌正面`,
                    `Flip ${section.providerLabel} back to the front`,
                  )}
                  label={t(
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
                </IconButton>
              </div>
            </header>
            <section
              className="nt-provider-card__back-body"
              aria-label={t(
                `${section.providerLabel} 账号生命周期`,
                `${section.providerLabel} credential lifecycle`,
              )}
            >
              {backMounted ? children() : null}
            </section>
          </div>
        </div>
      </article>
    </div>
  );
}
