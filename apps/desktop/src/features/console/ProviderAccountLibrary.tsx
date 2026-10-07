import { Database, UserPlus } from "lucide-react";
import { useState, type ReactNode } from "react";

import { AccountLibraryPager } from "./AccountLibraryPager";
import { CHATGPT_POOL_CATEGORIES, isChatgptPool } from "./chatgptPool";
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
  const libraries = isChatgptPool(section.providerPreset, section.protocolProfile)
    ? CHATGPT_POOL_CATEGORIES.map((category) => ({
        name: category.label,
        accounts: section.identityCategories.find((entry) => entry.id === category.id)?.accounts ?? [],
      }))
    : groupAccountsByLibrary(providerAccounts);
  const [selectedLibrary, setSelectedLibrary] = useState<string | null>(null);
  const activeLibrary = libraries.find((library) => library.name === selectedLibrary) ?? libraries[0];
  const maxAccountCount = libraries.reduce((maximum, library) => Math.max(maximum, library.accounts.length), 0);
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
          <div className="nt-provider-account-library__tabs" role="tablist" aria-label={t("账号子分组", "Account subgroups")}>
            {libraries.map((library, index) => (
              <button key={library.name} id={`${accountLibraryId}-tab-${index}`} type="button" role="tab"
                aria-selected={library.name === activeLibrary?.name}
                aria-controls={`${accountLibraryId}-panel`}
                tabIndex={library.name === activeLibrary?.name ? 0 : -1}
                onClick={() => setSelectedLibrary(library.name)}
                onKeyDown={(event) => {
                  let next = index;
                  if (event.key === "ArrowRight") next = (index + 1) % libraries.length;
                  else if (event.key === "ArrowLeft") next = (index + libraries.length - 1) % libraries.length;
                  else if (event.key === "Home") next = 0;
                  else if (event.key === "End") next = libraries.length - 1;
                  else return;
                  event.preventDefault();
                  setSelectedLibrary(libraries[next].name);
                  document.getElementById(`${accountLibraryId}-tab-${next}`)?.focus();
                }}>
                {library.name} <span>{library.accounts.length}</span>
              </button>
            ))}
          </div>
          {activeLibrary ? (
            <section id={`${accountLibraryId}-panel`} role="tabpanel"
              aria-labelledby={`${accountLibraryId}-tab-${libraries.indexOf(activeLibrary)}`}>
              <AccountLibraryPager t={t} libraryKey={`${section.providerId}:${activeLibrary.name}`}
                accounts={activeLibrary.accounts} maxAccountCount={maxAccountCount} renderAccount={renderAccount} />
            </section>
          ) : null}
        </div>
      ) : (
        <AccountLibraryPager t={t} libraryKey={section.providerId}
          accounts={[]} maxAccountCount={0} renderAccount={renderAccount} />
      )}
    </section>
  );
}
