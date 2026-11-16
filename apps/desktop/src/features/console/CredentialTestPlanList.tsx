import { useId, useState } from "react";
import { Button, DataTable, EmptyState } from "@neuro/ui";
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
  const id = useId();
  const resultId = (row: TestPlanRow) => `${id}-${encodeURIComponent(row.key)}`;
  if (!rows.length) return <EmptyState title={t("暂无测试计划", "No test plans")} />;
  const items = rows.map((row) => {
    const context = testProviderContext(document, row.providerId);
    const scope = row.scopes.map((value) => context?.options.find((option) => testScopeKey(option.scope) === testScopeKey(value))?.label
      ?? (value.kind === "pool" ? row.providerId : value.id)).join(" · ");
    const cases = row.policy.cases.filter((item) => item.enabled).map((item) => item.name).join(" · ") || t("未选择", "None selected");
    const matches = responses.flatMap((response) => testPlanResultRows(row, response.result.results));
    return { row, scope, cases, matches: matches.length };
  });
  return <div className="nt-test-plan-scroll"><DataTable label={t("测试计划", "Test plans")} rows={items} getRowKey={({ row }) => row.key}
    columns={[
      {
        key: "name", header: t("名字", "Name"), rowHeader: true, width: "18%", cell: ({ row }) => <h3 className="nt-test-plan-name"><Button compact wrap variant="quiet"
          aria-expanded={expanded.includes(row.key)} aria-controls={resultId(row)} aria-label={t(`展开测试结果 · ${row.name}`, `Expand results · ${row.name}`)}
          onClick={() => { const open = expanded.includes(row.key); setExpanded((previous) => open ? previous.filter((key) => key !== row.key) : [...previous, row.key]); onExpand(row, !open); }}>
          <span aria-hidden="true">{expanded.includes(row.key) ? "▾" : "▸"}</span> {row.name}</Button></h3>
      },
      { key: "scope", header: t("测试范围", "Scope"), width: "18%", cell: ({ scope }) => <span className="nt-test-plan-text" title={scope}>{scope}</span> },
      { key: "cases", header: t("测试集", "Test cases"), width: "18%", cell: ({ cases }) => <span className="nt-test-plan-text" title={cases}>{cases}</span> },
      { key: "interval", header: t("自动测试间隔", "Automatic interval"), width: "13%", cell: ({ row }) => row.policy.automaticEnabled ? t(`${row.policy.intervalMinutes} 分钟`, `${row.policy.intervalMinutes} min`) : t("未启用", "Disabled") },
      { key: "run", header: t("手动测试", "Manual test"), width: "13%", cell: ({ row }) => <Button compact wrap disabled={runDisabled(row)} loading={runningKey === row.key} onClick={() => onRun(row)}>{t("手动测试", "Run test")}</Button> },
      { key: "edit", header: t("编辑", "Edit"), width: "10%", cell: ({ row }) => <Button compact wrap data-plan-key={row.key} disabled={locked} onClick={() => onEdit(row)}>{t("编辑", "Edit")}</Button> },
      { key: "delete", header: t("删除", "Delete"), width: "10%", cell: ({ row }) => <Button compact wrap variant="danger" disabled={locked} onClick={() => onDelete(row)}>{t("删除", "Delete")}</Button> },
    ]}
    rowDetails={({ row, matches }) => ({
      id: resultId(row), open: expanded.includes(row.key), content: expanded.includes(row.key) ? <>
        <p>{t(`测试结果 (${matches})`, `Test results (${matches})`)}</p>
        <CredentialTestResults response={null} responses={responses.map((response) => ({
          ...response, result: {
            ...response.result,
            results: testPlanResultRows(row, response.result.results)
          }
        }))} accounts={accounts} busy={runningKey === row.key} error={null} t={t} />
      </> : null
    })} /></div>;
}
