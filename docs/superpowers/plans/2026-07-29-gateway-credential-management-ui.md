# Gateway Credential Management UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current provider-bucket-heavy credential UI with a Sub2API-inspired account ledger and credential-groups admin surface while preserving Gateway's draft-first route-config behavior.

**Architecture:** Extract the account and group derivation logic into a focused console view-model module, then split the current monolithic `BrowserConsoleApp` account/group rendering into two dedicated workspace components: a ledger-style accounts surface and a master-detail credential-groups surface. Keep all draft mutation, probe, and save behavior in `BrowserConsoleApp`, but pass the derived data and callbacks into the extracted components so the UI can be modernized without inventing a new backend contract.

**Tech Stack:** React 19, TypeScript, Testing Library, Vitest, existing `useUiLocale` bilingual copy helpers, and `apps/desktop/src/styles.css`.

---

## File Structure

Existing files to modify:

```text
apps/desktop/src/features/console/BrowserConsoleApp.tsx
  # existing state owner; keep route-config, draft, probe, dialog, and save logic here
apps/desktop/src/features/console/BrowserConsoleApp.test.tsx
  # integration coverage for account ledger and groups workspace behavior
apps/desktop/src/features/console/BrowserConsoleApp.i18n.test.tsx
  # bilingual label coverage after workspace copy changes
apps/desktop/src/styles.css
  # new admin-ledger and master-detail layout styles
```

New focused files to create:

```text
apps/desktop/src/features/console/accountManagementViewModel.ts
  # shared account-ledger rows, filters, group directory items, and member candidate builders
apps/desktop/src/features/console/accountManagementViewModel.test.ts
  # pure tests for derived rows and filters
apps/desktop/src/features/console/AccountsLedgerWorkspace.tsx
  # account ledger summary cards, toolbar, table/list rows, and empty states
apps/desktop/src/features/console/CredentialGroupsWorkspace.tsx
  # group summary cards, left-hand directory, right-hand detail form, and member manager
```

## Task 1: Extract the account and group view-model layer first

**Files:**
- Create: `apps/desktop/src/features/console/accountManagementViewModel.ts`
- Create: `apps/desktop/src/features/console/accountManagementViewModel.test.ts`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.tsx`

- [ ] **Step 1: Write failing pure tests for ledger rows, filters, and group directory items**

```ts
import { describe, expect, it } from "vitest";
import {
  buildAccountLedgerRows,
  buildCredentialGroupDirectory,
  buildGroupMemberCandidates,
  filterAccountLedgerRows,
} from "./accountManagementViewModel";

