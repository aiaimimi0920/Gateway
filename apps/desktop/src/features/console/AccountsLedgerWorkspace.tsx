import { useMemo, useState } from "react";
import { EmptyState } from "@neuro/ui";

import { CredentialDirectory } from "./CredentialDirectory";
import { PROVIDER_ACCESS_KINDS } from "./providerAccessKind";
import { ProviderRemoveDialog } from "./ProviderRemoveDialog";
import {
  ProviderLifecycleActionDialog,
  type PendingProviderLifecycleAction,
} from "./ProviderLifecycleActionDialog";
import { ProviderLifecycleBack, type ProviderLifecycleBackOptions } from "./ProviderLifecycleBack";
import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import { AccountsLedgerTable } from "./AccountsLedgerTable";
import {
  CredentialRemoveDialog,
  ProviderAccountCard,
  type AccountCardActionId,
  type AccountsLedgerPilotAccount,
  type PendingCredentialRemoval,
} from "./ProviderAccountCard";
import { providerPoolEntries, providerPoolTableRows } from "./providerPoolView";

export type {
  AccountsLedgerPilotAccount,
  AccountsLedgerPilotCategory,
  AccountsLedgerPilotSection,
  AccountsLedgerWorkspaceProps,
} from "./accountsLedgerTypes";



export function AccountsLedgerWorkspace(props: AccountsLedgerWorkspaceProps) {
  const {
    t,
    notice,
    editorLocked,
    totalAccounts,
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
    onUpdateProviderPoolMinSize,
    onUpdateProviderStoragePath,
    onUpdateProviderArchivePath,
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
    onRefreshDiscovery,
  } = props;
  const poolView = props.poolView ?? { query: "", sort: "default" as const };
  const providerEntries = useMemo(
    () => providerPoolEntries(pilotSections, poolView),
    [pilotSections, poolView.query, poolView.sort, poolView.accessKinds],
  );
  const hasAccounts = totalAccounts > 0;
  const hasSiteFilter = Boolean(poolView.query.trim()) || (poolView.accessKinds?.length ?? PROVIDER_ACCESS_KINDS.length) < PROVIDER_ACCESS_KINDS.length;
  const pilotProviderIds = new Set(pilotSections.flatMap((section) => section.providerIds));
  const tableRows = providerPoolTableRows(rows.filter((row) => !pilotProviderIds.has(row.providerId)), poolView);
  const hasVisibleLedgerContent = providerEntries.length > 0 || tableRows.length > 0;
  const [pendingCredentialRemoval, setPendingCredentialRemoval] =
    useState<PendingCredentialRemoval | null>(null);
  const [pendingProviderRemoval, setPendingProviderRemoval] = useState<AccountsLedgerPilotSection | null>(null);
  const [pendingProviderLifecycleAction, setPendingProviderLifecycleAction] =
    useState<PendingProviderLifecycleAction | null>(null);

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
    if (props.lifecycleActionsLocked && action.kind !== "enable-permanent-delete") return;
    if (action.kind === "prune") { onPruneProviderCredentials(action.providerId); return; }
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

  const handleAccountAction = (
    action: AccountCardActionId,
    providerId: string,
    account: AccountsLedgerPilotAccount,
  ) => {
    switch (action) {
      case "discover":
        onRefreshDiscovery?.(providerId, account.accountId);
        break;
      case "probe":
        onOpenProbe(providerId, account);
        break;
      case "model-mapping": {
        const section = pilotSections.find((entry) => entry.providerId === providerId);
        if (section) onOpenModelMapping(section);
        break;
      }
      case "duplicate":
        onDuplicate(providerId, account);
        break;
    }
  };

  const renderProviderLifecycleBack = (options: ProviderLifecycleBackOptions) => (
    <ProviderLifecycleBack
      options={options}
      settings={props.renderProviderSettings?.(options.section)}
      storageSecretFields={props.storageSecretFields}
      onUpdateProviderStorageConnection={props.onUpdateProviderStorageConnection}
      lifecycleActionsLocked={props.lifecycleActionsLocked}
      editorLocked={editorLocked}
      pruneBusyProviderId={pruneBusyProviderId}
      refillBusyProviderId={refillBusyProviderId}
      archivePurgeBusyProviderId={archivePurgeBusyProviderId}
      onUpdateProviderPoolTargetSize={onUpdateProviderPoolTargetSize}
      onUpdateProviderPoolMinSize={onUpdateProviderPoolMinSize}
      onUpdateProviderRequestClientProfile={props.onUpdateProviderRequestClientProfile}
      onUpdateProviderStoragePath={onUpdateProviderStoragePath}
      onUpdateProviderArchivePath={onUpdateProviderArchivePath}
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
      onAction={handleAccountAction}
    />
  );

  return (
    <div className="nt-stack">
      {notice}
      <CredentialDirectory entries={providerEntries} props={props}
        renderAccount={renderProviderAccountCard} onRemove={setPendingProviderRemoval}
        renderSettings={({ section, snapshot }) => renderProviderLifecycleBack({
          section, active: true, discardEdits: props.discardLifecycleDrafts,
          availableCount: snapshot.availablePoolCount,
          coolingCount: snapshot.poolSegments.rateLimited,
          invalidCount: snapshot.poolSegments.invalid,
          automation: automationByProvider.get(section.providerId),
          refill: refillByProvider.get(section.providerId),
        })} />
      <AccountsLedgerTable
        t={t}
        editorLocked={editorLocked}
        rows={tableRows}
        onEdit={onEdit}
        onAddExplicit={onAddExplicit}
      />
      {!hasAccounts && !hasVisibleLedgerContent && !hasSiteFilter ? (
        <EmptyState title={t("暂无账号", "No accounts yet")} />
      ) : null}

      {(hasAccounts || hasSiteFilter) && !hasVisibleLedgerContent ? (
        <EmptyState title={hasSiteFilter ? t("没有匹配站点", "No matching sites") : t("没有匹配账号", "No matching accounts")} />
      ) : null}

      <CredentialRemoveDialog
        t={t}
        pending={pendingCredentialRemoval}
        onCancel={() => setPendingCredentialRemoval(null)}
        onConfirm={confirmCredentialRemoval}
      />
      <ProviderRemoveDialog pending={pendingProviderRemoval} locked={editorLocked}
        onCancel={() => setPendingProviderRemoval(null)}
        onConfirm={() => {
          if (pendingProviderRemoval && !editorLocked) props.onRemoveProvider(pendingProviderRemoval.providerId);
          setPendingProviderRemoval(null);
        }} t={t} />

      <ProviderLifecycleActionDialog
        pending={pendingProviderLifecycleAction}
        onCancel={() => setPendingProviderLifecycleAction(null)}
        onConfirm={confirmProviderLifecycleAction}
        t={t}
      />
    </div>
  );
}
