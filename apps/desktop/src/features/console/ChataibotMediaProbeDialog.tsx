import { Button, ConfirmDialog, Dialog, Field, Notice, Select, Textarea } from "@neuro/ui";
import { useEffect, useMemo, useState } from "react";
import type { ConsoleProviderProbeRequest, ConsoleRouteDocument } from "../../api/contracts";
import { sessionMaterialTarget } from "./providerSessionTarget";
import { isRecord } from "./routeDocument";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { useCredentialTestRuns, type TestProbeAction } from "./useCredentialTestRuns";
import { ChataibotProbeImages } from "./ChataibotProbeImages";
import { chataibotProbeModels } from "./chataibotProbeModels";

type Props = { document: ConsoleRouteDocument | null; providerId: string; credentialId: string;
  t(zh: string, en: string): string; onProbe: TestProbeAction; onClose(): void;
  locked: boolean; draftReady: boolean; error: string | null };
/** Exact manual operation; cost consent is ephemeral and invalidated by input or target changes. */
export function ChataibotMediaProbeDialog({ document, providerId, credentialId, t, onProbe, onClose, locked, draftReady, error }: Props) {
  const context = testProviderContext(document, providerId), account = context?.accounts.find(row => row.id === credentialId);
  const credential = context?.credentials.find(row => row.id === credentialId);
  const identity = useMemo(() => {
    const { credentials: _credentials, ...provider } = context?.provider ?? {};
    return JSON.stringify([providerId, credentialId, provider, credential]);
  }, [providerId, credentialId, context?.provider, credential]);
  const [operation, setOperation] = useState<"image" | "image_edit">("image"), [selected, setSelected] = useState("");
  const [prompt, setPrompt] = useState(""), [images, setImages] = useState<string[]>([]);
  const [pending, setPending] = useState<{ identity: string; request: ConsoleProviderProbeRequest } | null>(null);
  const runs = useCredentialTestRuns(identity, [], onProbe);
  useEffect(() => { setImages([]); setSelected(""); }, [identity]);
  const models = context && credential ? chataibotProbeModels(context.provider, credential, operation === "image_edit") : [];
  const model = models.includes(selected) ? selected : models[0] ?? "";
  let targetError: string | null = null;
  try {
    if (!document) throw new Error(t("配置不可用。", "Configuration unavailable."));
    const target = sessionMaterialTarget(document, providerId, credentialId, { product: "chataibot-web" });
    // Runtime compilation takes the provider flow (including its serialized callFlow alias),
    // not a credential-level block. Do not invent an override/inheritance rule in the UI.
    const flow = target.provider.call_flow ?? target.provider.callFlow;
    if (!isRecord(flow) || flow.special_adapter !== "chataibot_web") throw new Error(t("此账号尚未启用 ChatAIBot Web 执行器。", "This account has not enabled the ChatAIBot Web executor."));
  } catch (cause) { targetError = cause instanceof Error ? cause.message : String(cause); }
  const request: ConsoleProviderProbeRequest = { prompt, model, credentialIds: [credentialId],
    media: { operation, confirmCharge: true, parameters: operation === "image_edit" ? { images } : {} } };
  const requestKey = JSON.stringify(request);
  useEffect(() => { setPending(null); }, [identity, requestKey, draftReady]);
  const busy = locked || runs.busy, targetReady = !targetError && context?.provider.enabled !== false && account?.enabled === true;
  const withinBudget = new TextEncoder().encode(requestKey).length <= 262_144;
  const canRun = !busy && draftReady && targetReady && !!model && prompt.trim().length > 0
    && new TextEncoder().encode(prompt).length <= 8192 && withinBudget && (operation !== "image_edit" || images.length > 0);
  const close = () => { runs.cancel(); setPending(null); onClose(); };
  const confirm = async () => {
    const saved = pending; setPending(null);
    if (!canRun || !saved || saved.identity !== identity || JSON.stringify(saved.request) !== requestKey) return;
    await runs.run([{ providerId, request: saved.request }]);
  };
  const result = runs.responses.at(-1)?.result.results.find(row => row.credentialId === credentialId);
  return <Dialog open title={t(`图片测试 · ${account?.name ?? credentialId}`, `Image test · ${account?.name ?? credentialId}`)}
    closeLabel={t("关闭测试弹窗", "Close test dialog")} className="nt-pool-test-dialog" onOpenChange={open => { if (!open) close(); }}>
    <div className="nt-test-dialog-body"><div className="nt-test-dialog-form">
      <Field label={t("操作", "Operation")}><Select value={operation} disabled={busy} onChange={event => { setOperation(event.target.value as typeof operation); setSelected(""); setImages([]); }}>
        <option value="image">{t("图片生成", "Image generation")}</option><option value="image_edit">{t("图片编辑 / 多图合成", "Image edit / merge")}</option>
      </Select></Field>
      <Field label={t("模型", "Model")}><Select value={model} disabled={busy || !models.length} onChange={event => setSelected(event.target.value)}>
        {models.length ? models.map(value => <option key={value}>{value}</option>) : <option value="">{t("没有匹配的已配置模型", "No matching configured model")}</option>}
      </Select></Field>
      <Field label={t("提示词", "Prompt")}><Textarea rows={3} value={prompt} maxLength={6000} disabled={busy} onChange={event => setPrompt(event.target.value)} /></Field>
      {operation === "image_edit" ? <ChataibotProbeImages key={identity} t={t} disabled={busy} onChange={setImages} /> : null}
      {!draftReady ? <Notice tone="warning">{t("请先保存当前路由草稿，再运行测试。", "Save the route draft before running tests.")}</Notice> : null}
      {!targetReady ? <Notice tone="danger">{targetError ?? t("账号已停用或不存在。", "The account is disabled or missing.")}</Notice> : null}
      {!withinBudget ? <Notice tone="danger">{t("测试请求超过 256 KiB。", "Test request exceeds 256 KiB.")}</Notice> : null}
      {error ?? runs.error ? <Notice tone="danger">{error ?? runs.error}</Notice> : null}
      {result ? <Notice tone={result.status === "passed" ? "success" : result.status === "failed" ? "danger" : "warning"}>{result.message}</Notice> : null}
    </div></div>
    <div className="n-dialog-actions"><Button onClick={close}>{t("关闭", "Close")}</Button><Button variant="primary" disabled={!canRun} loading={runs.busy}
      onClick={() => { if (canRun) setPending({ identity, request }); }}>{t("运行测试", "Run tests")}</Button></div>
    <ConfirmDialog open={!!pending} onOpenChange={open => { if (!open) setPending(null); }} title={t("确认图片生成", "Confirm image generation")}
      description={t(`账号 ${account?.name ?? credentialId} · ${operation} · ${model}：可能消耗供应商额度，扣额未知。关闭或取消本地等待不能撤销已受理的远端生成，也不保证退费。`,
        `Account ${account?.name ?? credentialId} · ${operation} · ${model}: may consume provider quota; charge unknown. Closing or cancelling local waiting cannot revoke an accepted generation or guarantee a refund.`)}
      confirmLabel={t("确认生成", "Confirm generation")} cancelLabel={t("取消", "Cancel")} confirmDisabled={!canRun || pending?.identity !== identity} onConfirm={confirm} />
  </Dialog>;
}
