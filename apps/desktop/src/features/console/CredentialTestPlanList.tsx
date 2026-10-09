import { useState } from "react";
import type { ConsoleProviderProbeResponse, ConsoleRouteDocument } from "../../api/contracts";
import type { TestPlanRow } from "./credentialTestPlansDocument";
import type { TestSelectionAccount } from "./credentialTestSelection";
import { testProviderContext, testScopeKey } from "./credentialTestPolicyDocument";
import { testPlanResultRows } from "./useTestPlanResults";
import { CredentialTestResults } from "./CredentialTestResults";

type Props = {
  rows: TestPlanRow[];
  document: ConsoleRouteDocument | null;
  responses: ConsoleProviderProbeResponse[];
  accounts: TestSelectionAccount[];
  locked: boolean;
  runningKey: string | null;
  runDisabled(row: TestPlanRow): boolean;
  onRun(row: TestPlanRow): void;
  onEdit(row: TestPlanRow): void;
  onDelete(row: TestPlanRow): void;
  onExpand(row: TestPlanRow, open: boolean): void;
  t(zh: string, en: string): string;
};

/** A single tabular row is the plan; its result is a separate, opt-in expanded row. */
export function CredentialTestPlanList({ rows, document, responses, accounts, locked, runningKey, runDisabled, onRun, onEdit, onDelete, onExpand, t }: Props) {
  const [expanded, setExpanded] = useState<string[]>([]);
  if (!rows.length) return <p className="nt-copy">{t("暂无测试计划", "No test plans")}</p>;
  return <div className="nt-test-plan-scroll"><table className="nt-test-plan-table">
    <thead><tr>{[t("名字", "Name"), t("测试范围", "Scope"), t("测试集", "Test cases"), t("自动测试间隔", "Automatic interval"),
      t("手动测试", "Manual test"), t("编辑", "Edit"), t("删除", "Delete")].map((label) => <th key={label} scope="col">{label}</th>)}</tr></thead>
    <tbody>{rows.map((row) => {
      const context = testProviderContext(document, row.providerId);
      const scope = row.scopes.map((value) => context?.options.find((option) => testScopeKey(option.scope) === testScopeKey(value))?.label
        ?? (value.kind === "pool" ? row.providerId : value.id)).join(" · ");
      const cases = row.policy.cases.filter((item) => item.enabled).map((item) => item.name).join(" · ") || t("未选择", "None selected");
      const open = expanded.includes(row.key);
      const matches = responses.flatMap((response) => testPlanResultRows(row, response.result.results));
      return <PlanRows key={row.key} row={row} scope={scope} cases={cases} open={open} matches={matches.length}
        responses={responses} accounts={accounts} locked={locked} running={runningKey === row.key} disabled={runDisabled(row)} t={t}
        onRun={() => onRun(row)} onEdit={() => onEdit(row)} onDelete={() => onDelete(row)}
        onToggle={() => { setExpanded((previous) => open ? previous.filter((key) => key !== row.key) : [...previous, row.key]); onExpand(row, !open); }} />;
    })}</tbody>
  </table></div>;
}

function PlanRows({ row, scope, cases, open, matches, responses, accounts, locked, running, disabled, onRun, onEdit, onDelete, onToggle, t }: {
  row: TestPlanRow; scope: string; cases: string; open: boolean; matches: number; responses: ConsoleProviderProbeResponse[];
  accounts: TestSelectionAccount[]; locked: boolean; running: boolean; disabled: boolean;
  onRun(): void; onEdit(): void; onDelete(): void; onToggle(): void; t: Props["t"];
}) {
  const resultId = `test-plan-results-${encodeURIComponent(row.key)}`;
  return <>
    <tr className="nt-test-plan-row">
      <th scope="row"><button type="button" className="nt-test-plan-name" title={row.name} aria-expanded={open} aria-controls={resultId}
        onClick={onToggle} aria-label={t(`展开测试结果 · ${row.name}`, `Expand results · ${row.name}`)}>
        <span aria-hidden="true">{open ? "▾" : "▸"}</span><h3>{row.name}</h3>
      </button></th>
      <td><span className="nt-test-plan-text" title={scope}>{scope}</span></td>
      <td><span className="nt-test-plan-text" title={cases}>{cases}</span></td>
      <td>{row.policy.automaticEnabled ? t(`${row.policy.intervalMinutes} 分钟`, `${row.policy.intervalMinutes} min`) : t("未启用", "Disabled")}</td>
      <td><button type="button" className="nt-btn nt-btn--outline" disabled={disabled} onClick={onRun}>{running ? t("测试中…", "Testing…") : t("手动测试", "Run test")}</button></td>
      <td><button type="button" data-plan-key={row.key} className="nt-btn nt-btn--outline" disabled={locked} onClick={onEdit}>{t("编辑", "Edit")}</button></td>
      <td><button type="button" className="nt-btn nt-btn--outline nt-test-plan-delete" disabled={locked} onClick={onDelete}>{t("删除", "Delete")}</button></td>
    </tr>
    <tr id={resultId} hidden={!open} className="nt-test-plan-expanded"><td colSpan={7}>
      {open ? <><p className="nt-copy">{t(`测试结果 (${matches})`, `Test results (${matches})`)}</p>
        <CredentialTestResults response={null} responses={responses.map((response) => ({ ...response, result: { ...response.result,
          results: testPlanResultRows(row, response.result.results) } }))} accounts={accounts} busy={running} error={null} t={t} /></> : null}
    </td></tr>
  </>;
}
