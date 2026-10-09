import * as Dialog from "@radix-ui/react-dialog";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
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
  const returnFocus = useRef<HTMLElement | null>(null);
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
  const add = () => edit({ key: "new", id: crypto.randomUUID(), providerId: section.providerId, name: "", legacy: false,
    scopes: section.providerIds.map((providerId) => ({ kind: "pool", providerId })), policy: defaultTestPolicy() }, false);
  const savePlan = (name: string, scopes: CredentialTestScope[], policy: CredentialTestPolicy) => {
    if (!editor || !onPlanChange) return false;
    const changes = [...new Set(scopes.map((scope) => scope.providerId))].map((providerId): TestPlanChange => ({ providerId, id: editor.id,
      plan: { id: editor.id, name, policy, scopes: scopes.filter((scope) => scope.providerId === providerId).map((scope) =>
        scope.kind === "pool" ? { kind: "pool" } : { kind: scope.kind, id: scope.id }) } }));
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
  return <Dialog.Root open onOpenChange={(open) => { if (!open) probe.closePilotActionDialog(); }}>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay" />
      <Dialog.Content ref={content} className="dialog-content nt-pilot-dialog nt-pool-test-dialog nt-test-manager" aria-describedby={undefined}
        onOpenAutoFocus={() => { returnFocus.current = window.document.activeElement instanceof HTMLElement ? window.document.activeElement : null; }}
        onCloseAutoFocus={(event) => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus(); }}>
        <div className="nt-test-dialog-header">
          <div className="nt-test-manager-heading"><Dialog.Title className="nt-test-manager-title">{section.providerLabel}</Dialog.Title>
          <div className="nt-pool-test-tabs" role="tablist" aria-label={t("测试页面", "Test pages")}>
            {(["plans", "results"] as const).map((value) => <button key={value} type="button" role="tab" id={`test-page-${value}`}
              className="nt-btn nt-btn--outline" aria-controls="test-manager-page" aria-selected={page === value} tabIndex={page === value ? 0 : -1}
              disabled={runs.busy || probe.providerProbeBusy} onClick={(event) => { navigationFocus.current = event.currentTarget; navigationKey.current = null; setEditor(null); setPage(value); setError(null); }}
              onKeyDown={(event) => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
                event.preventDefault(); const next = value === "plans" ? "results" : "plans"; setEditor(null); setPage(next);
                navigationFocus.current = window.document.getElementById(`test-page-${next}`); navigationKey.current = null;
                navigationFocus.current?.focus();
              } }}>{value === "plans" ? t("测试", "Tests") : "IQ"}</button>)}
          </div></div>
          <div className="nt-test-manager-actions">
            {editor ? <button type="button" className="nt-btn nt-btn--outline" disabled={runs.busy || probe.providerProbeBusy} onClick={() => setEditor(null)}>{t("返回列表", "Back to plans")}</button>
              : <button type="button" data-add-plan className="nt-btn nt-btn--primary" disabled={locked || !document || !onPlanChange} onClick={add}>{t("添加", "Add")}</button>}
            <Dialog.Close className="nt-btn nt-btn--outline" aria-label={t("关闭测试弹窗", "Close test dialog")}>×</Dialog.Close>
          </div>
        </div>
        {error ? <p role="alert" className="nt-banner nt-banner--danger">{error}</p> : null}
        {editor ? <div className="nt-test-manager-editor" id="test-manager-page" role="tabpanel" aria-labelledby="test-page-plans"><CredentialTestDialog {...props} embedded key={editor.key}
          probe={{ ...probe, section: editor.persisted && !editor.legacy ? { ...section, providerIds: [editor.providerId] } : section }}
          initialScope={editor.legacy ? editor.scopes[0] : undefined}
          plan={editor.legacy ? undefined : editor} onSavePlan={savePlan}
          onSave={(scope, policy) => { const success = props.onSave?.(scope, policy) ?? false; if (success) setEditor(null); return success; }} /></div>
          : <div className="nt-test-manager-page" id="test-manager-page" role="tabpanel" aria-labelledby={`test-page-${page}`}>
            {results.error ?? runs.error ?? probe.providerProbeError ? <p role="alert" className="nt-banner nt-banner--danger">{results.error ?? runs.error ?? probe.providerProbeError}</p> : null}
            {results.summaryResponses.some((response) => response.result.truncated) ? <p className="nt-copy" role="status">{t("结果较多：当前汇总最多 128 条 / 池；展开计划可读取该计划的结果。", "Large result set: summary shows up to 128 rows per pool; expand a plan to read its results.")}</p> : null}
            {probe.draftDirty || !probe.draftMatchesActiveRevision ? <p className="nt-copy" role="status">{t("请先保存当前路由草稿，再运行测试。", "Save the route draft before running tests.")}</p> : null}
            {page === "results" ? <CredentialTestModelSummary results={results.summaryResults} t={t} />
              : <CredentialTestPlanList rows={rows} document={document} responses={results.responses} accounts={allAccounts}
                locked={locked} runningKey={runningKey} runDisabled={runDisabled} onRun={(row) => void runPlan(row)} onEdit={edit}
                onDelete={(row) => { setPending(row); setError(null); }} onExpand={results.expand} t={t} />}
          </div>}
        <AlertDialog.Root open={!!pending} onOpenChange={(open) => { if (!open) setPending(null); }}>
          <AlertDialog.Portal><AlertDialog.Overlay className="dialog-overlay" /><AlertDialog.Content className="dialog-content nt-test-plan-confirm">
            <AlertDialog.Title>{t("删除测试计划", "Delete test plan")}</AlertDialog.Title>
            <AlertDialog.Description>{t(`确认删除“${pending?.name ?? ""}”？`, `Delete “${pending?.name ?? ""}”?`)}</AlertDialog.Description>
            <div className="dialog-actions"><AlertDialog.Cancel asChild><button type="button" className="nt-btn nt-btn--outline">{t("取消", "Cancel")}</button></AlertDialog.Cancel>
              <button type="button" className="nt-btn nt-btn--danger" disabled={locked} onClick={remove}>{t("确认删除", "Confirm deletion")}</button></div>
          </AlertDialog.Content></AlertDialog.Portal>
        </AlertDialog.Root>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>;
}
