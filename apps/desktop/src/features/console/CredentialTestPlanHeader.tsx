import { Checkbox, CheckboxGroup, Field, Input, MultiSelect, Select } from "@neuro/ui";
import type { TranslateFn } from "./accountsLedgerTypes";
import type { useCredentialTestDraft } from "./useCredentialTestDraft";

/** Whole-pool membership is distinct from selecting all currently listed subpools. */
export function CredentialTestPlanHeader({ name, onNameChange, state, locked, onChange, t }: {
  name: string; onNameChange(name: string): void; state: ReturnType<typeof useCredentialTestDraft>;
  locked: boolean; onChange(): void; t: TranslateFn;
}) {
  const kind = state.selection.kind === "account" ? "account" : "subpool";
  const options = state.options.filter((option) => option.scope.kind === kind);
  // Preserve legacy pool scopes, including ungrouped and future accounts, until a subset is chosen.
  const wholePool = state.selection.kind === "pool";
  const keys = wholePool ? options.filter((option) => state.targets.some((target) => target.providerId === option.scope.providerId)).map((option) => option.key) : state.selection.keys;
  const selected = options.filter((option) => keys.includes(option.key));
  const all = kind === "subpool" ? wholePool && state.targets.length === state.options.filter((option) => option.scope.kind === "pool").length
    : options.length > 0 && selected.length === options.length;
  const label = kind === "subpool" ? t("选择子池", "Select subpools") : t("选择账户", "Select accounts");
  const selectAll = (checked: boolean) => {
    if (checked && kind === "subpool") state.changeTargets("pool", state.options.filter((option) => option.scope.kind === "pool").map((option) => option.key));
    else state.changeTargets(kind, checked ? options.map((option) => option.key) : []);
    onChange();
  };
  const choices = options.map((option) => ({ value: option.key, label: option.label.replace(/^(子池|账户) · /, "") }));
  const changeTargets = (values: string[]) => { state.changeTargets(kind, values); onChange(); };
  return <div className="nt-test-plan-header">
    <Field label={t("计划", "Plan")}>
      <Input compact aria-label={t("计划名称", "Plan name")} value={name} maxLength={256} disabled={locked}
        onChange={(event) => { onNameChange(event.target.value); onChange(); }} /></Field>
    <Field label={t("计划类型", "Plan type")}>
      <Select compact value={kind} disabled={locked}
        onChange={(event) => { state.changeKind(event.target.value === "account" ? "account" : "subpool"); onChange(); }}>
        <option value="subpool">{t("子池类", "Subpools")}</option><option value="account">{t("账户类", "Accounts")}</option>
      </Select></Field>
    <div className="nt-test-plan-targets">
      <Checkbox label={kind === "subpool" ? t("整个凭据池（含未分组及新增账户）", "Whole pool (including ungrouped and future accounts)") : t("全选账户", "Select all accounts")}
        checked={all} indeterminate={!all && (selected.length > 0 || wholePool)} disabled={locked || (kind === "account" && !options.length)} onChange={(event) => selectAll(event.target.checked)} />
      {options.length > 6 ? <MultiSelect label={label} compact options={choices} values={keys} onValuesChange={changeTargets} disabled={locked}
        formatSummary={(count) => t(`已选 ${count} 项`, `${count} selected`)} search={{ label: t("搜索范围", "Search targets"), emptyMessage: t("无匹配项", "No matches") }} />
        : <CheckboxGroup label={label} options={choices} values={keys} onValuesChange={changeTargets} disabled={locked} />}
      {!options.length ? <span className="n-muted">{kind === "subpool" ? t("暂无子池", "No subpools") : t("暂无账户", "No accounts")}</span> : null}
    </div>
  </div>;
}
