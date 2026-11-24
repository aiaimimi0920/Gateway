import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Checkbox, Field, Input, Notice, Textarea } from "@neuro/ui";
import { lumaCreationResponseSchema, type LumaCreationTask, type LumaReviewAnswer } from "../../api/console/luma-creation";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { LumaMcpCreationFacts } from "./LumaMcpCreationFacts";
import { LumaMcpCreationOptions } from "./LumaMcpCreationOptions";
import { LumaMcpCreationReview } from "./LumaMcpCreationReview";
import { mcpDraftOptions, newMcpDraft, type McpDraft } from "./lumaMcpCreationDraft";

type Props = Pick<LumaOAuthProps, "client" | "managementToken" | "secretGrant" | "providerId" | "credentialId"
  | "expectedRevision" | "disabled" | "onRequestSecretAccess">;
type Action = "prepare" | "start" | "read" | "cancel" | "end" | "respond";
export function LumaMcpCreationPanel(props: Props) {
  const { t } = useUiLocale();
  const scope = useMemo(() => ({ client: props.client, token: props.managementToken, grant: props.secretGrant,
    providerId: props.providerId, credentialId: props.credentialId, revision: props.expectedRevision, disabled: props.disabled }),
  [props.client, props.managementToken, props.secretGrant, props.providerId, props.credentialId, props.expectedRevision, props.disabled]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [draft, setDraft] = useState(newMcpDraft);
  const { kind, options, reuse, valid: validOptions } = mcpDraftOptions(draft);
  const [prompt, setPrompt] = useState("");
  const [budget, setBudget] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [id, setId] = useState("");
  const [task, setTask] = useState<LumaCreationTask | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  useEffect(() => {
    setTask(null); setConfirmed(false); setBusy(false); setError(false); setSubmitted(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  useEffect(() => { setId(""); }, [props.client, props.managementToken, props.providerId, props.credentialId]);
  useEffect(() => { setDraft(newMcpDraft()); },
    [props.client, props.managementToken, props.providerId, props.credentialId, props.expectedRevision]);
  useEffect(() => { setConfirmed(false); }, [draft, prompt, budget]);
  const enabled = !scope.disabled && !!scope.token && !!scope.credentialId && !!scope.revision;
  const valid = validOptions && prompt.trim().length > 0 && [...prompt].length <= 6000
    && /^\d+$/.test(budget) && Number(budget) >= 1 && Number(budget) <= 100000000;
  const run = async (action: Action, answer?: LumaReviewAnswer) => {
    if (!enabled || pending.current) return;
    if (!scope.grant) {
      try { await props.onRequestSecretAccess(); } catch { if (current.current === scope) setError(true); }
      return;
    }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(false);
    if (action === "start") { setSubmitted(true); setConfirmed(false); }
    try {
      const response = await scope.client.request("/v1/internal/gateway/console/luma-mcp-creation", lumaCreationResponseSchema, {
        method: "POST", cache: "no-store", managementToken: scope.token, secretGrant: scope.grant,
        signal: controller.signal, notifyAuthenticationFailure: false,
        body: { providerId: scope.providerId, credentialId: scope.credentialId, expectedRevision: scope.revision, action,
          ...(action !== "prepare" ? { taskId: id.trim() } : {}),
          ...(["prepare", "start"].includes(action) ? { request: { kind, prompt, budgetCredits: Number(budget), ...(options ? { options } : {}), ...(reuse ? { reuse } : {}) } } : {}),
          ...(["start", "respond"].includes(action) ? { chargeConfirmed: true } : {}),
          ...(action === "respond" ? { answer } : {}) },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      const result = response.task;
      if (response.revision !== scope.revision || response.providerId !== scope.providerId || response.credentialId !== scope.credentialId
        || (action === "prepare" && result.revision !== scope.revision)
        || result.providerId !== scope.providerId || result.credentialId !== scope.credentialId
        || (action !== "prepare" && result.id !== id.trim())) throw new Error("binding");
      setTask(result); setId(result.id);
      // An explicit read proving no dispatch occurred allows the original saved request to be submitted.
      setSubmitted(result.state !== "prepared");
    } catch { if (!controller.signal.aborted && current.current === scope) setError(true); }
    finally { if (pending.current === controller) { pending.current = null; setBusy(false); } }
  };
  const locked = busy || !!task;
  const reuseAsset = (assetId: string, operation: McpDraft["operation"]) => {
    if (!task || busy || !enabled) return;
    setDraft({ ...newMcpDraft(task.kind === "video" ? "luma_ray3_2" : "luma_uni_1", operation),
      reuseEnabled: true, reuseTask: task.id, sourceAsset: operation === "generate" ? "" : assetId,
      referenceAssets: operation === "generate" ? assetId : "" });
    setPrompt(""); setBudget(""); setTask(null); setId(""); setConfirmed(false); setSubmitted(false); setError(false);
  };
  const labels: Record<string, string> = { prepared: t("待提交", "Prepared"), opening: t("正在创建会话", "Opening session"),
    submitting: t("正在提交", "Submitting"), running: t("生成中", "Running"), completed: t("上游已完成", "Upstream completed"),
    failed: t("生成失败", "Failed"), cancelled: t("已取消", "Cancelled"), needs_input: t("需要进一步输入", "Needs input"),
    responding: t("正在提交答复", "Submitting answer"),
    superseded: t("已由其他运行接续", "Superseded"), unconfirmed: t("提交结果未确认", "Submission unconfirmed") };
  return <section className="n-stack" aria-label={t("Luma MCP 创作", "Luma MCP creation")}>
    <LumaMcpCreationOptions draft={draft} disabled={!enabled || locked} onChange={setDraft} />
    <Field label={t("提示词", "Prompt")}><Textarea value={prompt} maxLength={6000} disabled={!enabled || locked} onChange={e => setPrompt(e.target.value)} /></Field>
    <Field label={t("会话预算上限（credits）", "Session budget ceiling (credits)")}><Input value={budget} type="number" min={1} max={100000000}
      disabled={!enabled || locked} onChange={e => setBudget(e.target.value)} /></Field>
    {!task && <Button disabled={!enabled || busy || !valid} onClick={() => void run("prepare")}>{t("准备任务", "Prepare task")}</Button>}
    {task?.state === "prepared" && !submitted && <>
      <Checkbox checked={confirmed} disabled={busy} onChange={e => setConfirmed(e.target.checked)}
        label={t("确认使用此 Luma 账号的 credits 生成", "Confirm generation using this Luma account's credits")} />
      <Button disabled={!enabled || busy || !confirmed || !valid} onClick={() => void run("start")}>{t("提交生成", "Generate")}</Button>
    </>}
    <Field label={t("任务 ID", "Task ID")}><Input value={id} disabled={busy || !!task} onChange={e => setId(e.target.value)} /></Field>
    <div className="nt-actions">
      <Button disabled={!enabled || busy || !id.trim()} onClick={() => void run("read")}>{t("查询结果与费用", "Read result and cost")}</Button>
      {task?.runId && !task.ended && <Button disabled={!enabled || busy} onClick={() => void run("cancel")}>{t("取消生成", "Cancel generation")}</Button>}
      {task?.sessionId && !task.ended && <Button disabled={!enabled || busy} onClick={() => void run("end")}>{t("结束会话", "End session")}</Button>}
      {task && <Button disabled={busy} onClick={() => { setTask(null); setId(""); setConfirmed(false); setSubmitted(false); setDraft(newMcpDraft()); }}>{t("另一任务", "Another task")}</Button>}
    </div>
    {error && <Notice tone="danger">{t("操作未确认。请保留任务 ID，查询原任务后再操作。", "Operation unconfirmed. Keep the task ID and read the existing task.")}</Notice>}
    {task && <>
      <Notice>{labels[task.state] ?? task.state}{task.ended ? t(" · 会话已结束", " · Session ended") : ""}</Notice>
      {task.error && <Notice tone="warning">{t("上次操作未确认，可查询原任务。", "Last operation unconfirmed. Read the existing task.")}</Notice>}
      <LumaMcpCreationFacts task={task} onReuse={!busy && enabled ? reuseAsset : undefined} />
      <LumaMcpCreationReview task={task} disabled={busy || !enabled || !scope.grant} onAnswer={answer => void run("respond", answer)} />
    </>}
  </section>;
}
