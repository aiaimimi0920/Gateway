import { Fragment, useEffect, useMemo, useRef, useState } from "react";

import { ProviderAccountLibrary } from "./ProviderAccountLibrary";
import { ProviderLedgerCard } from "./ProviderLedgerCard";
import {
  ProviderLifecycleActionDialog,
  type PendingProviderLifecycleAction,
} from "./ProviderLifecycleActionDialog";
import { ProviderLifecycleBack, type ProviderLifecycleBackOptions } from "./ProviderLifecycleBack";
import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import { AccountsLedgerTable } from "./AccountsLedgerTable";
import {
  ACCOUNT_CARD_MENU_ITEMS,
  CredentialRemoveDialog,
  ProviderAccountCard,
  useAccountCardMenu,
  type AccountCardMenuActionId,
  type AccountsLedgerPilotAccount,
  type PendingCredentialRemoval,
} from "./ProviderAccountCard";
import { buildProviderCardSnapshot } from "./providerCardSnapshot";

export type {
  AccountsLedgerPilotAccount,
  AccountsLedgerPilotCategory,
  AccountsLedgerPilotSection,
  AccountsLedgerWorkspaceProps,
} from "./accountsLedgerTypes";

type PilotMenuActionId = AccountCardMenuActionId;
type ProviderCardSide = "front" | "back";