describe("accountManagementViewModel", () => {
  const catalog = {
    groups: [{ id: "group-vip", name: "VIP 分组", billingMultiplier: 1.5, enabled: true }],
    accounts: [
      {
        id: "acc-prod-1",
        displayName: "生产账号 A",
        vendorKey: "openai",
        vendorName: "OpenAI",
        providerId: "managed-provider",
        providerLabel: "Managed OpenAI",
        providerPreset: "openai",
        baseUrl: "https://api.example.com/v1",
        hostLabel: "api.example.com",
        mode: "credential",
        enabled: true,
        supportedModels: ["gpt-5.4"],
        groupIds: ["group-vip"],
        groupNames: ["VIP 分组"],
      },
      {
        id: "managed-provider::default",
        displayName: "Managed OpenAI 默认账号",
        vendorKey: "openai",
        vendorName: "OpenAI",
        providerId: "managed-provider",
        providerLabel: "Managed OpenAI",
        providerPreset: "openai",
        baseUrl: "https://api.example.com/v1",
        hostLabel: "api.example.com",
        mode: "provider-default",
        enabled: false,
        supportedModels: ["gpt-5.4-mini"],
        groupIds: [],
        groupNames: [],
      },
    ],
    providerBuckets: [],
    ungroupedCount: 1,
  };

  it("builds ledger rows with provider, groups, and probe state", () => {
    const rows = buildAccountLedgerRows(catalog, {
      "acc-prod-1": {
        credentialId: "acc-prod-1",
        providerId: "managed-provider",
        status: "passed",
        message: "Credential connectivity probe passed.",
        checkedAt: "2026-07-29T10:30:00Z",
      },
    });

    expect(rows[0]).toMatchObject({
      accountId: "acc-prod-1",
      providerLabel: "Managed OpenAI",
      vendorLabel: "OpenAI",
      groupLabels: ["VIP 分组"],
      enabled: true,
      probeStatus: "passed",
    });
    expect(rows[1]).toMatchObject({
      accountId: "managed-provider::default",
      mode: "provider-default",
      groupLabels: [],
      probeStatus: "not-tested",
    });
  });

  it("filters ledger rows by search, grouping, provider, and enabled state", () => {
    const rows = buildAccountLedgerRows(catalog, {});
    const filtered = filterAccountLedgerRows(rows, {
      query: "生产账号 A",
      membership: "grouped",
      enabled: "enabled",
      groupId: "group-vip",
      providerKey: "openai",
    });

    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.accountId).toBe("acc-prod-1");
  });

  it("builds group directory items and candidate rows without provider buckets", () => {
    const directory = buildCredentialGroupDirectory(catalog, [
      {
        id: "draft-group-vip",
        groupId: "group-vip",
        name: "VIP 分组",
        billingMultiplier: "1.5",
        description: "",
        notes: "",
        enabled: true,
        providerCredentialIds: ["acc-prod-1"],
      },
    ]);
    const candidates = buildGroupMemberCandidates(catalog.accounts, {
      selectedCredentialIds: ["acc-prod-1"],
      query: "Managed OpenAI",
      mode: "all",
    });

    expect(directory[0]).toMatchObject({
      rowId: "draft-group-vip",
      groupId: "group-vip",
      memberCount: 1,
      providerLabels: ["Managed OpenAI"],
    });
    expect(candidates[0]).toMatchObject({
      accountId: "acc-prod-1",
      selected: true,
      providerLabel: "Managed OpenAI",
    });
  });
});
```

- [ ] **Step 2: Run the focused test and confirm the missing-module failure**

Run: `npm test -- --run src/features/console/accountManagementViewModel.test.ts`  
Expected: FAIL because `accountManagementViewModel.ts` does not exist yet.

- [ ] **Step 3: Implement the extracted view-model module with exported types and helpers**

```ts
export type AccountLedgerRow = {
  accountId: string;
  displayName: string;
  providerId: string;
  providerLabel: string;
  providerPreset: string | null;
  vendorKey: string;
  vendorLabel: string;
  mode: "credential" | "provider-default";
  enabled: boolean;
  supportedModels: string[];
  groupIds: string[];
  groupLabels: string[];
  hostLabel: string | null;
  probeStatus: "not-tested" | "passed" | "failed" | "unsupported" | "error";
  probeMessage: string | null;
  probeCheckedAt: string | null;
  searchText: string;
};

export function buildAccountLedgerRows(
  catalog: RouteAccountCatalog,
  probeResults: Record<string, CredentialProbeViewResult>,
): AccountLedgerRow[] {
  return catalog.accounts.map((account) => {
    const probe = probeResults[account.id];
    return {
      accountId: account.id,
      displayName: account.displayName,
      providerId: account.providerId,
      providerLabel: account.providerLabel,
      providerPreset: account.providerPreset,
      vendorKey: account.vendorKey,
      vendorLabel: account.vendorName,
      mode: account.mode,
      enabled: account.enabled,
      supportedModels: [...account.supportedModels],
      groupIds: [...account.groupIds],
      groupLabels: [...account.groupNames],
      hostLabel: account.hostLabel,
      probeStatus: probe?.status ?? "not-tested",
      probeMessage: probe?.message ?? null,
      probeCheckedAt: probe?.checkedAt ?? null,
      searchText: [
        account.displayName,
        account.id,
        account.providerLabel,
        account.vendorName,
        ...account.supportedModels,
        ...account.groupNames,
      ]
        .join(" ")
        .toLowerCase(),
    };
  });
}

