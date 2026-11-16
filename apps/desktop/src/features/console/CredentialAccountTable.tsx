import { useState } from "react";
import { Button, Input, Switch } from "@neuro/ui";
import type { AccountsLedgerPilotAccount, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";

export function CredentialAccountTable({ accounts, props, onSelect }: {
  accounts: AccountsLedgerPilotAccount[]; props: AccountsLedgerWorkspaceProps; onSelect(account: AccountsLedgerPilotAccount): void;
}) {
  const { t } = props;
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [page, setPage] = useState(0);
  const rows = accounts.filter(a => `${a.displayName} ${a.providerId}`.toLocaleLowerCase().includes(query.toLocaleLowerCase()) && (filter === "all" || a.enabled === (filter === "enabled")));
  const pages = Math.max(1, Math.ceil(rows.length / 20));
  const activePage = Math.min(page, pages - 1);
  return <div className="nt-credential-account-table">
    <div className="nt-credential-directory__toolbar"><div>{[["all", t("全部", "All")], ["enabled", t("已启用", "Enabled")], ["disabled", t("已停用", "Disabled")]].map(([value, label]) => <Button key={value} compact variant="quiet" aria-pressed={filter === value} onClick={() => { setFilter(value); setPage(0); }}>{label}</Button>)}</div>
      <Input type="search" aria-label={t("搜索账号", "Search accounts")} placeholder={t("搜索账号", "Search accounts")} value={query} onChange={e => { setQuery(e.target.value); setPage(0); }} />
    </div>
    <div className="nt-credential-directory__table-scroll"><table><thead><tr>{[t("账号 / 服务商", "Account / Provider"), t("并发", "Concurrency"), t("请求数", "Requests"), t("验证", "Verification"), t("调度", "Dispatch")].map(label => <th key={label}>{label}</th>)}</tr></thead>
      <tbody>{rows.slice(activePage * 20, (activePage + 1) * 20).map(a => <tr key={`${a.providerId}:${a.accountId}`}>
        <td><Button compact variant="quiet" onClick={() => onSelect(a)}>{a.displayName}</Button><small>{a.providerId}</small></td>
        <td>{a.concurrencyUsed == null ? "—" : `${a.concurrencyUsed}/${a.concurrencyTotal ?? "—"}`}</td><td>{a.requestCount?.toLocaleString() ?? "—"}</td>
        <td>{({ verified: t("已验证", "Verified"), failed: t("验证失败", "Failed"), blocked: t("受阻", "Blocked"), "not-tested": t("未验证", "Not tested") })[a.verificationStatus]}</td>
        <td><Switch checked={a.enabled} aria-label={`${t("调度", "Dispatch")} ${a.displayName}`} disabled={props.editorLocked || a.previewOnly || !a.dispatchEditable || a.mode === "provider-default"} onChange={e => props.onToggleDispatch(a.providerId, a.accountId, e.target.checked)} /></td>
      </tr>)}</tbody></table>{!rows.length && <p className="n-muted">{accounts.length ? t("没有匹配账号", "No matching accounts") : t("尚未添加账号", "No accounts yet")}</p>}</div>
    <div className="nt-credential-directory__pager"><span>{rows.length} {t("个账号", "accounts")}</span><Button compact disabled={!activePage} onClick={() => setPage(activePage - 1)}>{t("上一页", "Previous")}</Button><span>{activePage + 1} / {pages}</span><Button compact disabled={activePage + 1 >= pages} onClick={() => setPage(activePage + 1)}>{t("下一页", "Next")}</Button></div>
  </div>;
}
