import { Database, UserPlus } from "lucide-react";
import type { ReactNode } from "react";

import { AccountLibraryPager } from "./AccountLibraryPager";
import type { AccountsLedgerPilotAccount } from "./accountCardTypes";
import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";

type ProviderAccountLibraryProps = Pick<AccountsLedgerWorkspaceProps,
  "t" | "editorLocked" | "onOpenGeminiManualAdd" | "onAddExplicit"
> & {
  section: AccountsLedgerPilotSection;
  providerAccounts: readonly AccountsLedgerPilotAccount[];
  renderAccount: (account: AccountsLedgerPilotAccount) => ReactNode;
};

function groupAccountsByLibrary(accounts: readonly AccountsLedgerPilotAccount[]) {
  const libraries = new Map<string, AccountsLedgerPilotAccount[]>();
  accounts.forEach((account) => {
    const libraryName = account.libraryName?.trim() || "default";
    const libraryAccounts = libraries.get(libraryName) ?? [];
    libraryAccounts.push(account);
    libraries.set(libraryName, libraryAccounts);
  });
  return [...libraries.entries()]
    .map(([name, libraryAccounts]) => ({ name, accounts: libraryAccounts }))
    .sort((left, right) => left.name.localeCompare(right.name));
}

// Library names partition source accounts independently from routing groups.
export function ProviderAccountLibrary({
  section,
  providerAccounts,
  t,
  editorLocked,
  onOpenGeminiManualAdd,
  onAddExplicit,
  renderAccount,
}: ProviderAccountLibraryProps) {
  const libraries = groupAccountsByLibrary(providerAccounts);
  const accountLibraryId = `provider-account-library-${section.providerId.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
  return (
    <section
      id={accountLibraryId}
      className="nt-provider-account-library nt-provider-account-library--attached"
      role="region"
      aria-label={t(
        `${section.providerLabel} 账号库`,
        `${section.providerLabel} account library`,
      )}
    >
      <header className="nt-provider-account-library__head">
        <span>
          <Database size={14} aria-hidden="true" />
          <strong>
            {t(
              `${section.providerLabel} 账号库`,
              `${section.providerLabel} account libraries`,
            )}
          </strong>
        </span>
        <div className="nt-provider-account-library__head-actions">
          <span className="nt-provider-account-library__count">{providerAccounts.length}</span>
          <button
            className="nt-btn nt-btn--secondary nt-btn--compact"
            type="button"
            disabled={editorLocked}
            onClick={() => {
              if (section.manualAddFamily) {
                onOpenGeminiManualAdd(section.manualAddFamily, section.providerId);
                return;
              }
              onAddExplicit(section.providerId);
            }}
          >
            <UserPlus size={14} aria-hidden="true" />
            {section.manualAddFamily
              ? t("手动添加", "Add manually")
              : t("手动录入账号", "Add account manually")}
          </button>
        </div>
      </header>
      {libraries.length > 0 ? (
        <div className="nt-provider-account-library__groups">
          {libraries.map((library) => (
            <section
              className="nt-provider-account-library__group"
              aria-label={t(`${library.name} 账号库`, `${library.name} account library`)}
              key={library.name}
            >
              <header className="nt-provider-account-library__group-head">
                <strong>{library.name}</strong>
                <span>{t(`${library.accounts.length} 个账号`, `${library.accounts.length} accounts`)}</span>
              </header>
              <AccountLibraryPager
                t={t}
                libraryKey={`${section.providerId}:${library.name}`}
                accounts={library.accounts}
                renderAccount={renderAccount}
              />
            </section>
          ))}
        </div>
      ) : (
        <div className="nt-provider-account-library__empty">
          <span>{t("当前账号库为空", "Account library is empty")}</span>
        </div>
      )}
    </section>
  );
}
