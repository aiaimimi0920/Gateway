import type { TranslateFn } from "./accountsLedgerTypes";
import type { useCredentialTestDraft } from "./useCredentialTestDraft";

export function CredentialTestScopeFields({ state, locked, onChange, t }: {
  state: ReturnType<typeof useCredentialTestDraft>; locked: boolean; onChange(): void; t: TranslateFn;
}) {
  if (state.scopeLocked) {
    const account = state.allAccounts[0];
    return <div className="nt-test-scope-anchor" aria-label={t("固定账户", "Fixed account")}>{t("账户", "Account")} · {account?.name ?? state.targets[0]?.providerId}</div>;
  }
  const secondary = state.options.filter((option) => option.scope.kind === state.selection.kind);
  return <fieldset className="nt-test-scope" disabled={locked}>
    <div className="nt-test-scope-kinds" role="radiogroup" aria-label={t("策略配置对象", "Policy scope")}>
      {(["pool", "subpool", "account"] as const).map((kind, index) => <label key={kind}>
        <input type="radio" name="test-policy-scope" value={kind} checked={state.selection.kind === kind}
          onChange={() => { state.changeKind(kind); onChange(); }} />
        <span>{t(["全池", "子池", "账户"][index], ["Pool", "Subpools", "Account"][index])}</span>
      </label>)}
    </div>
    {state.selection.kind === "subpool" ? <div className="nt-test-scope-options" role="group" aria-label={t("选择子池", "Select subpools")}>
      {secondary.map((option) => <label key={option.key}><input type="checkbox" checked={state.selection.keys.includes(option.key)}
        onChange={(event) => { state.changeKeys(event.target.checked ? [...state.selection.keys, option.key] : state.selection.keys.filter((key) => key !== option.key)); onChange(); }} />
        <span>{option.label.replace(/^子池 · /, "")}</span></label>)}
      {!secondary.length ? <span className="nt-copy">{t("暂无子池", "No subpools")}</span> : null}
    </div> : null}
    {state.selection.kind === "account" ? <select className="nt-input" aria-label={t("选择账户", "Select account")}
      value={state.selection.keys[0] ?? ""} onChange={(event) => { state.changeKeys(event.target.value ? [event.target.value] : []); onChange(); }}>
      <option value="">{t("选择账户", "Select account")}</option>
      {secondary.map((option) => <option key={option.key} value={option.key}>{option.label.replace(/^账户 · /, "")}</option>)}
    </select> : null}
  </fieldset>;
}