export function filterAccountLedgerRows(
  rows: AccountLedgerRow[],
  filters: {
    query: string;
    membership: AccountMembershipFilter;
    enabled: AccountEnabledFilter;
    groupId: string;
    providerKey: string;
  },
): AccountLedgerRow[] {
  const query = filters.query.trim().toLowerCase();
  return rows.filter((row) => {
    if (query.length > 0 && !row.searchText.includes(query)) return false;
    if (filters.membership === "grouped" && row.groupIds.length === 0) return false;
    if (filters.membership === "ungrouped" && row.groupIds.length > 0) return false;
    if (filters.enabled === "enabled" && !row.enabled) return false;
    if (filters.enabled === "disabled" && row.enabled) return false;
    if (filters.groupId !== "all" && !row.groupIds.includes(filters.groupId)) return false;
    if (filters.providerKey !== "all" && row.vendorKey !== filters.providerKey) return false;
    return true;
  });
}
```

Also move or re-export the account/group catalog types that both new workspace components will need so `BrowserConsoleApp.tsx` stops being the only place that knows those shapes.

- [ ] **Step 4: Wire `BrowserConsoleApp.tsx` to consume the extracted helpers without changing visible UI yet**

```ts
import {
  buildAccountLedgerRows,
  buildCredentialGroupDirectory,
  buildGroupMemberCandidates,
  filterAccountLedgerRows,
} from "./accountManagementViewModel";

const accountLedgerRows = useMemo(
  () => buildAccountLedgerRows(displayedAccountCatalog, credentialProbeResults),
  [displayedAccountCatalog, credentialProbeResults],
);

const filteredAccountLedgerRows = useMemo(
  () =>
    filterAccountLedgerRows(accountLedgerRows, {
      query: accountSearch,
      membership: accountMembershipFilter,
      enabled: accountEnabledFilter,
      groupId: selectedAccountGroupFilter,
      providerKey: selectedProviderFilter,
    }),
  [accountEnabledFilter, accountLedgerRows, accountMembershipFilter, accountSearch, selectedAccountGroupFilter, selectedProviderFilter],
);
```

Add any missing `useState` for the new filters now, but keep rendering unchanged in this task so failures are easy to localize.

- [ ] **Step 5: Re-run focused validation and typecheck**

Run: `npm test -- --run src/features/console/accountManagementViewModel.test.ts`  
Expected: PASS.

Run: `npm run typecheck`  
Expected: PASS with the new module imported into `BrowserConsoleApp.tsx`.

- [ ] **Step 6: Commit the extracted view-model boundary**

```powershell
git add apps/desktop/src/features/console/accountManagementViewModel.ts apps/desktop/src/features/console/accountManagementViewModel.test.ts apps/desktop/src/features/console/BrowserConsoleApp.tsx
git commit -m "feat(ui): extract gateway account management view model"
```

## Task 2: Replace the accounts workspace with a ledger-style admin component

**Files:**
- Create: `apps/desktop/src/features/console/AccountsLedgerWorkspace.tsx`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.tsx`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.test.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Write failing integration assertions for the new account ledger layout**

