import { Button, ConfirmDialog, Dialog, Field, Notice, Select, Textarea } from "@neuro/ui";
import { useEffect, useState } from "react";
import type { ConsoleProviderProbeRequest, ConsoleRouteDocument } from "../../api/contracts";
import { lumaAgentsMaterialTarget } from "./lumaAgents";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { useCredentialTestRuns, type TestProbeAction } from "./useCredentialTestRuns";
import { emptyMediaProbeSource, LumaMediaProbeSource, mediaProbeSource } from "./LumaMediaProbeSource";
import { LumaVideoProbeControls } from "./LumaVideoProbeControls";
import { emptyLumaVideoControls, lumaVideoProbeControls } from "./lumaVideoProbeDraft";

type Operation = "image" | "image_edit" | "video" | "video_reframe" | "video_edit" | "layering";
type Props = { document: ConsoleRouteDocument | null; providerId: string; credentialId: string;
  t(zh: string, en: string): string; onProbe: TestProbeAction; onClose(): void;
  locked: boolean; draftReady: boolean; error: string | null };

/** A manual one-account operation never persists cost authorization or becomes a text test plan. */
export function LumaMediaProbeDialog({ document, providerId, credentialId, t, onProbe, onClose, locked, draftReady, error }: Props) {
  const context = testProviderContext(document, providerId);
  const account = context?.accounts.find(a => a.id === credentialId);
  const credential = context?.credentials.find(c => c.id === credentialId);
  const { credentials: _credentials, ...provider } = context?.provider ?? {};
  const identity = JSON.stringify([providerId, credentialId, provider, credential]);
  const [operation, setOperation] = useState<Operation>("image");
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [source, setSource] = useState(emptyMediaProbeSource);
  const [duration, setDuration] = useState("5s");
  const [resolution, setResolution] = useState("360p");
  const [aspectRatio, setAspectRatio] = useState("16:9");
  const [editStrength, setEditStrength] = useState("auto");
  const [layerResolution, setLayerResolution] = useState("1k");
  const [videoControls, setVideoControls] = useState(emptyLumaVideoControls);
  const [pending, setPending] = useState<{ identity: string; request: ConsoleProviderProbeRequest } | null>(null);
  const runs = useCredentialTestRuns(identity, [], onProbe);
  const reframe = operation === "video_reframe";
  const videoEdit = operation === "video_edit";
  const layering = operation === "layering";
  const video = operation === "video" || reframe || videoEdit;
  const needsSource = operation === "image_edit" || reframe || videoEdit || layering;
  const incompatibleResolution = reframe && resolution === "1080p" && ["9:16", "3:4"].includes(aspectRatio);
  const models = (account?.models ?? []).filter(model => layering ? model === "uni-1" : video ? model === "ray-3.2" : ["uni-1", "uni-1-max"].includes(model));
  const model = models.includes(selectedModel) ? selectedModel : models[0] ?? "";
  const busy = locked || runs.busy;
  const sourceValue = needsSource ? mediaProbeSource(source, video ? "video" : "image") : null;
  let targetError: string | null = null;
  try { if (document) lumaAgentsMaterialTarget(document, providerId, credentialId); else targetError = t("配置不可用。", "Configuration unavailable."); }
  catch (cause) { targetError = cause instanceof Error ? cause.message : String(cause); }
  const targetReady = !targetError && context?.provider.enabled !== false && account?.enabled === true;
  const advanced = lumaVideoProbeControls(videoControls, operation, resolution, editStrength);
  const parameters = layering ? { source: sourceValue, layering: { resolution: layerResolution } }
    : videoEdit ? { source: sourceValue, video: { resolution, ...advanced } }
    : reframe ? { source: sourceValue, aspect_ratio: aspectRatio, video: { duration, resolution, ...advanced } }
    : operation === "image_edit" ? { source: sourceValue } : video ? { video: { duration, resolution } } : {};
  const request: ConsoleProviderProbeRequest = { prompt, model, credentialIds: [credentialId], media: { operation, confirmCharge: true, parameters } };
  const requestKey = JSON.stringify(request);
  useEffect(() => { setPending(null); }, [identity, requestKey, draftReady]);
  const withinBudget = new TextEncoder().encode(requestKey).length <= 262_144;
  const canRun = !busy && draftReady && targetReady && !!model && (layering || prompt.trim().length > 0)
    && [...prompt].length <= (layering ? 500 : 6000) && new TextEncoder().encode(prompt).length <= 8192 && (!needsSource || sourceValue !== null) && !incompatibleResolution && advanced !== null && withinBudget;
  const close = () => { runs.cancel(); onClose(); };
  const prepare = () => {
    if (!canRun) return;
    setPending({ identity, request });
  };
  const confirm = async () => {
    const saved = pending; setPending(null);
    if (!canRun || !saved || saved.identity !== identity || JSON.stringify(saved.request) !== requestKey) return;
    await runs.run([{ providerId, request: saved.request }]);
  };
  const result = runs.responses.at(-1)?.result.results.find(r => r.credentialId === credentialId);
  return <Dialog open title={t(`媒体测试 · ${account?.name ?? credentialId}`, `Media test · ${account?.name ?? credentialId}`)}
    closeLabel={t("关闭测试弹窗", "Close test dialog")} className="nt-pool-test-dialog" onOpenChange={open => { if (!open) close(); }}>
    <div className="nt-test-dialog-body"><div className="nt-test-dialog-form">
      <Field label={t("操作", "Operation")}><Select value={operation} disabled={busy} onChange={e => { setOperation(e.target.value as Operation); setSelectedModel(""); setSource(emptyMediaProbeSource(["video_reframe", "video_edit"].includes(e.target.value) ? "video" : "image")); setVideoControls(emptyLumaVideoControls()); setPending(null); }}>
        <option value="image">{t("图片生成", "Image generation")}</option><option value="image_edit">{t("图片编辑", "Image edit")}</option><option value="video">{t("视频生成", "Video generation")}</option>
        <option value="video_reframe">{t("视频扩画", "Video reframe")}</option>
        <option value="video_edit">{t("视频重绘", "Video edit")}</option>
        <option value="layering">{t("图片分层", "Image layering")}</option>
      </Select></Field>
      <Field label={t("模型", "Model")}><Select value={model} disabled={busy || !models.length} onChange={e => setSelectedModel(e.target.value)}>
        {!models.length ? <option value="">{t("没有匹配的已配置模型", "No matching configured model")}</option> : models.map(m => <option key={m}>{m}</option>)}
      </Select></Field>
      <Field label={layering ? t("分层指引（可选）", "Layer guidance (optional)") : t("提示词", "Prompt")}><Textarea rows={3} value={prompt} maxLength={layering ? 500 : 6000} disabled={busy} onChange={e => setPrompt(e.target.value)} /></Field>
      {needsSource ? <LumaMediaProbeSource value={source} onChange={setSource} disabled={busy} t={t} media={video ? "video" : "image"} /> : null}
      {layering ? <Field label={t("图层分辨率", "Layer resolution")}><Select value={layerResolution} disabled={busy} onChange={e => setLayerResolution(e.target.value)}>{["1k", "2k"].map(v => <option key={v}>{v}</option>)}</Select></Field> : null}
      {reframe ? <Field label={t("目标比例", "Target aspect ratio")}><Select value={aspectRatio} disabled={busy} onChange={e => setAspectRatio(e.target.value)}>{["9:16", "3:4", "1:1", "4:3", "16:9", "21:9"].map(v => <option key={v}>{v}</option>)}</Select></Field> : null}
      {video ? <>
        {!videoEdit ? <Field label={t("时长", "Duration")}><Select value={duration} disabled={busy} onChange={e => setDuration(e.target.value)}>{["5s", "10s"].map(v => <option key={v}>{v}</option>)}</Select></Field> : null}
        <Field label={t("分辨率", "Resolution")}><Select value={resolution} disabled={busy} onChange={e => setResolution(e.target.value)}>{["360p", "540p", "720p", "1080p"].map(v => <option key={v}>{v}</option>)}</Select></Field>
      </> : null}
      {videoEdit ? <Field label={t("重绘强度", "Edit strength")}><Select value={editStrength} disabled={busy} onChange={e => setEditStrength(e.target.value)}>
        <option value="auto">{t("自动", "Automatic")}</option>
        {["adhere", "flex", "reimagine"].flatMap((band, index) => [1, 2, 3].map(level => <option key={`${band}_${level}`} value={`${band}_${level}`}>{t(["保留原片", "均衡重绘", "自由重绘"][index], ["Adhere", "Flex", "Reimagine"][index])} {level}</option>))}
      </Select></Field> : null}
      <LumaVideoProbeControls value={videoControls} onChange={setVideoControls} operation={operation} resolution={resolution} disabled={busy} t={t} />
      {incompatibleResolution ? <Notice tone="warning">{t("竖向扩画暂不支持 1080p。", "Portrait reframing does not support 1080p yet.")}</Notice> : null}
      {advanced === null && !(videoEdit && videoControls.hdr && !["720p", "1080p"].includes(resolution)) ? <Notice tone="danger">{t("视频参数无效，请检查位置、约束和关键帧。", "Invalid video parameters. Check position, conditioning and keyframes.")}</Notice> : null}
      {!withinBudget ? <Notice tone="danger">{t("测试请求超过 256 KiB。", "Test request exceeds 256 KiB.")}</Notice> : null}
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
