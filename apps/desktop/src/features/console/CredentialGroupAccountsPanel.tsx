import { Database } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";

import type { GroupMemberCandidate } from "./accountManagementViewModel";
import type {
  EntitlementAccountCardBridge,
  TranslateFn,
} from "./credentialGroupsWorkspaceTypes";
import type { CredentialGroupCardSnapshot } from "./credentialGroupCardSnapshot";
import {
  ACCOUNT_CARD_MENU_ITEMS,
  AccountLibraryPager,
  ProviderAccountCard,
  type AccountsLedgerPilotAccount,
} from "./ProviderAccountCard";

type CredentialGroupAccountsPanelProps = {
  t: TranslateFn;
  snapshot: CredentialGroupCardSnapshot;
  accountsReady: boolean;
  editorLocked: boolean;
  members: GroupMemberCandidate[];
  accountCards?: EntitlementAccountCardBridge;
  activeMenuKey: string | null;
  onActiveMenuKeyChange: Dispatch<SetStateAction<string | null>>;
  registerMenuTrigger: (key: string, node: HTMLButtonElement | null) => void;
};

export function CredentialGroupAccountsPanel({
  t,
  snapshot,
  accountsReady,
  editorLocked,
  members,
  accountCards,
  activeMenuKey,
  onActiveMenuKeyChange,
  registerMenuTrigger,
}: CredentialGroupAccountsPanelProps) {
  const { group, label, accountPanelId, selectedProviderIds, providerScopeNarrowed } =
    snapshot;
  const scopedMembers = providerScopeNarrowed
    ? members.filter((candidate) => selectedProviderIds.has(candidate.providerId))
    : members;
  const bridge = accountCards ?? null;
  const resolvedAccounts =
    bridge && accountsReady
      ? scopedMembers
          .map((candidate) => bridge.accountsById.get(candidate.accountId))
          .filter((account): account is AccountsLedgerPilotAccount => account != null)
      : [];
  const accountCount = providerScopeNarrowed ? scopedMembers.length : group.memberCount;

  return (
    <section
      className="nt-entitlement-group-card__accounts nt-entitlement-group-card__accounts--attached"
      id={accountPanelId}
      role="region"
      aria-label={t(`${label} 组内账号`, `${label} group accounts`)}
    >
      <header className="nt-entitlement-group-card__accounts-head">
        <span>
          <Database size={14} aria-hidden="true" />
          <strong>{t("组内账号", "Group accounts")}</strong>
          {providerScopeNarrowed ? (
            <span className="nt-entitlement-group-card__accounts-scope">
              {t(
                `已按 ${selectedProviderIds.size} 家服务商筛选`,
                `Filtered to ${selectedProviderIds.size} providers`,
              )}
            </span>
          ) : null}
        </span>
        <span className="nt-entitlement-group-card__accounts-count">{accountCount}</span>
      </header>
      {accountsReady && bridge && resolvedAccounts.length > 0 ? (
        <AccountLibraryPager
          t={t}
          libraryKey={`entitlement-group:${group.rowId}:${[...selectedProviderIds].sort().join(",")}`}
          accounts={resolvedAccounts}
          renderAccount={(account) => (
            <ProviderAccountCard
              key={`${account.providerId}:${account.accountId}`}
              t={t}
              account={account}
              editorLocked={editorLocked}
              groupOptions={bridge.groupOptions}
              handlers={bridge.handlers}
              menu={{
                keyPrefix: `entitlement-group:${group.rowId}`,
                activeKey: activeMenuKey,
                onActiveKeyChange: onActiveMenuKeyChange,
                registerTrigger: registerMenuTrigger,
                items: bridge.menuItems ?? ACCOUNT_CARD_MENU_ITEMS,
                onAction: bridge.onMenuAction,
              }}
            />
          )}
        />
      ) : accountsReady && scopedMembers.length > 0 ? (
        <div className="nt-entitlement-account-grid">
          {scopedMembers.map((candidate) => (
            <article className="nt-entitlement-account-card" key={`${group.rowId}:${candidate.accountId}`}>
              <div className="nt-entitlement-account-card__head">
                <div>
                  <strong>{candidate.displayName}</strong>
                  <small>{candidate.accountId}</small>
                </div>
                <span className={candidate.enabled ? "nt-badge nt-badge--success" : "nt-badge nt-badge--warning"}>
                  {candidate.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}
                </span>
              </div>
              <div className="nt-entitlement-account-card__provider">
                <strong>{candidate.providerLabel}</strong>
                <span>{candidate.vendorLabel}</span>
              </div>
            </article>
          ))}
        </div>
      ) : accountsReady ? (
        <p className="nt-empty">
          {providerScopeNarrowed && members.length > 0
            ? t(
                "所选服务商范围内没有账号。",
                "No accounts fall inside the selected provider scope.",
              )
            : t(
                "这个权益组还没有账号。",
                "This entitlement group has no accounts yet.",
              )}
        </p>
      ) : null}
    </section>
  );
}