```ts
it("renders a ledger-style accounts workspace with summary cards, filters, and provider metadata", async () => {
  const consoleApi = createConsoleApi();
  const user = userEvent.setup();

  renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
  await waitForConsoleReady();
  await openWorkspace(user, /账号台账/i);

  expect(screen.getByRole("heading", { name: /账号台账/i })).toBeInTheDocument();
  expect(screen.getByText(/统一查看 Gateway 当前可路由账号/i)).toBeInTheDocument();
  expect(screen.getByText(/账号总数/i)).toBeInTheDocument();
  expect(screen.getByRole("searchbox", { name: /搜索账号/i })).toBeInTheDocument();
  expect(screen.getByLabelText(/凭证分组/i)).toBeInTheDocument();
  expect(screen.getByLabelText(/服务商/i)).toBeInTheDocument();
  expect(screen.getByRole("table", { name: /账号台账表/i })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: /Provider/i })).toBeInTheDocument();
  expect(screen.getByText("Managed OpenAI")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: /Managed OpenAI · 2 个账号/i })).not.toBeInTheDocument();
});

it("filters ledger rows by provider and specific group without using provider buckets", async () => {
  const consoleApi = createConsoleApi();
  const user = userEvent.setup();

  renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
  await waitForConsoleReady();
  await openWorkspace(user, /账号台账/i);

  await user.selectOptions(screen.getByLabelText(/凭证分组/i), "group-vip");
  await user.selectOptions(screen.getByLabelText(/服务商/i), "openai");

  expect(screen.getByText("生产账号 A")).toBeInTheDocument();
  expect(screen.queryByText("生产账号 B")).not.toBeInTheDocument();
});
```

Update the existing account-workspace assertions instead of duplicating old bucket-specific expectations.

- [ ] **Step 2: Run the account-focused integration test and confirm it fails on the old UI**

Run: `npm test -- --run src/features/console/BrowserConsoleApp.test.tsx`  
Expected: FAIL because the current accounts workspace still renders provider buckets and lacks the new table/filter structure.

- [ ] **Step 3: Implement `AccountsLedgerWorkspace.tsx` and pass it the existing actions from `BrowserConsoleApp`**

```tsx
export type AccountsLedgerWorkspaceProps = {
  t: BrowserConsoleTranslate;
  editorLocked: boolean;
  totalAccounts: number;
  totalGroups: number;
  ungroupedCount: number;
  providerCount: number;
  rows: AccountLedgerRow[];
  groupOptions: Array<{ value: string; label: string }>;
  providerOptions: Array<{ value: string; label: string }>;
  query: string;
  membership: AccountMembershipFilter;
  enabled: AccountEnabledFilter;
  selectedGroupId: string;
  selectedProviderKey: string;
  onQueryChange: (value: string) => void;
  onMembershipChange: (value: AccountMembershipFilter) => void;
  onEnabledChange: (value: AccountEnabledFilter) => void;
  onGroupChange: (value: string) => void;
  onProviderChange: (value: string) => void;
  onOpenAddAccount: () => void;
  onOpenGroups: () => void;
  onBackToEditor: () => void;
  onProbe: (accountId: string) => void;
  onEdit: (providerId: string, accountId: string) => void;
  onDelete: (providerId: string, accountId: string, displayName: string) => void;
  onAddExplicit: (providerId: string) => void;
};

export function AccountsLedgerWorkspace(props: AccountsLedgerWorkspaceProps) {
  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel nt-ledger-shell">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Accounts Ledger</p>
            <h2>{props.t("账号台账", "Accounts")}</h2>
            <p className="nt-copy">
              {props.t(
                "统一查看 Gateway 当前可路由账号、分组归属与连通状态。",
                "View Gateway's routable accounts, group assignment, and connectivity state in one ledger.",
              )}
            </p>
          </div>
          <div className="nt-actions">
            <button className="nt-btn nt-btn--primary" type="button" onClick={props.onOpenAddAccount}>
              {props.t("添加账号", "Add account")}
            </button>
            <button className="nt-btn nt-btn--secondary" type="button" onClick={props.onOpenGroups}>
              {props.t("进入凭证分组", "Open credential groups")}
            </button>
            <button className="nt-btn nt-btn--outline" type="button" onClick={props.onBackToEditor}>
              {props.t("返回路由编辑", "Back to route editor")}
            </button>
          </div>
        </div>
        <div className="nt-ledger-stats">{/* summary cards */}</div>
        <div className="nt-ledger-toolbar">{/* search + select controls */}</div>
      </article>

      <section className="nt-ledger-table" role="table" aria-label={props.t("账号台账表", "Account ledger table")}>
        <div className="nt-ledger-table__head" role="row">
          <span role="columnheader">{props.t("账号", "Account")}</span>
          <span role="columnheader">Provider</span>
          <span role="columnheader">{props.t("模式", "Mode")}</span>
          <span role="columnheader">{props.t("状态", "Status")}</span>
          <span role="columnheader">{props.t("模型", "Models")}</span>
          <span role="columnheader">{props.t("凭证分组", "Groups")}</span>
          <span role="columnheader">{props.t("探测状态", "Probe")}</span>
          <span role="columnheader">{props.t("操作", "Actions")}</span>
        </div>
        {/* rows or empty state */}
      </section>
    </div>
  );
}
```

