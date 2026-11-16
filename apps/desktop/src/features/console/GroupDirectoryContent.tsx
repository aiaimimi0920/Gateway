import { useState } from "react";
import { Button, Dialog, EmptyState, Input, SegmentedControl } from "@neuro/ui";
import type { CredentialGroupDirectoryItem, GroupMemberCandidate } from "./groupDirectoryViewModel";
import type { EntitlementAccountCardBridge, TranslateFn } from "./credentialGroupsWorkspaceTypes";
import { ProviderAccountCard } from "./ProviderAccountCard";
import { ScopePager } from "./EntitlementScopePager";

type Props = { group: CredentialGroupDirectoryItem; members: GroupMemberCandidate[]; ready: boolean; bridge?: EntitlementAccountCardBridge; locked: boolean; t: TranslateFn };

/** Scope rows are derived from the selected group, never inferred from the full catalog. */
export function GroupDirectoryContent({ group, members, ready, bridge, locked, t }: Props) {
  const [tab, setTab] = useState("accounts");
  const [query, setQuery] = useState("");
  const [pageState, setPage] = useState(1);
  const [accountId, setAccountId] = useState<string | null>(null);
  const search = query.trim().toLowerCase();
  const accounts = (ready ? members : []).filter(item => `${item.displayName} ${item.providerLabel}`.toLowerCase().includes(search));
  const models = group.modelScopes.filter(item => `${item.model} ${item.members.map(member => member.providerLabel).join(" ")}`.toLowerCase().includes(search));
  const total = tab === "accounts" ? accounts.length : models.length;
  const pages = Math.max(1, Math.ceil(total / 20));
  const page = Math.min(pageState, pages);
  const account = accountId ? bridge?.accountsById.get(accountId) : null;
  return <>
    <div className="nt-directory-toolbar">
      <SegmentedControl name="group-directory-view" hideLabel label={t("分组内容", "Group content")} value={tab} onValueChange={value => { setTab(value); setPage(1); }} options={[{ value: "accounts", label: t("账号范围", "Accounts") }, { value: "models", label: t("关联模型", "Models") }]} />
      <Input compact aria-label={t("搜索组内内容", "Search group content")} placeholder={t("搜索账号、模型或供应商…", "Search account, model or provider…")} value={query} onChange={event => { setQuery(event.target.value); setPage(1); }} />
    </div>
    <div className="nt-directory-surface"><table className="nt-directory-table" aria-label={tab === "accounts" ? t("组内账号", "Group accounts") : t("关联模型", "Group models")}>
      <thead><tr>{(tab === "accounts" ? [t("供应商", "Provider"), t("账号", "Account"), t("配置状态", "Configuration state"), ""] : [t("模型", "Model"), t("供应商", "Providers"), t("关联账号", "Accounts")]).map((label, index) => <th key={index}>{label}</th>)}</tr></thead>
      <tbody>{tab === "accounts" ? accounts.slice((page - 1) * 20, page * 20).map(item => <tr key={item.accountId}>
        <td>{item.providerLabel}</td><td>{item.displayName}</td><td><span className="nt-directory-state" data-enabled={item.enabled}>{item.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}</span></td>
        <td>{bridge?.accountsById.has(item.accountId) && <Button compact variant="quiet" onClick={() => setAccountId(item.accountId)}>{t("管理账号", "Manage account")}</Button>}</td>
      </tr>) : models.slice((page - 1) * 20, page * 20).map(item => <tr key={item.model}><td title={item.model}>{item.model}</td><td>{[...new Set(item.members.map(member => member.providerLabel))].join(" · ")}</td><td>{item.members.length}</td></tr>)}</tbody>
    </table>{total === 0 && <EmptyState title={!ready && tab === "accounts" ? t("正在加载账号…", "Loading accounts…") : t("没有匹配的内容", "No matching content")} />}</div>
    <div className="nt-directory-toolbar"><span className="n-muted">{total}</span><ScopePager t={t} page={page} totalPages={pages} navLabel={t("分组内容分页", "Group content pages")} onPage={setPage} /></div>
    <Dialog title={t("账号管理", "Account management")} open={!!account} onOpenChange={open => { if (!open) setAccountId(null); }}>
      {account && bridge && <ProviderAccountCard t={t} account={account} editorLocked={locked} groupOptions={bridge.groupOptions} handlers={bridge.handlers} onAction={bridge.onAccountAction} />}
    </Dialog>
  </>;
}
