import { Button, ConfirmDialog, Dialog, Field, Notice, Select, Textarea } from "@neuro/ui";
import { useEffect, useState } from "react";
import type { ConsoleProviderProbeRequest, ConsoleRouteDocument } from "../../api/contracts";
import { sessionMaterialTarget } from "./providerSessionTarget";
import { isRecord } from "./routeDocument";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { useCredentialTestRuns, type TestProbeAction } from "./useCredentialTestRuns";
import { udioProbeModels } from "./udioProbeModels";

type Props = { document: ConsoleRouteDocument | null; providerId: string; credentialId: string;
  activeRevision: string | null; authorizationContext?: string | null;
  t(zh: string, en: string): string; onProbe: TestProbeAction; onClose(): void;
  locked: boolean; draftReady: boolean; error: string | null };

/** Every one-account task requires fresh non-persistent consent, never automatic audio health. */
export function UdioMediaProbeDialog({ document, providerId, credentialId, activeRevision, authorizationContext, t, onProbe, onClose, locked, draftReady, error }: Props) {
  const context = testProviderContext(document, providerId), account = context?.accounts.find(row => row.id === credentialId);
  const credential = context?.credentials.find(row => row.id === credentialId);
  const { credentials: _credentials, ...provider } = context?.provider ?? {};
  const identity = JSON.stringify([providerId, credentialId, activeRevision, authorizationContext, provider, credential]);
  const [selected, setSelected] = useState(""), [prompt, setPrompt] = useState("");
  const [pending, setPending] = useState<{ identity: string; request: ConsoleProviderProbeRequest } | null>(null);
  const runs = useCredentialTestRuns(identity, [], onProbe);
  useEffect(() => { setSelected(""); }, [identity]);
  const models = context && credential ? udioProbeModels(context.provider, credential) : [];
  const model = models.includes(selected) ? selected : models[0] ?? "";
  let targetError: string | null = null;
  try {
    if (!document) throw new Error(t("配置不可用。", "Configuration unavailable."));
    const target = sessionMaterialTarget(document, providerId, credentialId, { product: "udio-web" });
    const flow = target.provider.call_flow ?? target.provider.callFlow;
    if (!isRecord(flow) || flow.special_adapter !== "udio_web") throw new Error(t("此账号尚未启用 Udio Web 执行器。", "This account has not enabled the Udio Web executor."));
    const modes = target.credential.endpoint_execution_modes ?? target.provider.endpoint_execution_modes;
    const mode = (isRecord(modes) ? modes.music_generations : undefined) ?? target.credential.execution_mode ?? target.provider.execution_mode ?? "browser_backed";
    if (mode !== "browser_backed") throw new Error(t("此任务测试需要已配置的浏览器执行模式。", "This task test requires the configured browser-backed mode."));
  } catch (cause) { targetError = cause instanceof Error ? cause.message : String(cause); }
  const request: ConsoleProviderProbeRequest = { prompt, model, credentialIds: [credentialId], media: { operation: "music", confirmCharge: true, parameters: {} } };
  const requestKey = JSON.stringify(request);
  useEffect(() => { setPending(null); }, [identity, requestKey, draftReady]);
  const busy = locked || runs.busy, targetReady = !targetError && context?.provider.enabled !== false && account?.enabled === true;
  const canRun = !busy && draftReady && !!activeRevision && targetReady && !!model && !!prompt.trim() && new TextEncoder().encode(prompt).length <= 8192;
  const close = () => { runs.cancel(); setPending(null); onClose(); };
  const confirm = async () => {
    const saved = pending; setPending(null);
    if (!canRun || !saved || saved.identity !== identity || JSON.stringify(saved.request) !== requestKey) return;
    await runs.run([{ providerId, request: saved.request }]);
  };
  const result = runs.responses.at(-1)?.result.results.find(row => row.credentialId === credentialId);
  return <Dialog open title={t(`音乐任务测试 · ${account?.name ?? credentialId}`, `Music task test · ${account?.name ?? credentialId}`)}
    closeLabel={t("关闭测试弹窗", "Close test dialog")} className="nt-pool-test-dialog" onOpenChange={open => { if (!open) close(); }}>
    <div className="nt-test-dialog-body"><div className="nt-test-dialog-form">
      <Field label={t("模型", "Model")}><Select value={model} disabled={busy || !models.length} onChange={event => setSelected(event.target.value)}>
        {models.length ? models.map(value => <option key={value}>{value}</option>) : <option value="">{t("没有匹配的已配置模型", "No matching configured model")}</option>}
      </Select></Field>
      <Field label={t("提示词", "Prompt")}><Textarea rows={3} value={prompt} maxLength={6000} disabled={busy} onChange={event => setPrompt(event.target.value)} /></Field>
      {!draftReady || !activeRevision ? <Notice tone="warning">{t("请先保存当前路由草稿，再运行测试。", "Save the route draft before running tests.")}</Notice> : null}
      {!targetReady ? <Notice tone="danger">{targetError ?? t("账号已停用或不存在。", "The account is disabled or missing.")}</Notice> : null}
      {error ?? runs.error ? <Notice tone="danger">{error ?? runs.error}</Notice> : null}
      {result ? <Notice tone={result.status === "failed" ? "danger" : "warning"}>{result.message}</Notice> : null}
    </div></div>
    <div className="n-dialog-actions"><Button onClick={close}>{t("关闭", "Close")}</Button><Button variant="primary" disabled={!canRun} loading={runs.busy}
      onClick={() => { if (canRun) setPending({ identity, request }); }}>{t("运行测试", "Run tests")}</Button></div>
    <ConfirmDialog open={!!pending} onOpenChange={open => { if (!open) setPending(null); }} title={t("确认音乐任务生成", "Confirm music task generation")}
      description={t(`账号 ${account?.name ?? credentialId} · ${model}：可能消耗生成额度，扣额未知。只验证任务元数据，不下载音频、不证明可播放或恢复模型健康。关闭或取消本地等待不能撤销已受理的生成，也不保证退费。`,
        `Account ${account?.name ?? credentialId} · ${model}: generation may consume quota; charge unknown. Task metadata only: no audio download, playback proof or model health recovery. Closing or cancelling local waiting cannot revoke an accepted generation or guarantee a refund.`)}
      confirmLabel={t("确认生成", "Confirm generation")} cancelLabel={t("取消", "Cancel")} confirmDisabled={!canRun || pending?.identity !== identity} onConfirm={confirm} />
  </Dialog>;
}