Use the extracted ledger rows; do not rebuild provider buckets inside the new component.

- [ ] **Step 4: Replace the inline `accountsWorkspace` block in `BrowserConsoleApp.tsx` and add the new CSS classes**

```ts
const accountsWorkspace = (
  <AccountsLedgerWorkspace
    t={t}
    editorLocked={editorLocked}
    totalAccounts={displayedAccountCatalog.accounts.length}
    totalGroups={displayedAccountCatalog.groups.length}
    ungroupedCount={displayedAccountCatalog.ungroupedCount}
    providerCount={displayedAccountCatalog.providerBuckets.length}
    rows={filteredAccountLedgerRows}
    groupOptions={accountLedgerGroupOptions}
    providerOptions={accountLedgerProviderOptions}
    query={accountSearch}
    membership={accountMembershipFilter}
    enabled={accountEnabledFilter}
    selectedGroupId={selectedAccountGroupFilter}
    selectedProviderKey={selectedProviderFilter}
    onQueryChange={setAccountSearch}
    onMembershipChange={setAccountMembershipFilter}
    onEnabledChange={setAccountEnabledFilter}
    onGroupChange={setSelectedAccountGroupFilter}
    onProviderChange={setSelectedProviderFilter}
    onOpenAddAccount={() => openAddCredentialDialog()}
    onOpenGroups={() => setActiveWorkspace("groups")}
    onBackToEditor={() => setActiveWorkspace("editor")}
    onProbe={(accountId) => void handleCredentialProbe(findAccountOrThrow(accountId))}
    onEdit={(providerId, accountId) => openEditCredentialDialog(providerId, accountId)}
    onDelete={(providerId, accountId, displayName) => removeCredential(providerId, accountId, displayName)}
    onAddExplicit={(providerId) => openAddCredentialDialog(providerId)}
  />
);
```

Add CSS for:

```css
.nt-ledger-shell {}
.nt-ledger-stats { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
.nt-ledger-toolbar { display: grid; grid-template-columns: minmax(240px, 1.4fr) repeat(4, minmax(140px, 1fr)); gap: 12px; }
.nt-ledger-table { display: grid; gap: 8px; }
.nt-ledger-table__head,
.nt-ledger-table__row { display: grid; grid-template-columns: minmax(220px, 1.5fr) 180px 130px 110px minmax(180px, 1.2fr) minmax(180px, 1fr) 180px 180px; }
```

Do not delete the old `.nt-account-bucket*` rules yet; other workspaces still compile against the shared stylesheet until Task 3 finishes.

- [ ] **Step 5: Re-run the account UI tests and typecheck**

Run: `npm test -- --run src/features/console/BrowserConsoleApp.test.tsx`  
Expected: PASS for the updated account-ledger assertions and the existing probe/edit behaviors.

Run: `npm run typecheck`  
Expected: PASS with `AccountsLedgerWorkspace.tsx` imported into `BrowserConsoleApp.tsx`.

- [ ] **Step 6: Commit the new account ledger UI**

```powershell
git add apps/desktop/src/features/console/AccountsLedgerWorkspace.tsx apps/desktop/src/features/console/BrowserConsoleApp.tsx apps/desktop/src/features/console/BrowserConsoleApp.test.tsx apps/desktop/src/styles.css
git commit -m "feat(ui): redesign gateway accounts workspace as ledger"
```

