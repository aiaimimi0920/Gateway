import { Database } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";

import type { EntitlementAccountCardBridge } from "./credentialGroupsWorkspaceTypes";
import type { ModelPoolProviderLink } from "./modelPoolViewModel";
import {
  ACCOUNT_CARD_MENU_ITEMS,
  AccountLibraryPager,
  ProviderAccountCard,
  type AccountsLedgerPilotAccount,
} from "./ProviderAccountCard";

type TranslateFn = (zh: string, en: string) => string;

export type ModelPoolServingAccountsPanelProps = {
  t: TranslateFn;
  model: string;
  accountPanelId: string;
  selectedLinks: readonly ModelPoolProviderLink[];
  scopedAccountIds: readonly string[];
  chainNarrowed: boolean;
  accountCards?: EntitlementAccountCardBridge;
  editorLocked: boolean;
  activeMenuKey: string | null;
  onActiveMenuKeyChange: Dispatch<SetStateAction<string | null>>;
  registerMenuTrigger: (key: string, node: HTMLButtonElement | null) => void;
};

/** Renders the account library attached to an expanded model card. */
export function ModelPoolServingAccountsPanel({
  t,
  model,
  accountPanelId,
  selectedLinks,
  scopedAccountIds,
  chainNarrowed,
  accountCards,
  editorLocked,
  activeMenuKey,
  onActiveMenuKeyChange,
  registerMenuTrigger,
}: ModelPoolServingAccountsPanelProps) {
  const bridge = accountCards ?? null;
  const resolvedAccounts = bridge
    ? scopedAccountIds
        .map((accountId) => bridge.accountsById.get(accountId))
        .filter((account): account is AccountsLedgerPilotAccount => account != null)
    : [];

  return (
    <section
      className="nt-provider-account-library nt-provider-account-library--attached"
      id={accountPanelId}
      role="region"
      aria-label={t(`${model} 可用账号`, `${model} serving accounts`)}
    >
      <header className="nt-provider-account-library__head">
        <span>
          <Database size={14} aria-hidden="true" />
          <strong>{t(`${model} 账号库`, `${model} account library`)}</strong>
          {chainNarrowed ? (
            <span className="nt-entitlement-group-card__accounts-scope">
              {t(
                `已按 ${selectedLinks.length} 家服务商筛选`,
                `Filtered to ${selectedLinks.length} providers`,
              )}
            </span>
          ) : null}
        </span>
        <span className="nt-provider-account-library__count">
          {scopedAccountIds.length}
        </span>
      </header>
      {bridge && resolvedAccounts.length > 0 ? (
        <AccountLibraryPager
          t={t}
          libraryKey={`model-pool:${model}:${selectedLinks
            .map((link) => link.providerId)
            .join(",")}`}
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
                keyPrefix: `model-pool:${model}`,
                activeKey: activeMenuKey,
                onActiveKeyChange: onActiveMenuKeyChange,
                registerTrigger: registerMenuTrigger,
                items: bridge.menuItems ?? ACCOUNT_CARD_MENU_ITEMS,
                onAction: bridge.onMenuAction,
              }}
            />
          )}
        />
      ) : (
        <div className="nt-provider-account-library__pager">
          <div className="nt-provider-account-library__empty">
            {chainNarrowed
              ? t(
                  "所选服务商范围内没有账号。",
                  "No accounts fall inside the selected provider scope.",
                )
              : t("这个模型还没有可用账号。", "No account serves this model yet.")}
          </div>
        </div>
      )}
    </section>
  );
}
