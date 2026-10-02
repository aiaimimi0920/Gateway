import { useConsoleController } from "./useConsoleController";

import { buildConsoleRouteFeedback } from "./consoleRouteFeedback";
import {
  buildConsoleNavigation,
  ROUTE_CONFIG_INDEPENDENT_WORKSPACES,
  type ConsoleWorkspaceId,
} from "./consoleNavigation";
import { ConsoleWorkspaceHeaderActions } from "./ConsoleWorkspaceHeaderActions";
import { ConsoleIndependentWorkspace } from "./ConsoleIndependentWorkspace";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import { ConsoleModelWorkspace } from "./ConsoleModelWorkspace";
import { type ConsoleApi } from "../../api/console";
import { NeuroTooltipProvider } from "../../components/ActionTooltip";
import { AppShell } from "../shell/AppShell";
import {
  AccountsLedgerWorkspace,
  type AccountsLedgerPilotAccount,
} from "./AccountsLedgerWorkspace";
import { CredentialGroupsWorkspace } from "./CredentialGroupsWorkspace";
import { CredentialRemoveDialog } from "./ProviderAccountCard";

export type BrowserConsoleAppProps = {
  consoleApi?: ConsoleApi;
};





































































export function BrowserConsoleApp(props: BrowserConsoleAppProps) {
  return (
    <NeuroTooltipProvider>
      <BrowserConsoleContent {...props} />
    </NeuroTooltipProvider>
  );
}