## Task 3: Replace the groups workspace with a master-detail admin surface

**Files:**
- Create: `apps/desktop/src/features/console/CredentialGroupsWorkspace.tsx`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.tsx`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.test.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Write failing integration assertions for the new credential-groups layout**

```ts
it("renders credential groups as a directory plus detail editor", async () => {
  const consoleApi = createConsoleApi();
  const user = userEvent.setup();

  renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
  await waitForConsoleReady();
  await openWorkspace(user, /凭证分组|分组策略/i);

  expect(screen.getByRole("heading", { name: /凭证分组|分组策略/i })).toBeInTheDocument();
  expect(screen.getByRole("navigation", { name: /分组列表/i })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: /VIP 分组/i })).toBeInTheDocument();
  expect(screen.getByRole("region", { name: /分组详情/i })).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: /分组 ID/i })).toHaveValue("group-vip");
  expect(screen.getByRole("region", { name: /成员管理/i })).toBeInTheDocument();
  expect(screen.queryByText(/No selectable accounts match this filter\./i)).not.toBeInTheDocument();
});

it("adds a group and manages members from compact candidate rows", async () => {
  const consoleApi = createConsoleApi();
  const user = userEvent.setup();

  renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
  await waitForConsoleReady();
  await openWorkspace(user, /凭证分组|分组策略/i);

  await user.click(screen.getByRole("button", { name: /添加分组/i }));
  await user.type(screen.getByLabelText(/分组 ID/i), "group-team-b");
  await user.type(screen.getByLabelText(/分组名称/i), "Team B");
  await user.type(screen.getByRole("searchbox", { name: /筛选候选账号/i }), "生产账号 B");
  await user.click(screen.getByRole("button", { name: /加入.*生产账号 B/i }));

  await openWorkspace(user, /高级 JSON/i);
  const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
  expect((editor as HTMLTextAreaElement).value).toContain('"group-team-b"');
  expect((editor as HTMLTextAreaElement).value).toContain('"acc-prod-2"');
});
```

Update the existing structured-group tests so they verify draft synchronization through the new layout instead of the old per-card checkbox cloud.

- [ ] **Step 2: Run the groups-focused integration test and confirm it fails on the current UI**

Run: `npm test -- --run src/features/console/BrowserConsoleApp.test.tsx`  
Expected: FAIL because the current UI still renders stacked cards and bucketed candidate pickers.

- [ ] **Step 3: Implement `CredentialGroupsWorkspace.tsx` as a master-detail admin component**

```tsx
export type CredentialGroupsWorkspaceProps = {
  t: BrowserConsoleTranslate;
  editorLocked: boolean;
  groups: CredentialGroupDirectoryItem[];
  selectedGroupRowId: string | null;
  selectedGroup: AccountGroupDraftRow | null;
  memberCandidates: GroupMemberCandidate[];
  totalAccounts: number;
  ungroupedCount: number;
  enabledGroupCount: number;
  onSelectGroup: (rowId: string) => void;
  onAddGroup: () => void;
  onBackToAccounts: () => void;
  onUpdateField: (rowId: string, field: keyof AccountGroupDraftRow, value: string | boolean) => void;
  onToggleEnabled: (rowId: string, nextEnabled: boolean) => void;
  onRemoveGroup: (rowId: string) => void;
  memberQuery: string;
  memberMode: "all" | "members" | "ungrouped";
  onMemberQueryChange: (value: string) => void;
  onMemberModeChange: (value: "all" | "members" | "ungrouped") => void;
  onToggleMember: (rowId: string, accountId: string) => void;
};

