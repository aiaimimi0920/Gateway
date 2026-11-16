import { Button, ConfirmDialog, Dialog, Notice, Tab, TabList, TabPanel, TabsRoot } from "@neuro/ui";
import { useEffect, useRef, useState, type ComponentProps } from "react";
import { CredentialTestDialog } from "./CredentialTestDialog";
import { CredentialTestPlanList } from "./CredentialTestPlanList";
import { useCredentialTestRuns } from "./useCredentialTestRuns";
import { CredentialTestModelSummary } from "./CredentialTestModelSummary";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import { selectedTestAccounts } from "./credentialTestSelection";
import { prepareTestPlanRun, testPlanRows, type TestPlanChange, type TestPlanRow } from "./credentialTestPlansDocument";
import { useTestPlanResults } from "./useTestPlanResults";
import type { CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";

type Props = ComponentProps<typeof CredentialTestDialog> & { onPlanChange?(changes: TestPlanChange[]): boolean };

/** One dialog owns list, editor and summary navigation, preserving the original trigger focus. */
export function CredentialTestManager(props: Props) {
  const { probe, document, schedule, onPlanChange } = props;
  const { t, section } = probe;
  const [page, setPage] = useState<"plans" | "results">("plans");
  const [editor, setEditor] = useState<(TestPlanRow & { persisted: boolean }) | null>(null);
  const [pending, setPending] = useState<TestPlanRow | null>(null);
  const [error, setError] = useState<string | null>(null);
  const navigationFocus = useRef<HTMLElement | null>(null);
  const navigationKey = useRef<string | null>(null);
  const content = useRef<HTMLDivElement | null>(null);
  const rows = testPlanRows(document, section.providerIds);
  const runs = useCredentialTestRuns(JSON.stringify(section.providerIds), [], props.onProbe);
  const [runningKey, setRunningKey] = useState<string | null>(null);
  const results = useTestPlanResults(section.providerIds, props.onLoadResults, !editor && !runs.busy && !probe.providerProbeBusy);
  const locked = schedule.editorLocked || probe.providerProbeBusy || runs.busy;
  const runDisabled = (row: TestPlanRow) => locked || probe.draftDirty || !probe.draftMatchesActiveRevision || !!prepareTestPlanRun(document, row).error;
  const runPlan = async (row: TestPlanRow) => {
    if (runDisabled(row)) return;
    setError(null); setRunningKey(row.key);
    try { await runs.run(prepareTestPlanRun(document, row).invocations); results.refresh(); }
    finally { setRunningKey(null); }
  };
  const allAccounts = selectedTestAccounts(document, section.providerIds.map((providerId) => ({ kind: "pool", providerId })));
  useEffect(() => {
    // Initial modal focus belongs to Dialog so it can first capture the external trigger.
    if (!editor && !navigationFocus.current && !navigationKey.current) return;
    if (editor) content.current?.querySelector<HTMLInputElement>("input:not(:disabled)")?.focus();
    else {
      const trigger = navigationFocus.current?.isConnected ? navigationFocus.current
        : [...(content.current?.querySelectorAll<HTMLButtonElement>("[data-plan-key]") ?? [])]
          .find((button) => button.dataset.planKey === navigationKey.current)
        ?? content.current?.querySelector<HTMLButtonElement>("[data-add-plan]");
      trigger?.focus();
    }
  }, [editor]);
  const edit = (row: TestPlanRow, persisted = true) => {
    navigationFocus.current = window.document.activeElement instanceof HTMLElement ? window.document.activeElement : null;
    navigationKey.current = persisted ? row.key : null;
    setError(null); setPage("plans"); setEditor({ ...row, persisted });
  };
  const add = () => edit({
    key: "new", id: crypto.randomUUID(), providerId: section.providerId, name: "", legacy: false,
    scopes: section.providerIds.map((providerId) => ({ kind: "pool", providerId })), policy: defaultTestPolicy()
  }, false);
  const savePlan = (name: string, scopes: CredentialTestScope[], policy: CredentialTestPolicy) => {
    if (!editor || !onPlanChange) return false;
    const changes = [...new Set(scopes.map((scope) => scope.providerId))].map((providerId): TestPlanChange => ({
      providerId, id: editor.id,
      plan: {
        id: editor.id, name, policy, scopes: scopes.filter((scope) => scope.providerId === providerId).map((scope) =>
          scope.kind === "pool" ? { kind: "pool" } : { kind: scope.kind, id: scope.id })
      }
    }));
    if (onPlanChange(changes)) { setEditor(null); return true; }
    setError(t("计划保存失败，请检查配置错误。", "Plan save failed. Check the configuration error.")); return false;
  };
  const remove = () => {
    if (!pending || locked) return;
    const success = pending.legacy ? props.onSave?.(pending.scopes[0], null)
      : onPlanChange?.([{ providerId: pending.providerId, id: pending.id, plan: null }]);
    if (success) { void results.expand(pending, false); setPending(null); setError(null); results.refresh(); }
    else setError(t("计划删除失败，请检查配置错误。", "Plan deletion failed. Check the configuration error."));
  };
  return <Dialog open title={section.providerLabel} closeLabel={t("关闭测试弹窗", "Close test dialog")}
    className="nt-pool-test-dialog" onOpenChange={(open) => { if (!open) probe.closePilotActionDialog(); }}>
    <TabsRoot ref={content} className="nt-test-manager" value={page} onValueChange={(value) => { setEditor(null); setPage(value as typeof page); setError(null); }}>
      <div className="nt-test-dialog-header">
        <TabList label={t("测试页面", "Test pages")}>
          {(["plans", "results"] as const).map((value) => <Tab key={value} value={value} disabled={runs.busy || probe.providerProbeBusy}
            onFocus={(event) => { navigationFocus.current = event.currentTarget; navigationKey.current = null; }}
            onClick={() => { setEditor(null); setError(null); }}>{value === "plans" ? t("测试", "Tests") : "IQ"}</Tab>)}
        </TabList>
        {editor ? <Button compact disabled={runs.busy || probe.providerProbeBusy} onClick={() => setEditor(null)}>{t("返回列表", "Back to plans")}</Button>
          : <Button compact data-add-plan variant="primary" disabled={locked || !document || !onPlanChange} onClick={add}>{t("添加", "Add")}</Button>}
      </div>
      {error && !pending ? <Notice tone="danger">{error}</Notice> : null}
      {!editor ? <>
        {results.error ?? runs.error ?? probe.providerProbeError ? <Notice tone="danger">{results.error ?? runs.error ?? probe.providerProbeError}</Notice> : null}
        {results.summaryResponses.some((response) => response.result.truncated) ? <Notice>{t("结果较多：当前汇总最多 128 条 / 池；展开计划可读取该计划的结果。", "Large result set: summary shows up to 128 rows per pool; expand a plan to read its results.")}</Notice> : null}
        {probe.draftDirty || !probe.draftMatchesActiveRevision ? <Notice tone="warning">{t("请先保存当前路由草稿，再运行测试。", "Save the route draft before running tests.")}</Notice> : null}
      </> : null}
      <TabPanel value="plans" forceMount hidden={page !== "plans"} className="nt-test-manager-page">
        {editor ? <div className="nt-test-manager-editor"><CredentialTestDialog {...props} embedded key={editor.key}
          probe={{ ...probe, section: editor.persisted && !editor.legacy ? { ...section, providerIds: [editor.providerId] } : section }}
          initialScope={editor.legacy ? editor.scopes[0] : undefined}
          plan={editor.legacy ? undefined : editor} onSavePlan={savePlan}
          onSave={(scope, policy) => { const success = props.onSave?.(scope, policy) ?? false; if (success) setEditor(null); return success; }} /></div>
          : <CredentialTestPlanList rows={rows} document={document} responses={results.responses} accounts={allAccounts}
            locked={locked} runningKey={runningKey} runDisabled={runDisabled} onRun={(row) => void runPlan(row)} onEdit={edit}
            onDelete={(row) => { setPending(row); setError(null); }} onExpand={results.expand} t={t} />}
      </TabPanel>
      <TabPanel value="results" forceMount hidden={page !== "results"} className="nt-test-manager-page"><CredentialTestModelSummary results={results.summaryResults} t={t} /></TabPanel>
    </TabsRoot>
    <ConfirmDialog open={!!pending} onOpenChange={(open) => { if (!open) setPending(null); }}
      title={t("删除测试计划", "Delete test plan")} description={t(`确认删除“${pending?.name ?? ""}”？`, `Delete “${pending?.name ?? ""}”?`)}
      confirmLabel={t("确认删除", "Confirm deletion")} cancelLabel={t("取消", "Cancel")} confirmDisabled={locked} onConfirm={remove}
      resolveReturnFocus={(target) => target?.isConnected && !target.matches(":disabled") ? target
        : content.current?.querySelector<HTMLButtonElement>("[data-add-plan]:not(:disabled)")
        ?? content.current?.closest('[role="dialog"]')?.querySelector<HTMLButtonElement>("button:not(:disabled)") ?? null}>
      {error ? <Notice tone="danger">{error}</Notice> : null}
    </ConfirmDialog>
  </Dialog>;
}
