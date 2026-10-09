import * as Dialog from "@radix-ui/react-dialog";
import { useRef, useState } from "react";
import type { ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import type { ComponentProps } from "react";
import type { PilotProviderProbePanel } from "./PilotProviderProbePanel";
import type { PilotProviderSchedulePanel } from "./PilotProviderSchedulePanel";
import { useCredentialTestDraft } from "./useCredentialTestDraft";
import { testPolicyError } from "./credentialTestPolicyDocument";
import { CredentialTestPlanFields } from "./CredentialTestPlanFields";
import { CredentialTestScopeFields } from "./CredentialTestScopeFields";
import { CredentialTestPlanHeader } from "./CredentialTestPlanHeader";
import { useCredentialTestRuns, type TestProbeAction, type TestResultLoader } from "./useCredentialTestRuns";
import { testScopeKey } from "./credentialTestPolicyDocument";

type Props = {
  probe: Omit<ComponentProps<typeof PilotProviderProbePanel>, "startProviderProbe">;
  schedule: ComponentProps<typeof PilotProviderSchedulePanel>;
  initialTab: "auto" | "manual";
  document: ConsoleRouteDocument | null;
  initialScope?: CredentialTestScope;
  scopeLocked?: boolean;
  onProbe: TestProbeAction;
  onSave?(scope: CredentialTestScope | CredentialTestScope[], policy: CredentialTestPolicy | null): boolean;
  onLoadResults?: TestResultLoader;
  embedded?: boolean;
  plan?: { id: string; name: string; scopes: CredentialTestScope[]; policy: CredentialTestPolicy; persisted: boolean };
  onSavePlan?(name: string, scopes: CredentialTestScope[], policy: CredentialTestPolicy): boolean;
};

/** The plan editor owns configuration only; the list owns execution and results. */
export function CredentialTestDialog({ probe, schedule, document, initialScope, scopeLocked = false, onProbe, onSave, embedded = false, plan, onSavePlan }: Props) {
  const { t, section, closePilotActionDialog, providerProbeBusy, providerProbeError } = probe;
  const [saved, setSaved] = useState(false);
  const [name, setName] = useState(plan?.name ?? "");
  const returnFocus = useRef<HTMLElement | null>(null);
  const state = useCredentialTestDraft(document, section, initialScope, scopeLocked, plan);
  const { targets, draft, accounts, models } = state;
  const runs = useCredentialTestRuns(state.selectionKey, targets, onProbe);
  const busy = providerProbeBusy || runs.busy;
  const locked = busy || schedule.editorLocked;
  const policyError = plan && (!name.trim() || new TextEncoder().encode(name).length > 256 || /\p{Cc}/u.test(name))
    ? t("请填写 1–256 字节的计划名称", "Enter a plan name of 1–256 bytes") : testPolicyError(draft.policy);
  const runError = policyError ?? (draft.selected.length === 0 || draft.selected.length > 128
    ? t("请选择 1–128 个账户", "Select 1–128 accounts") : state.calls > 128 ? t("本轮超过 128 次调用", "Round exceeds 128 calls") : null);
  const selectedAvailable = draft.selected.every((key) => accounts.some((account) => account.key === key));
  const inherited = state.selection.kind !== "pool" && draft.inherit;
  const unchanged = !plan || (plan.persisted && name === plan.name && JSON.stringify(draft.policy) === JSON.stringify(plan.policy)
    && JSON.stringify(targets.map(testScopeKey).sort()) === JSON.stringify(plan.scopes.map(testScopeKey).sort()));
  const canRun = !locked && state.available && !runError && unchanged && selectedAvailable && !probe.draftDirty && probe.draftMatchesActiveRevision;
  const canSave = !locked && state.available && targets.length <= 128 && (!policyError || inherited) && (!draft.mixed || inherited) && (plan ? !!onSavePlan : !!onSave);
  const save = () => {
    if (!canSave) return;
    if (plan ? onSavePlan?.(name.trim(), targets, draft.policy) : onSave?.(targets.length === 1 ? targets[0] : targets, inherited ? null : draft.policy)) setSaved(true);
  };
  const run = () => { if (canRun) void runs.run(plan ? [...new Set(targets.map((scope) => scope.providerId))].map((providerId) => ({ providerId,
    request: { planId: plan.id, credentialIds: accounts.filter((account) => account.providerId === providerId && draft.selected.includes(account.key)).map((account) => account.id) },
  })).filter((invocation) => invocation.request.credentialIds.length > 0) : state.invocations); };
  const close = () => { runs.cancel(); closePilotActionDialog(); };
  const content = <>
        {!embedded ? <div className="nt-test-dialog-header"><Dialog.Title>{t(`测试 · ${section.providerLabel}`, `Test · ${section.providerLabel}`)}</Dialog.Title>
          <Dialog.Close className="nt-btn nt-btn--outline" aria-label={t("关闭测试弹窗", "Close test dialog")}>×</Dialog.Close></div> : null}
        <div className="nt-test-dialog-body" id="credential-test-form" role="group" aria-label={t("测试计划配置", "Test plan configuration")}>
          <div className="nt-test-dialog-form">
            {plan ? <CredentialTestPlanHeader name={name} onNameChange={setName} state={state} locked={locked} t={t} onChange={() => setSaved(false)} />
              : <CredentialTestScopeFields state={state} locked={locked} t={t} onChange={() => setSaved(false)} />}
            <div className="nt-test-inheritance">{draft.mixed ? <span>{t("配置不同", "Mixed settings")}</span> : null}
              {!plan && state.selection.kind !== "pool" && state.available ? <label><input type="checkbox" checked={draft.inherit} disabled={locked} onChange={(event) => {
                state.setInheritance(event.target.checked); setSaved(false);
              }} />{t("继承", "Inherit")}</label> : null}</div>
            <fieldset className="nt-pool-test-schedule" disabled={locked || inherited || !state.available}>
              <label><input type="checkbox" checked={draft.policy.automaticEnabled} onChange={(event) => { state.updatePolicy({ ...draft.policy, automaticEnabled: event.target.checked }); setSaved(false); }} />{t("启用自动测试", "Enable automatic tests")}</label>
              <label className="nt-field"><span>{t("执行间隔（分钟）", "Interval (minutes)")}</span>
                <input className="nt-input" type="number" min={1} max={10_080} step={1} value={draft.policy.intervalMinutes || ""}
                  aria-invalid={!Number.isInteger(draft.policy.intervalMinutes) || draft.policy.intervalMinutes < 1 || draft.policy.intervalMinutes > 10_080}
                  onChange={(event) => { state.updatePolicy({ ...draft.policy, intervalMinutes: Number(event.target.value) }); setSaved(false); }} /></label>
            </fieldset>
            <CredentialTestPlanFields policy={draft.policy} models={models} configuredModels={state.configuredModels} locked={locked || inherited || !state.available} t={t}
              onChange={(policy) => { state.updatePolicy(policy); setSaved(false); }} />

          </div>
        </div>
        <div className="nt-test-dialog-footer">
          <p className="nt-copy nt-test-footer-status" role="status">{(embedded ? policyError : runs.error ?? providerProbeError ?? runError) ?? (saved ? t("已更新", "Updated") : "")}</p>
          <div className="dialog-actions"><button type="button" className="nt-btn nt-btn--secondary" onClick={close}>{t("关闭", "Close")}</button>
            {!embedded ? <button type="button" className="nt-btn nt-btn--outline" disabled={!canRun} onClick={run}>{busy ? t("测试中…", "Testing…") : t("运行测试", "Run tests")}</button> : null}
            <button type="button" className="nt-btn nt-btn--primary" disabled={!canSave} onClick={save}>{plan ? t("保存计划", "Save plan") : t("保存策略", "Save policy")}</button>
          </div>
        </div>
      </>;
  if (embedded) return content;
  return <Dialog.Root open onOpenChange={(open) => { if (!open) close(); }}>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay" />
      <Dialog.Content className="dialog-content nt-pilot-dialog nt-pool-test-dialog" aria-describedby={undefined}
        onOpenAutoFocus={() => { returnFocus.current = window.document.activeElement instanceof HTMLElement ? window.document.activeElement : null; }}
        onCloseAutoFocus={(event) => { event.preventDefault(); const trigger = returnFocus.current; if (trigger?.isConnected && !trigger.closest("[inert]")) trigger.focus(); }}>
        {content}</Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>;
}
