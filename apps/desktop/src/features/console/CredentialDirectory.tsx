import { useMemo, useState, type ReactNode } from "react";
import { Button, Dialog, Select, Switch } from "@neuro/ui";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import type { providerPoolEntries } from "./providerPoolView";
import { buildProviderCardSnapshot } from "./providerCardSnapshot";
import { providerAccessKind, providerAccessKindLabel, PROVIDER_ACCESS_KINDS, type ProviderAccessKind } from "./providerAccessKind";
import { CredentialPoolOverview } from "./CredentialPoolOverview";
import { CredentialAccountTable } from "./CredentialAccountTable";
import { CardModelList } from "./CardModelList";
import "./CredentialDirectory.css";

type Entry = ReturnType<typeof providerPoolEntries>[number];
type Props = {
  entries: Entry[]; props: AccountsLedgerWorkspaceProps;
  renderSettings(entry: Entry): ReactNode;
  renderAccount(section: AccountsLedgerPilotSection, account: AccountsLedgerPilotAccount): ReactNode;
  onRemove(section: AccountsLedgerPilotSection): void;
};
/** Selection uses identities, not indexes; live refresh never mixes provider and account scopes. */
export function CredentialDirectory({ entries, props, renderSettings, renderAccount, onRemove }: Props) {
  const { t, editorLocked } = props;
  const [kind, setKind] = useState<ProviderAccessKind | "all">("all");
  const [providerId, setProviderId] = useState<string | null>(null);
  const [accountKey, setAccountKey] = useState<string | null>(null);
  const [view, setView] = useState("accounts");
  const [settingsId, setSettingsId] = useState<string | null>(null);
  const filtered = entries.filter(e => kind === "all" || providerAccessKind(e.section) === kind);
  const entry = filtered.find(e => e.section.providerId === providerId);
  const allAccounts = useMemo(() => [...new Map(filtered.flatMap(e => e.snapshot.accounts).map(a => [`${a.providerId}:${a.accountId}`, a])).values()], [entries, kind]);
  const accounts = entry?.snapshot.accounts ?? allAccounts;
  const account = accounts.find(a => `${a.providerId}:${a.accountId}` === accountKey);
  const owner = account ? filtered.find(e => e.section.providerIds.includes(account.providerId)) : undefined;
  const accountSnapshot = useMemo(() => account ? buildProviderCardSnapshot({ supportsIdentityCategories: false, identityCategories: [], directAccounts: [account], poolTargetSize: 1 }) : null, [account]);
  const settingsEntry = entries.find(e => e.section.providerId === settingsId);
  const name = account?.displayName ?? entry?.section.providerLabel ?? t("全部账号", "All accounts");
  const chooseProvider = (id: string | null) => { setProviderId(id); setAccountKey(null); setView("accounts"); };
  const selectAccount = (a: AccountsLedgerPilotAccount) => { setAccountKey(`${a.providerId}:${a.accountId}`); setView("models"); };
  return <div className="nt-credential-directory">
    <aside className="nt-credential-directory__rail"><div className="nt-credential-directory__kinds" role="group" aria-label={t("接入形式", "Access type")}>
      <Button compact variant="quiet" aria-pressed={kind === "all"} onClick={() => { setKind("all"); chooseProvider(null); }}>{t("全部接入", "All types")}</Button>
      {PROVIDER_ACCESS_KINDS.map(k => <Button key={k} compact variant="quiet" aria-pressed={kind === k} onClick={() => { setKind(k); chooseProvider(null); }}>{providerAccessKindLabel(k, t)}</Button>)}
    </div><nav aria-label={t("服务商目录", "Provider directory")}>
      <Button compact variant="quiet" aria-pressed={!entry} onClick={() => chooseProvider(null)}>{t("全部账号", "All accounts")}<small>{allAccounts.length}</small></Button>
      {filtered.map(e => <Button key={e.section.providerId} compact variant="quiet" aria-pressed={entry === e} onClick={() => chooseProvider(e.section.providerId)}><span>{e.section.providerLabel}<small>{providerAccessKindLabel(providerAccessKind(e.section), t)}</small></span><small>{e.snapshot.accounts.length}</small></Button>)}
    </nav></aside>
    <section className="nt-credential-directory__main">
      {account && <div className="nt-credential-directory__breadcrumb"><Button compact variant="quiet" onClick={() => { setAccountKey(null); setView("accounts"); }}>{t("← 返回账号列表", "← Back to accounts")}</Button><span>{owner?.section.providerLabel ?? account.providerId} / {account.displayName}</span></div>}
      <header className="nt-credential-directory__head"><div><h2>{name}</h2><small>{account ? `${account.providerId} · ${t("账号详情", "Account details")}` : `${accounts.length} ${t("个账号", "accounts")}`}</small></div><div className="nt-credential-directory__actions">
        {account && <><Select aria-label={t("切换账号", "Switch account")} value={accountKey ?? ""} onChange={e => setAccountKey(e.target.value)}>{(owner?.snapshot.accounts ?? accounts).map(a => <option key={`${a.providerId}:${a.accountId}`} value={`${a.providerId}:${a.accountId}`}>{a.displayName}</option>)}</Select><Button compact disabled={editorLocked || account.previewOnly || account.mode === "provider-default"} onClick={() => props.onEdit(account.providerId, account.accountId)}>{t("编辑账号", "Edit account")}</Button></>}
        {!account && entry && <><Button compact disabled={editorLocked} onClick={() => entry.section.manualAddFamily ? props.onOpenGeminiManualAdd(entry.section.manualAddFamily, entry.section.providerId) : props.onAddExplicit(entry.section.providerId)}>{t("添加账号", "Add account")}</Button><Button compact onClick={() => setSettingsId(entry.section.providerId)}>{t("服务商设置", "Provider settings")}</Button>
          <Switch aria-label={t("服务商调度", "Provider dispatch")} checked={entry.snapshot.actionableAccounts.length > 0 && entry.snapshot.actionableAccounts.every(a => a.enabled)} disabled={editorLocked || !entry.snapshot.actionableAccounts.length} onChange={e => { for (const a of entry.snapshot.actionableAccounts) props.onToggleDispatch(a.providerId, a.accountId, e.target.checked); }} />
          <Button compact disabled={editorLocked} onClick={() => props.onOpenProviderSchedule(entry.section)}>{t("测试", "Test")}</Button><Button compact disabled={editorLocked} onClick={() => props.onOpenModelMapping(entry.section)}>{t("模型映射", "Model mapping")}</Button>
          <details className="nt-credential-directory__menu"><summary aria-label={t("更多操作", "More actions")}>···</summary><Button compact variant="danger" disabled={editorLocked} onClick={() => onRemove(entry.section)}>{t("删除服务商", "Delete provider")}</Button></details></>}
      </div></header>
      {accountSnapshot ? <CredentialPoolOverview snapshot={accountSnapshot} account name={name} t={t} /> : entry && <CredentialPoolOverview snapshot={entry.snapshot} name={name} t={t} />}
      {(entry || account) && <div className="nt-credential-directory__tabs" role="group" aria-label={t("详情视图", "Detail view")}>
        {(account ? [["models", t("模型", "Models")], ["settings", t("账号配置", "Account settings")]] : [["accounts", t("账号", "Accounts")], ["models", t("模型", "Models")]]).map(([id, label]) => <Button compact key={id} variant="quiet" aria-pressed={view === id} onClick={() => setView(id)}>{label}</Button>)}
      </div>}
      {account && view === "settings" ? <div className="nt-credential-account-settings">{owner && renderAccount(owner.section, account)}</div> : (entry || account) && view === "models" ?
        <div className="nt-credential-directory__models"><CardModelList models={account?.supportedModels ?? entry?.snapshot.supportedModels ?? []} providerId={account?.providerId ?? entry!.section.providerId} label={name} traffic={account ? account.modelTraffic : entry?.section.modelTraffic} t={t} /></div> :
        <CredentialAccountTable key={entry?.section.providerId ?? "all"} accounts={accounts} props={props} onSelect={selectAccount} />}
    </section>
    <Dialog open={Boolean(settingsEntry)} onOpenChange={open => { if (!open) setSettingsId(null); }} title={t("服务商设置", "Provider settings")} closeLabel={t("关闭", "Close")} className="nt-credential-settings-dialog">{settingsEntry && renderSettings(settingsEntry)}</Dialog>
  </div>;
}
