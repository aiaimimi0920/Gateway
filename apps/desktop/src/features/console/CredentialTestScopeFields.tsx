import type { TranslateFn } from "./accountsLedgerTypes";
import type { useCredentialTestDraft } from "./useCredentialTestDraft";

export function CredentialTestScopeFields({ state, locked, onChange, t }: {
  state: ReturnType<typeof useCredentialTestDraft>; locked: boolean; onChange(): void; t: TranslateFn;
}) {
  const name = useId();
  if (state.scopeLocked) {
    const account = state.allAccounts[0];
    return <div className="nt-test-scope-anchor" aria-label={t("固定账户", "Fixed account")}>{t("账户", "Account")} · {account?.name ?? state.targets[0]?.providerId}</div>;
  }
  const secondary = state.options.filter((option) => option.scope.kind === state.selection.kind);
  const options = secondary.map((option) => ({ value: option.key, label: option.label.replace(/^子池 · /, "") }));
  const changeKeys = (values: string[]) => { state.changeKeys(values); onChange(); };
  return <fieldset className="nt-test-scope" disabled={locked}>
    <SegmentedControl label={t("策略配置对象", "Policy scope")} name={name} hideLabel disabled={locked} value={state.selection.kind}
      options={(["pool", "subpool", "account"] as const).map((kind, index) => ({ value: kind, label: t(["全池", "子池", "账户"][index], ["Pool", "Subpools", "Account"][index]) }))}
      onValueChange={(kind) => { state.changeKind(kind); onChange(); }} />
    {state.selection.kind === "subpool" ? <div className="nt-test-scope-options">
      {secondary.length > 6 ? <MultiSelect compact label={t("选择子池", "Select subpools")} options={options} values={state.selection.keys} onValuesChange={changeKeys} disabled={locked}
        formatSummary={(count) => t(`已选 ${count} 项`, `${count} selected`)} search={{ label: t("搜索子池", "Search subpools"), emptyMessage: t("无匹配项", "No matches") }} />
        : <CheckboxGroup label={t("选择子池", "Select subpools")} options={options} values={state.selection.keys} onValuesChange={changeKeys} disabled={locked} />}
      {!secondary.length ? <span className="n-muted">{t("暂无子池", "No subpools")}</span> : null}
    </div> : null}
    {state.selection.kind === "account" ? <Select compact aria-label={t("选择账户", "Select account")}
      value={state.selection.keys[0] ?? ""} onChange={(event) => { state.changeKeys(event.target.value ? [event.target.value] : []); onChange(); }}>
      <option value="">{t("选择账户", "Select account")}</option>
      {secondary.map((option) => <option key={option.key} value={option.key}>{option.label.replace(/^账户 · /, "")}</option>)}
    </Select> : null}
  </fieldset>;
}
import { CheckboxGroup, MultiSelect, SegmentedControl, Select } from "@neuro/ui";
import { useId } from "react";
