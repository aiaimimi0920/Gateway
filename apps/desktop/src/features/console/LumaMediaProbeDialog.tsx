import { Button, ConfirmDialog, Dialog, Field, Notice, Select, Textarea } from "@neuro/ui";
import { useEffect, useState } from "react";
import type { ConsoleProviderProbeRequest, ConsoleRouteDocument } from "../../api/contracts";
import { lumaAgentsMaterialTarget } from "./lumaAgents";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { useCredentialTestRuns, type TestProbeAction } from "./useCredentialTestRuns";
import { emptyMediaProbeSource, LumaMediaProbeSource, mediaProbeSource } from "./LumaMediaProbeSource";

type Operation = "image" | "image_edit" | "video";
type Props = { document: ConsoleRouteDocument | null; providerId: string; credentialId: string;
  t(zh: string, en: string): string; onProbe: TestProbeAction; onClose(): void;
  locked: boolean; draftReady: boolean; error: string | null };

/** A manual one-account operation never persists cost authorization or becomes a text test plan. */
export function LumaMediaProbeDialog({ document, providerId, credentialId, t, onProbe, onClose, locked, draftReady, error }: Props) {
  const context = testProviderContext(document, providerId);
  const account = context?.accounts.find(a => a.id === credentialId);
  const credential = context?.credentials.find(c => c.id === credentialId);
  const identity = JSON.stringify([providerId, credentialId, context?.provider.preset, context?.provider.base_url,
    context?.provider.enabled, context?.provider.model_map, context?.provider.model_map_targets,
    credential?.base_url, credential?.adapter, credential?.enabled, account]);
  const [operation, setOperation] = useState<Operation>("image");
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [source, setSource] = useState(emptyMediaProbeSource);
  const [duration, setDuration] = useState("5s");
  const [resolution, setResolution] = useState("360p");
  const [pending, setPending] = useState<{ identity: string; request: ConsoleProviderProbeRequest } | null>(null);
  const runs = useCredentialTestRuns(identity, [], onProbe);
  useEffect(() => { setPending(null); }, [identity]);
  const models = (account?.models ?? []).filter(model => operation === "video" ? model === "ray-3.2" : ["uni-1", "uni-1-max"].includes(model));
  const model = models.includes(selectedModel) ? selectedModel : models[0] ?? "";
  const busy = locked || runs.busy;
  const sourceValue = operation === "image_edit" ? mediaProbeSource(source) : null;
  let targetError: string | null = null;
  try { if (document) lumaAgentsMaterialTarget(document, providerId, credentialId); else targetError = t("配置不可用。", "Configuration unavailable."); }
  catch (cause) { targetError = cause instanceof Error ? cause.message : String(cause); }
  const targetReady = !targetError && context?.provider.enabled !== false && account?.enabled === true;
  const canRun = !busy && draftReady && targetReady && !!model && prompt.trim().length > 0
    && [...prompt].length <= 6000 && new TextEncoder().encode(prompt).length <= 8192 && (operation !== "image_edit" || sourceValue !== null);
  const close = () => { runs.cancel(); onClose(); };
  const prepare = () => {
    if (!canRun) return;
    const parameters = operation === "image_edit" ? { source: sourceValue } : operation === "video" ? { video: { duration, resolution } } : {};
    setPending({ identity, request: { prompt, model, credentialIds: [credentialId], media: { operation, confirmCharge: true, parameters } } });
  };
  const confirm = async () => {
    const saved = pending; setPending(null);
    if (!canRun || !saved || saved.identity !== identity) return;
    await runs.run([{ providerId, request: saved.request }]);
  };
  const result = runs.responses.at(-1)?.result.results.find(r => r.credentialId === credentialId);
  return <Dialog open title={t(`媒体测试 · ${account?.name ?? credentialId}`, `Media test · ${account?.name ?? credentialId}`)}
    closeLabel={t("关闭测试弹窗", "Close test dialog")} className="nt-pool-test-dialog" onOpenChange={open => { if (!open) close(); }}>
    <div className="nt-test-dialog-body"><div className="nt-test-dialog-form">
      <Field label={t("操作", "Operation")}><Select value={operation} disabled={busy} onChange={e => { setOperation(e.target.value as Operation); setSelectedModel(""); setPending(null); }}>
        <option value="image">{t("图片生成", "Image generation")}</option><option value="image_edit">{t("图片编辑", "Image edit")}</option><option value="video">{t("视频生成", "Video generation")}</option>
      </Select></Field>
      <Field label={t("模型", "Model")}><Select value={model} disabled={busy || !models.length} onChange={e => setSelectedModel(e.target.value)}>
        {!models.length ? <option value="">{t("没有匹配的已配置模型", "No matching configured model")}</option> : models.map(m => <option key={m}>{m}</option>)}
      </Select></Field>
      <Field label={t("提示词", "Prompt")}><Textarea rows={3} value={prompt} maxLength={6000} disabled={busy} onChange={e => setPrompt(e.target.value)} /></Field>
      {operation === "image_edit" ? <LumaMediaProbeSource value={source} onChange={setSource} disabled={busy} t={t} /> : null}
      {operation === "video" ? <>
        <Field label={t("时长", "Duration")}><Select value={duration} disabled={busy} onChange={e => setDuration(e.target.value)}>{["5s", "10s"].map(v => <option key={v}>{v}</option>)}</Select></Field>
        <Field label={t("分辨率", "Resolution")}><Select value={resolution} disabled={busy} onChange={e => setResolution(e.target.value)}>{["360p", "540p", "720p", "1080p"].map(v => <option key={v}>{v}</option>)}</Select></Field>
      </> : null}
      {!draftReady ? <Notice tone="warning">{t("请先保存当前路由草稿，再运行测试。", "Save the route draft before running tests.")}</Notice> : null}
      {!targetReady ? <Notice tone="danger">{targetError ?? t("账号已停用或不存在。", "The account is disabled or missing.")}</Notice> : null}
      {error ?? runs.error ? <Notice tone="danger">{error ?? runs.error}</Notice> : null}
      {result ? <Notice tone={result.status === "passed" ? "success" : result.status === "failed" ? "danger" : "warning"}>{result.message}</Notice> : null}
    </div></div>
    <div className="n-dialog-actions"><Button onClick={close}>{t("关闭", "Close")}</Button><Button variant="primary" disabled={!canRun} loading={runs.busy} onClick={prepare}>{t("运行测试", "Run tests")}</Button></div>
    <ConfirmDialog open={!!pending} onOpenChange={open => { if (!open) setPending(null); }} title={t("确认媒体生成", "Confirm media generation")}
      description={t(`账号 ${account?.name ?? credentialId} · ${operation} · ${model}：可能消耗供应商额度。关闭或取消本地等待不能撤销已受理的远端生成，也不保证退费。`,
        `Account ${account?.name ?? credentialId} · ${operation} · ${model}: may consume provider quota. Closing or cancelling local waiting cannot revoke an accepted remote generation or guarantee a refund.`)}
      confirmLabel={t("确认生成", "Confirm generation")} cancelLabel={t("取消", "Cancel")} confirmDisabled={!canRun || pending?.identity !== identity} onConfirm={confirm} />
  </Dialog>;
}