export function CredentialGroupsWorkspace(props: CredentialGroupsWorkspaceProps) {
  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel nt-groups-shell">
        <div className="nt-section__head">{/* title and actions */}</div>
        <div className="nt-groups-stats">{/* summary cards */}</div>
      </article>

      <section className="nt-group-admin" aria-label={props.t("凭证分组管理", "Credential group administration")}>
        <nav className="nt-group-admin__list" aria-label={props.t("分组列表", "Group list")}>
          {/* compact selectable directory entries */}
        </nav>
        <div className="nt-group-admin__detail">
          {/* base metadata form, membership summary, candidate rows */}
        </div>
      </section>
    </div>
  );
}
```

Inside the detail pane, expose separate sections for:

- base metadata;
- membership summary; and
- member management.

Candidate rows should be rendered from `memberCandidates` with join/remove buttons rather than nested provider buckets.

- [ ] **Step 4: Integrate the new groups component and add selected-group state in `BrowserConsoleApp.tsx`**

```ts
const [selectedAccountGroupRowId, setSelectedAccountGroupRowId] = useState<string | null>(null);
const [groupMemberQuery, setGroupMemberQuery] = useState("");
const [groupMemberMode, setGroupMemberMode] = useState<"all" | "members" | "ungrouped">("all");

const groupDirectory = useMemo(
  () => buildCredentialGroupDirectory(displayedAccountCatalog, accountGroupDraftRows),
  [displayedAccountCatalog, accountGroupDraftRows],
);

const effectiveSelectedGroupRowId = selectedAccountGroupRowId ?? groupDirectory[0]?.rowId ?? null;
const selectedAccountGroupDraft = accountGroupDraftRows.find((row) => row.id === effectiveSelectedGroupRowId) ?? null;
const selectedGroupMemberCandidates = useMemo(
  () =>
    buildGroupMemberCandidates(displayedAccountCatalog.accounts, {
      selectedCredentialIds: selectedAccountGroupDraft?.providerCredentialIds ?? [],
      query: groupMemberQuery,
      mode: groupMemberMode,
    }),
  [displayedAccountCatalog.accounts, groupMemberMode, groupMemberQuery, selectedAccountGroupDraft],
);
```

When `addAccountGroupRow()` creates a new row, also select it:

```ts
const nextRow = createAccountGroupDraftRow();
applyAccountGroupDraftRows([...accountGroupDraftRows, nextRow]);
setSelectedAccountGroupRowId(nextRow.id);
setGroupMemberQuery("");
setGroupMemberMode("all");
```

Add CSS for:

```css
.nt-group-admin { display: grid; grid-template-columns: minmax(240px, 320px) minmax(0, 1fr); gap: 16px; }
.nt-group-admin__list { display: grid; gap: 10px; }
.nt-group-admin__detail { display: grid; gap: 16px; }
.nt-group-directory-item { border: 1px solid rgba(255,255,255,0.08); border-radius: 16px; }
.nt-group-members { display: grid; gap: 8px; }
.nt-group-member-row { display: grid; grid-template-columns: minmax(220px, 1.3fr) 160px 120px 160px 120px; gap: 10px; align-items: center; }
```

- [ ] **Step 5: Re-run group integration tests and typecheck**

Run: `npm test -- --run src/features/console/BrowserConsoleApp.test.tsx`  
Expected: PASS for the updated group-management assertions and the existing draft-save behavior.

Run: `npm run typecheck`  
Expected: PASS with `CredentialGroupsWorkspace.tsx` compiled and imported.

- [ ] **Step 6: Commit the groups workspace redesign**

```powershell
git add apps/desktop/src/features/console/CredentialGroupsWorkspace.tsx apps/desktop/src/features/console/BrowserConsoleApp.tsx apps/desktop/src/features/console/BrowserConsoleApp.test.tsx apps/desktop/src/styles.css
git commit -m "feat(ui): redesign gateway credential groups workspace"
```

## Task 4: Finish copy, empty states, and end-to-end verification for the new admin experience

**Files:**
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.tsx`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.i18n.test.tsx`
- Modify: `apps/desktop/src/features/console/BrowserConsoleApp.test.tsx`
- Modify: `apps/desktop/src/styles.css`

- [ ] **Step 1: Write failing tests for bilingual workspace labels and the new empty states**

```ts
it("keeps the accounts and groups workspace labels bilingual after language switch", async () => {
  const consoleApi = createConsoleApi();
  const user = userEvent.setup();

  renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
  await waitForConsoleReady();

  expect(screen.getByRole("button", { name: /账号台账/i })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /凭证分组|分组策略/i })).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "切换界面语言" }));

  expect(screen.getByRole("button", { name: /Accounts/i })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Credential Groups|Groups/i })).toBeInTheDocument();
});