export function AccountsLedgerWorkspace(props: AccountsLedgerWorkspaceProps) {
  const {
    t,
    notice,
    editorLocked,
    totalAccounts,
    visibleCount,
    rows,
    pilotSections,
    automationByProvider,
    pruneBusyProviderId,
    refillByProvider,
    refillBusyProviderId,
    archivePurgeBusyProviderId,
    onOpenGeminiManualAdd,
    onEdit,
    onRemove,
    onAddExplicit,
    onToggleDispatch,
    onUpdateProviderPoolTargetSize,
    onToggleProviderAutoRefill,
    onToggleProviderAutoPrune,
    onToggleProviderPermanentDelete,
    onUpdateProviderStoragePassword,
    onRequestProviderRefill,
    onPruneProviderCredentials,
    onPurgeProviderArchive,
    onSetAccountGroup,
    onOpenProbe,
    onOpenProviderProbe,
    onOpenProviderSchedule,
    modelMappingCountByProvider,
    onOpenModelMapping,
    onOpenStats,
    onDuplicate,
  } = props;
  const providerCardSnapshots = useMemo(
    () => pilotSections.map((section) => buildProviderCardSnapshot(section)),
    [pilotSections],
  );
  const hasAccounts = totalAccounts > 0;
  const pilotProviderIds = new Set(pilotSections.flatMap((section) => section.providerIds));
  const tableRows = rows.filter((row) => !pilotProviderIds.has(row.providerId));
  const hasVisibleLedgerContent = visibleCount > 0;
  const [expandedAccountLibraryProviderId, setExpandedAccountLibraryProviderId] = useState<
    string | null
  >(null);
  const pilotMenu = useAccountCardMenu();
  const {
    activeMenuKey: activePilotActionMenuKey,
    setActiveMenuKey: setActivePilotActionMenuKey,
    registerMenuTrigger,
  } = pilotMenu;
  const [poolTargetDrafts, setPoolTargetDrafts] = useState<Record<string, string>>({});
  const [flippedProviderIds, setFlippedProviderIds] = useState<string[]>([]);
  const providerFlipButtonRefs = useRef(new Map<string, HTMLButtonElement>());
  const pendingFlipFocusRef = useRef<{ providerId: string; side: ProviderCardSide } | null>(null);
  const [providerFlipAnnouncement, setProviderFlipAnnouncement] = useState("");
  const [pendingCredentialRemoval, setPendingCredentialRemoval] =
    useState<PendingCredentialRemoval | null>(null);
  const [pendingProviderLifecycleAction, setPendingProviderLifecycleAction] =
    useState<PendingProviderLifecycleAction | null>(null);
  const [storagePasswordDrafts, setStoragePasswordDrafts] = useState<
    Record<string, { editing: boolean; value: string }>
  >({});

  useEffect(() => {
    const pendingFocus = pendingFlipFocusRef.current;
    if (!pendingFocus) {
      return;
    }
    pendingFlipFocusRef.current = null;
    providerFlipButtonRefs.current
      .get(`${pendingFocus.providerId}:${pendingFocus.side}`)
      ?.focus();
  }, [flippedProviderIds]);

  const toggleProviderAccountLibrary = (section: AccountsLedgerPilotSection) => {
    setExpandedAccountLibraryProviderId((current) =>
      current === section.providerId ? null : section.providerId,
    );
  };

  const toggleProviderFlip = (
    section: AccountsLedgerPilotSection,
    nextSide: ProviderCardSide,
  ) => {
    pendingFlipFocusRef.current = { providerId: section.providerId, side: nextSide };
    setProviderFlipAnnouncement(
      nextSide === "back"
        ? t(
            `${section.providerLabel} 卡牌背面已显示`,
            `${section.providerLabel} card back shown`,
          )
        : t(
            `${section.providerLabel} 卡牌正面已显示`,
            `${section.providerLabel} card front shown`,
          ),
    );
    setFlippedProviderIds((current) =>
      nextSide === "back"
        ? current.includes(section.providerId)
          ? current
          : [...current, section.providerId]
        : current.filter((id) => id !== section.providerId),
    );
  };

  const requestCredentialRemoval = (
    providerId: string,
    accountId: string,
    displayName: string,
  ) => {
    setPendingCredentialRemoval({ providerId, accountId, displayName });
  };

  const confirmCredentialRemoval = () => {
    if (!pendingCredentialRemoval) {
      return;
    }
    onRemove(
      pendingCredentialRemoval.providerId,
      pendingCredentialRemoval.accountId,
      pendingCredentialRemoval.displayName,
    );
    setPendingCredentialRemoval(null);
  };

  const requestProviderLifecycleAction = (
    action: PendingProviderLifecycleAction,
  ) => {
    setPendingProviderLifecycleAction(action);
  };

  const confirmProviderLifecycleAction = () => {
    const action = pendingProviderLifecycleAction;
    if (!action) {
      return;
    }
    if (action.kind === "enable-permanent-delete") {
      onToggleProviderPermanentDelete(action.providerId, true);
    } else if (action.kind === "prune") {
      onPruneProviderCredentials(action.providerId);
    } else {
      onPurgeProviderArchive(action.providerId);
    }
    setPendingProviderLifecycleAction(null);
  };

  const pilotMenuItems = ACCOUNT_CARD_MENU_ITEMS;

  const handlePilotMenuAction = (
    action: PilotMenuActionId,
    providerId: string,
    account: AccountsLedgerPilotAccount,
  ) => {
    setActivePilotActionMenuKey(null);
    switch (action) {
      case "probe":
        onOpenProbe(providerId, account);
        break;
      case "duplicate":
        onDuplicate(providerId, account);
        break;
    }
  };

  const commitPoolTargetDraft = (
    policyKey: string,
    committedValue: number,
    onCommit: (nextTargetSize: number) => void,
  ) => {
    const draftValue = poolTargetDrafts[policyKey];
    if (draftValue === undefined) {
      return;
    }

    const normalized = draftValue.trim();
    if (normalized.length === 0) {
      setPoolTargetDrafts((current) => {
        const next = { ...current };
        delete next[policyKey];
        return next;
      });
      return;
    }

    const parsed = Number(normalized);
    const nextTargetSize =
      Number.isFinite(parsed) && parsed >= 1 ? Math.floor(parsed) : committedValue;

    setPoolTargetDrafts((current) => {
      const next = { ...current };
      delete next[policyKey];
      return next;
    });

    if (nextTargetSize !== committedValue) {
      onCommit(nextTargetSize);
    }
  };


  const renderProviderLifecycleBack = (options: ProviderLifecycleBackOptions) => (
    <ProviderLifecycleBack
      options={options}
      poolTargetDrafts={poolTargetDrafts}
      setPoolTargetDrafts={setPoolTargetDrafts}
      commitPoolTargetDraft={commitPoolTargetDraft}
      storagePasswordDrafts={storagePasswordDrafts}
      setStoragePasswordDrafts={setStoragePasswordDrafts}
      editorLocked={editorLocked}
      pruneBusyProviderId={pruneBusyProviderId}
      refillBusyProviderId={refillBusyProviderId}
      archivePurgeBusyProviderId={archivePurgeBusyProviderId}
      onUpdateProviderPoolTargetSize={onUpdateProviderPoolTargetSize}
      onToggleProviderAutoRefill={onToggleProviderAutoRefill}
      onToggleProviderAutoPrune={onToggleProviderAutoPrune}
      onToggleProviderPermanentDelete={onToggleProviderPermanentDelete}
      onRequestProviderRefill={onRequestProviderRefill}
      onUpdateProviderStoragePassword={onUpdateProviderStoragePassword}
      requestProviderLifecycleAction={requestProviderLifecycleAction}
      t={t}
    />
  );

  const renderProviderAccountCard = (
    _section: AccountsLedgerPilotSection,
    account: AccountsLedgerPilotAccount,
  ) => (
    <ProviderAccountCard
      key={`${account.providerId}:${account.accountId}`}
      t={t}
      account={account}
      editorLocked={editorLocked}
      groupOptions={props.groupOptions}
      handlers={{
        onEdit,
        onRequestRemoval: requestCredentialRemoval,
        onAddExplicit,
        onToggleDispatch,
        onSetAccountGroup,
        onOpenStats,
      }}
      menu={{
        keyPrefix: "account-library",
        activeKey: activePilotActionMenuKey,
        onActiveKeyChange: setActivePilotActionMenuKey,
        registerTrigger: registerMenuTrigger,
        items: pilotMenuItems,
        onAction: handlePilotMenuAction,
      }}
    />
  );

  const renderProviderAccountLibrary = (
    section: AccountsLedgerPilotSection,
    providerAccounts: AccountsLedgerPilotAccount[],
  ) => (
    <ProviderAccountLibrary
      section={section}
      providerAccounts={providerAccounts}
      editorLocked={editorLocked}
      onOpenGeminiManualAdd={onOpenGeminiManualAdd}
      onAddExplicit={onAddExplicit}
      renderAccount={(account) => renderProviderAccountCard(section, account)}
      t={t}
    />
  );

  return (
    <div className="nt-stack">
      {notice}
      <p className="nt-visually-hidden" role="status" aria-live="polite" aria-atomic="true">
        {providerFlipAnnouncement}
      </p>
      <div className="nt-provider-card-grid">
        {pilotSections.map((section, sectionIndex) => {
          const accountLibraryExpanded =
            expandedAccountLibraryProviderId === section.providerId;
          const flipped = flippedProviderIds.includes(section.providerId);
          const snapshot = providerCardSnapshots[sectionIndex];
          const providerAccounts = snapshot.accounts;
          const providerAvailablePoolCount = snapshot.availablePoolCount;
          const poolSegments = snapshot.poolSegments;
          return (
            <Fragment key={section.providerId}>
              <ProviderLedgerCard
                section={section}
                snapshot={snapshot}
                t={t}
                editorLocked={editorLocked}
                accountLibraryExpanded={accountLibraryExpanded}
                flipped={flipped}
                providerModelMappingCount={modelMappingCountByProvider.get(section.providerId) ?? 0}
                toggleProviderAccountLibrary={toggleProviderAccountLibrary}
                toggleProviderFlip={toggleProviderFlip}
                providerFlipButtonRefs={providerFlipButtonRefs}
                menu={pilotMenu}
                handlers={{
                  onToggleDispatch,
                  onEdit,
                  onOpenProviderProbe,
                  onOpenProviderSchedule,
                  onOpenModelMapping,
                  requestCredentialRemoval,
                }}
              >
                {renderProviderLifecycleBack({
                  section,
                  availableCount: providerAvailablePoolCount,
                  coolingCount: poolSegments.rateLimited,
                  invalidCount: poolSegments.invalid,
                  automation: automationByProvider.get(section.providerId),
                  refill: refillByProvider.get(section.providerId),
                })}
              </ProviderLedgerCard>
              {accountLibraryExpanded
                ? renderProviderAccountLibrary(section, providerAccounts)
                : null}
            </Fragment>
          );
        })}
      </div>

      <AccountsLedgerTable
        t={t}
        editorLocked={editorLocked}
        rows={tableRows}
        onEdit={onEdit}
        onAddExplicit={onAddExplicit}
      />

      {!hasAccounts ? (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h2>{t("暂无账号", "No accounts yet")}</h2>
          <p className="nt-copy">
            {t(
              "当前 route-config 里还没有解析出 provider credential。请使用上方的“添加账号”，或在服务商账号库中手动录入账号。",
              "No provider credentials were found in the current route config. Use Add account above, or add one manually from a provider account library.",
            )}
          </p>
        </article>
      ) : null}

      {hasAccounts && !hasVisibleLedgerContent ? (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h2>{t("没有匹配账号", "No matching accounts")}</h2>
          <p className="nt-copy">
            {t(
              "当前筛选条件没有匹配任何账号，请调整关键词或账号状态。",
              "No accounts match the current filters. Adjust the query or account status.",
            )}
          </p>
        </article>
      ) : null}

      <CredentialRemoveDialog
        t={t}
        pending={pendingCredentialRemoval}
        onCancel={() => setPendingCredentialRemoval(null)}
        onConfirm={confirmCredentialRemoval}
      />

      <ProviderLifecycleActionDialog
        pending={pendingProviderLifecycleAction}
        onCancel={() => setPendingProviderLifecycleAction(null)}
        onConfirm={confirmProviderLifecycleAction}
        t={t}
      />
    </div>
  );
}