function BrowserConsoleContent({ consoleApi }: BrowserConsoleAppProps) {
  const controller = useConsoleController(consoleApi);
  const {
    access,
    accountGroupSummaryError,
    accountLedgerGroupOptions,
    accountLedgerProviderOptions,
    accountPilotSections,
    actionBusy,
    activeRouteDiagnostics,
    activeWorkspace,
    addAccountGroupRow,
    autosavePending,
    busy,
    consoleRailCollapsed,
    credentialArchivePurgeBusy,
    credentialPoolAutomationByProvider,
    credentialPoolAutomationError,
    credentialPoolPruneBusy,
    credentialRefillBusy,
    credentialRefillByProvider,
    credentialRefillError,
    deleteModelPoolModel,
    displayedAccountCatalog,
    draftDirty,
    draftDocumentState,
    editorLocked,
    effectiveSelectedAccountGroupRowId,
    error,
    filteredAccountLedgerRows,
    groupDirectory,
    groupMemberMode,
    groupMemberQuery,
    handleAddIdentityCategory,
    handlePruneCredentialPool,
    handlePurgeCredentialArchive,
    handleRequestCredentialRefill,
    modelMappingCountByProvider,
    modelPoolDialog,
    modelPoolDialogProviderOptions,
    modelPoolDirectory,
    modelPoolModelNames,
    mutationSupported,
    openAddCredentialDialog,
    openAddModelDialog,
    openDuplicateCredentialDialog,
    openEditCredentialDialog,
    openEditModelDialog,
    openGeminiManualAddDialog,
    openPilotProbeDialog,
    openPilotStatsDialog,
    openProviderProbeDialog,
    openProviderScheduleDialog,
    operations,
    pendingGroupAccountRemoval,
    pendingModelPoolRemoval,
    pilotAccountsById,
    providerCredentialInventoryError,
    refresh,
    removeAccountGroupRow,
    removeCredential,
    reorderModelPoolChain,
    resetModelPoolChain,
    routeConfig,
    selectedAccountGroupDraft,
    selectedGroupBillingInvalid,
    selectedGroupIdInvalid,
    selectedGroupMemberCandidates,
    selectedGroupMembers,
    setAccountRoutingGroup,
    setActiveWorkspace,
    setConsoleRailCollapsed,
    setGroupMemberMode,
    setGroupMemberQuery,
    setModelPoolDialog,
    setPendingGroupAccountRemoval,
    setPendingModelPoolRemoval,
    setProviderCatalogDialogOpen,
    setProviderModelMappingProviderId,
    setSelectedAccountGroupRowId,
    submitModelPoolDialog,
    t,
    telemetryError,
    toggleAccountGroupMember,
    toggleModelPoolEnabled,
    togglePilotCredentialDispatch,
    updateAccountGroupEnabled,
    updateAccountGroupRow,
    updateIdentityCategoryAutomationToggle,
    updatePilotIdentityCategoryPolicy,
    updatePilotProviderPolicy,
    updateProviderAutomationToggle,
    updateProviderStoragePassword,
    updateProviderStoragePath,
  } = controller;
  const { workspaceItems, utilityItems, activeWorkspaceLabel } =
    buildConsoleNavigation(activeWorkspace, t);
  const { activeDiagnosticsFeedback, accountSummaryFeedback, draftStructureNotice } =
    buildConsoleRouteFeedback({
      activeRouteDiagnostics,
      accountGroupSummaryError,
      draftDocumentState,
      t,
    });
  const accountsWorkspace = (
    <AccountsLedgerWorkspace
      t={t}
      notice={
        <>
          {draftStructureNotice}
          {providerCredentialInventoryError ? (
            <div className="nt-alert nt-alert--warning" role="status">
              <span>
                {t(
                  `账号库与额度状态暂时不可用：${providerCredentialInventoryError}`,
                  `Account library and quota status is temporarily unavailable: ${providerCredentialInventoryError}`,
                )}
              </span>
            </div>
          ) : null}
          {credentialPoolAutomationError ? (
            <div className="nt-alert nt-alert--warning" role="status">
              <span>
                {t(
                  `自动补号状态暂时不可用：${credentialPoolAutomationError}`,
                  `Pool automation status is temporarily unavailable: ${credentialPoolAutomationError}`,
                )}
              </span>
            </div>
          ) : null}
          {credentialRefillError ? (
            <div className="nt-alert nt-alert--warning" role="status">
              <span>
                {t(
                  `补号任务状态暂时不可用：${credentialRefillError}`,
                  `Credential refill task status is temporarily unavailable: ${credentialRefillError}`,
                )}
              </span>
            </div>
          ) : null}
          {telemetryError ? (
            <div className="nt-alert nt-alert--warning" role="status">
              <span>
                {t(
                  `实时并发、费用与成功率暂时不可用，卡片上的相关数字会显示为“—”：${telemetryError}`,
                  `Live concurrency, cost and success-rate telemetry is temporarily unavailable; those card numbers render as "—": ${telemetryError}`,
                )}
              </span>
            </div>
          ) : null}
        </>
      }
      editorLocked={editorLocked}
      totalAccounts={displayedAccountCatalog.accounts.length}
      visibleCount={filteredAccountLedgerRows.length}
      rows={filteredAccountLedgerRows}
      groupOptions={accountLedgerGroupOptions}
      pilotSections={accountPilotSections}
      automationByProvider={credentialPoolAutomationByProvider}
      pruneBusyProviderId={credentialPoolPruneBusy}
      refillByProvider={credentialRefillByProvider}
      refillBusyProviderId={credentialRefillBusy}
      archivePurgeBusyProviderId={credentialArchivePurgeBusy}
      onAddIdentityCategory={handleAddIdentityCategory}
      onOpenGeminiManualAdd={openGeminiManualAddDialog}
      onEdit={openEditCredentialDialog}
      onRemove={removeCredential}
      onAddExplicit={(providerId) => openAddCredentialDialog(providerId)}
      onToggleDispatch={togglePilotCredentialDispatch}
      onUpdatePoolTargetSize={(providerId, categoryId, nextTargetSize) =>
        updatePilotIdentityCategoryPolicy(providerId, categoryId, {
          poolTargetSize: nextTargetSize,
        })
      }
      onToggleAutoRefill={(providerId, categoryId, nextEnabled) =>
        updateIdentityCategoryAutomationToggle(
          providerId,
          categoryId,
          "autoRefillEnabled",
          nextEnabled,
        )
      }
      onToggleAutoPrune={(providerId, categoryId, nextEnabled) =>
        updateIdentityCategoryAutomationToggle(
          providerId,
          categoryId,
          "autoPruneEnabled",
          nextEnabled,
        )
      }
      discardLifecycleDrafts={!mutationSupported}
      lifecycleActionsLocked={draftDirty}
      onUpdateProviderPoolMinSize={(providerId, poolMinSize) => updatePilotProviderPolicy(providerId, { poolMinSize })}
      onUpdateProviderPoolTargetSize={(providerId, nextTargetSize) =>
        updatePilotProviderPolicy(providerId, {
          poolTargetSize: nextTargetSize,
        })
      }
      onToggleProviderAutoRefill={(providerId, nextEnabled) =>
        updateProviderAutomationToggle(providerId, "autoRefillEnabled", nextEnabled)
      }
      onToggleProviderAutoPrune={(providerId, nextEnabled) =>
        updateProviderAutomationToggle(providerId, "autoPruneEnabled", nextEnabled)
      }
      onToggleProviderPermanentDelete={(providerId, nextEnabled) =>
        updatePilotProviderPolicy(providerId, { permanentDeleteEnabled: nextEnabled })
      }
      onUpdateProviderStoragePath={(providerId, path) => updateProviderStoragePath(providerId, "credential_storage_path", path)}
      onUpdateProviderArchivePath={(providerId, path) => updateProviderStoragePath(providerId, "credential_archive_path", path)}
      onUpdateProviderStoragePassword={updateProviderStoragePassword}
      onRequestProviderRefill={(providerId) => void handleRequestCredentialRefill(providerId)}
      onPruneProviderCredentials={(providerId) => void handlePruneCredentialPool(providerId)}
      onPurgeProviderArchive={(providerId) => void handlePurgeCredentialArchive(providerId)}
      onSetAccountGroup={setAccountRoutingGroup}
      onOpenProbe={openPilotProbeDialog}
      onOpenProviderProbe={openProviderProbeDialog}
      onOpenProviderSchedule={openProviderScheduleDialog}
      modelMappingCountByProvider={modelMappingCountByProvider}
      onOpenModelMapping={(section) => setProviderModelMappingProviderId(section.providerId)}
      onOpenStats={openPilotStatsDialog}
      onDuplicate={openDuplicateCredentialDialog}
    />
  );
  // Shared by the entitlement and model pool pages: both mount the credential
  // pool's account cards, so both hand them the same handlers.
  const entitlementAccountCardBridge = {
    accountsById: pilotAccountsById,
    groupOptions: accountLedgerGroupOptions,
    handlers: {
      onEdit: openEditCredentialDialog,
      onRequestRemoval: (providerId: string, accountId: string, displayName: string) =>
        setPendingGroupAccountRemoval({ providerId, accountId, displayName }),
      onAddExplicit: openAddCredentialDialog,
      onToggleDispatch: togglePilotCredentialDispatch,
      onSetAccountGroup: setAccountRoutingGroup,
      onOpenStats: openPilotStatsDialog,
    },
    onMenuAction: (
      action: string,
      providerId: string,
      account: AccountsLedgerPilotAccount,
    ) => {
      if (action === "probe") {
        openPilotProbeDialog(providerId, account);
        return;
      }
      openDuplicateCredentialDialog(providerId, account);
    },
  };
  // The pages that reuse the pool's account cards also have to reuse the pool's
  // delete confirmation instead of deleting outright.
  const groupAccountRemovalDialog = (
    <CredentialRemoveDialog
      t={t}
      pending={pendingGroupAccountRemoval}
      onCancel={() => setPendingGroupAccountRemoval(null)}
      onConfirm={() => {
        if (!pendingGroupAccountRemoval) {
          return;
        }
        removeCredential(
          pendingGroupAccountRemoval.providerId,
          pendingGroupAccountRemoval.accountId,
          pendingGroupAccountRemoval.displayName,
        );
        setPendingGroupAccountRemoval(null);
      }}
    />
  );
  const groupsWorkspace = (
    <>
      <CredentialGroupsWorkspace
        t={t}
        notice={draftStructureNotice}
        editorLocked={editorLocked}
        groups={groupDirectory}
        selectedGroupRowId={effectiveSelectedAccountGroupRowId}
        selectedGroup={selectedAccountGroupDraft}
        selectedGroupIdInvalid={selectedGroupIdInvalid}
        selectedGroupBillingInvalid={selectedGroupBillingInvalid}
        selectedGroupMembers={selectedGroupMembers}
        memberCandidates={selectedGroupMemberCandidates}
        memberQuery={groupMemberQuery}
        memberMode={groupMemberMode}
        accountCards={entitlementAccountCardBridge}
        onSelectGroup={setSelectedAccountGroupRowId}
        onAddGroup={addAccountGroupRow}
        onUpdateField={updateAccountGroupRow}
        onToggleEnabled={updateAccountGroupEnabled}
        onRemoveGroup={removeAccountGroupRow}
        onMemberQueryChange={setGroupMemberQuery}
        onMemberModeChange={setGroupMemberMode}
        onToggleMember={toggleAccountGroupMember}
      />
      {groupAccountRemovalDialog}
    </>
  );
  const modelsWorkspace = (
    <ConsoleModelWorkspace
      t={t}
      draftStructureNotice={draftStructureNotice}
      editorLocked={editorLocked}
      modelPoolDirectory={modelPoolDirectory}
      entitlementAccountCardBridge={entitlementAccountCardBridge}
      reorderModelPoolChain={reorderModelPoolChain}
      resetModelPoolChain={resetModelPoolChain}
      toggleModelPoolEnabled={toggleModelPoolEnabled}
      openEditModelDialog={openEditModelDialog}
      setPendingModelPoolRemoval={setPendingModelPoolRemoval}
      modelPoolDialog={modelPoolDialog}
      modelPoolDialogProviderOptions={modelPoolDialogProviderOptions}
      modelPoolModelNames={modelPoolModelNames}
      setModelPoolDialog={setModelPoolDialog}
      submitModelPoolDialog={submitModelPoolDialog}
      pendingModelPoolRemoval={pendingModelPoolRemoval}
      deleteModelPoolModel={deleteModelPoolModel}
      groupAccountRemovalDialog={groupAccountRemovalDialog}
    />
  );

  const workspaceContent =
    activeWorkspace === "accounts"
      ? accountsWorkspace
      : activeWorkspace === "groups"
        ? groupsWorkspace
        : activeWorkspace === "models"
          ? modelsWorkspace
          : <ConsoleIndependentWorkspace
              activeWorkspace={activeWorkspace}
              operations={operations}
              access={access}
              editorLocked={editorLocked}
              t={t}
            />;
  // These pages read from Postgres/Redis instead of the route draft, so they
  // must not sit behind the `routeConfig` gate below.
  const routeConfigIndependent = ROUTE_CONFIG_INDEPENDENT_WORKSPACES.includes(activeWorkspace);
  const workspaceHeaderActions = activeWorkspace === "accounts" ||
    activeWorkspace === "groups" || activeWorkspace === "models" ? (
    <ConsoleWorkspaceHeaderActions
      activeWorkspace={activeWorkspace}
      actionBusy={actionBusy}
      autosavePending={autosavePending}
      draftDirty={draftDirty}
      editorLocked={editorLocked}
      accountLedgerProviderOptions={accountLedgerProviderOptions}
      modelPoolDialogProviderOptions={modelPoolDialogProviderOptions}
      setProviderCatalogDialogOpen={setProviderCatalogDialogOpen}
      openAddCredentialDialog={openAddCredentialDialog}
      addAccountGroupRow={addAccountGroupRow}
      openAddModelDialog={openAddModelDialog}
      t={t}
    />
  ) : null;

  return (
    <>
      <AppShell<ConsoleWorkspaceId>
        productName="Gateway"
        navLabel="Gateway console navigation"
        navItems={workspaceItems}
        utilityItems={utilityItems}
        activeNavId={activeWorkspace}
        onNavigate={setActiveWorkspace}
        railCollapsed={consoleRailCollapsed}
        onToggleRailCollapsed={() => setConsoleRailCollapsed((current) => !current)}
        title={activeWorkspaceLabel}
        headerActions={workspaceHeaderActions}
        onRefresh={() => void refresh()}
        refreshBusy={busy}
        showLocaleToggle
        shellClassName={`nt-shell--console${consoleRailCollapsed ? " nt-shell--console-rail-collapsed" : ""}`}
        stageClassName="nt-stage--console"
        beforeStage={
          <>
            {error ? (
              <div className="nt-alert nt-alert--danger" role="alert">
                <span>{error}</span>
              </div>
            ) : null}
          </>
        }
      >
        {routeConfigIndependent ? (
          workspaceContent
        ) : busy && !routeConfig ? (
          // Keep navigation available when route storage is slow or unavailable.
          <div role="status">{t("正在加载 Gateway 控制台...", "Loading Gateway console...")}</div>
        ) : routeConfig ? (
          <>
            {activeDiagnosticsFeedback}
            {accountSummaryFeedback}
            <section className="nt-workspace-ops">
              {!mutationSupported ? (
                <div className="nt-validation-list nt-validation-list--warning">
                  <strong>{t("当前实例是只读模式", "This instance is read only")}</strong>
                  <ul>
                    <li>
                      {t(
                        "route-config runtime 没有启用写入支持，浏览器控制台暂时只能查看。",
                        "Route-config mutation support is disabled, so this browser console is currently view-only.",
                      )}
                    </li>
                  </ul>
                </div>
              ) : null}
            </section>

            {workspaceContent}
          </>
        ) : null}
      </AppShell>
      <ConsoleGlobalDialogs controller={controller} />
    </>
  );
}