it("renders dedicated empty states for no accounts and no groups", async () => {
  const consoleApi = createConsoleApi();
  const user = userEvent.setup();

  vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
    routeConfig: {
      revision: { id: "r1-empty", sequence: 1, message: "empty" },
      source: "redis",
      diagnostics: { diagnostics: [] },
      requiresRepair: false,
      document: { providers: [], model_routes: [], aliases: {}, account_groups: [] },
      secrets: [],
      mutationSupported: true,
    },
  });

  renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
  await waitForConsoleReady();

  await openWorkspace(user, /账号台账/i);
  expect(screen.getByText(/当前 route-config 里还没有解析出 provider credential/i)).toBeInTheDocument();

  await openWorkspace(user, /凭证分组|分组策略/i);
  expect(screen.getByText(/当前还没有任何凭证分组/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the i18n and empty-state tests and confirm the old copy/layout fails**

Run: `npm test -- --run src/features/console/BrowserConsoleApp.i18n.test.tsx src/features/console/BrowserConsoleApp.test.tsx`  
Expected: FAIL because the current workspace labels and empty-state copy still reflect the pre-redesign UI.

- [ ] **Step 3: Finish the polish pass in the live components and stylesheet**

Apply these last-mile changes:

```ts
const workspaceItems = [
  { id: "overview", label: t("总览", "Overview") },
  { id: "editor", label: t("路由编辑", "Route Editor") },
  { id: "providers", label: t("Providers", "Providers") },
  { id: "accounts", label: t("账号台账", "Accounts") },
  { id: "groups", label: t("凭证分组", "Credential Groups") },
  { id: "secrets", label: t("敏感字段", "Secrets") },
  { id: "revisions", label: t("修订历史", "Revisions") },
] as const;
```

And in the two new components:

```tsx
<p className="nt-empty-state__copy">
  {t(
    "当前 route-config 里还没有解析出 provider credential。请先添加 credentials，再回到这里管理账号池。",
    "No provider credentials were found in the current route config. Add credentials first, then return here to manage the account pool.",
  )}
</p>

<p className="nt-empty-state__copy">
  {t(
    "当前还没有任何凭证分组。先创建一个分组，再把不同账号组织进可复用池。",
    "No credential groups exist yet. Create one first, then organize accounts into reusable pools.",
  )}
</p>
```

Also add responsive rules so the ledger table and group master-detail layout collapse cleanly below the existing mobile breakpoints.

- [ ] **Step 4: Run the full focused verification suite**

Run: `npm test -- --run src/features/console/accountManagementViewModel.test.ts src/features/console/BrowserConsoleApp.test.tsx src/features/console/BrowserConsoleApp.i18n.test.tsx`  
Expected: PASS.

Run: `npm run typecheck`  
Expected: PASS.

Run: `npm run build:web`  
Expected: PASS with the updated admin UI compiled for the web target.

- [ ] **Step 5: Commit the finished Gateway credential management UI redesign**

```powershell
git add apps/desktop/src/features/console/accountManagementViewModel.ts apps/desktop/src/features/console/accountManagementViewModel.test.ts apps/desktop/src/features/console/AccountsLedgerWorkspace.tsx apps/desktop/src/features/console/CredentialGroupsWorkspace.tsx apps/desktop/src/features/console/BrowserConsoleApp.tsx apps/desktop/src/features/console/BrowserConsoleApp.test.tsx apps/desktop/src/features/console/BrowserConsoleApp.i18n.test.tsx apps/desktop/src/styles.css
git commit -m "feat(ui): redesign gateway credential management console"
```
