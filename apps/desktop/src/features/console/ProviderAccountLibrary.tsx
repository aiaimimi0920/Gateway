import { Database, UserPlus } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Tab, TabList, TabPanel, TabsRoot } from "@neuro/ui";

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
          <Button compact wrap
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
          </Button>
        </div>
      </header>
      {libraries.length > 0 ? (
        <TabsRoot className="nt-provider-account-library__groups" value={activeLibrary?.name ?? ""} onValueChange={setSelectedLibrary}>
          <TabList label={t("账号子分组", "Account subgroups")}>
            {libraries.map(library => (
              <Tab key={library.name} value={library.name}>
                {library.name} <span>{library.accounts.length}</span>
              </Tab>
            ))}
          </TabList>
          {activeLibrary ? (
            <TabPanel value={activeLibrary.name}>
              <AccountLibraryPager t={t} libraryKey={`${section.providerId}:${activeLibrary.name}`}
                accounts={activeLibrary.accounts} maxAccountCount={maxAccountCount} renderAccount={renderAccount} />
            </TabPanel>
          ) : null}
        </TabsRoot>
      ) : (
        <AccountLibraryPager t={t} libraryKey={section.providerId}
          accounts={[]} maxAccountCount={0} renderAccount={renderAccount} />
      )}
    </section>
  );
}
