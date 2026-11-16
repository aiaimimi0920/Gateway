import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { VertexTargetFields } from "./VertexTargetFields";
import { validVertexTarget } from "./vertexOAuth";
import { useVertexOAuth, type VertexOAuthProps } from "./useVertexOAuth";

const validAtom = (value: string, max: number) => value.length <= max && /^[\x21-\x7e]+$/.test(value);
export function VertexOAuthPanel(props: VertexOAuthProps) {
  const { t } = useUiLocale();
  const flow = useVertexOAuth(props);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [target, setTarget] = useState(props.target ?? { project: "", location: "" });
  const [callbackUrl, setCallbackUrl] = useState("");
  const targetKey = JSON.stringify(props.target);
  useEffect(() => {
    setClientId(""); setClientSecret(""); setCallbackUrl(""); setTarget(props.target ?? { project: "", location: "" });
  }, [props.client, props.managementToken, props.providerId, props.credentialId, props.expectedRevision, targetKey]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const status = session?.status;
  useEffect(() => { if (status !== "waiting_user") setCallbackUrl(""); }, [status]);
  const unresolved = uncertain || status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  // Reopened receipts do not retain input drafts; never present a fresh empty quota as the captured save target.
  const showInputs = !session || !active && !unresolved && status !== "succeeded";
  useEffect(() => {
    props.onActivityChange?.(locked);
    return () => props.onActivityChange?.(false);
  }, [props.onActivityChange, locked]);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken || !props.target;
  const valid = validAtom(clientId, 1024) && (!clientSecret || validAtom(clientSecret, 4096)) && validVertexTarget(target);
  const messages: Record<string, string> = {
    waiting_user: t("等待浏览器授权", "Waiting for browser authorization"), exchanging: t("正在交换授权材料", "Exchanging authorization material"),
    ready: t("授权完成，待保存", "Authorization ready; not saved"), importing: t("正在保存，待确认", "Saving; confirmation pending"),
    succeeded: t("已确认保存", "Save confirmed"), save_unconfirmed: t("保存结果未确认；请核对配置与事务恢复，不要重复提交。", "Save unconfirmed; reconcile configuration and transaction recovery. Do not resubmit."),
    failed: t("授权失败", "Authorization failed"), cancelled: t("已取消授权", "Authorization cancelled"), expired: t("授权会话已过期", "Authorization session expired"),
  };
  return <section className="n-stack" aria-label="Vertex OAuth">
    {props.credentialId && <Field label={t("重新授权目标账号", "Reauthorization target")}><Input readOnly value={props.credentialId} /></Field>}
    {showInputs && <><Field label="Client ID"><Input value={clientId} maxLength={1024} autoComplete="off" spellCheck={false}
      disabled={locked || mutationDisabled} onChange={(event) => setClientId(event.target.value)} /></Field>
    <Field label={t("Client Secret（可选）", "Client secret (optional)")}><Input type="password" value={clientSecret}
      maxLength={4096} autoComplete="off" disabled={locked || mutationDisabled} onChange={(event) => setClientSecret(event.target.value)} /></Field>
    <VertexTargetFields value={target} fixed quotaFixed={Boolean(props.credentialId)} disabled={locked || mutationDisabled} onChange={setTarget} /></>}
    {!session && <Button variant="primary" loading={busy === "create"} disabled={mutationDisabled || locked || !valid} onClick={() => {
      void flow.run({ action: "create", input: { providerId: props.providerId, expectedRevision: props.expectedRevision,
        credentialId: props.credentialId, clientId, ...(clientSecret ? { clientSecret } : {}), target: { ...target } } }); setClientSecret("");
    }}>{t("通过 Vertex 登录", "Sign in with Vertex")}</Button>}
    {session && !active && !unresolved && status !== "succeeded" && <Button disabled={mutationDisabled || locked || !valid}
      onClick={() => { void flow.run({ action: "create", input: { providerId: props.providerId, expectedRevision: props.expectedRevision,
        credentialId: props.credentialId, clientId, ...(clientSecret ? { clientSecret } : {}), target: { ...target } } }); setClientSecret(""); }}>
      {t("重新开始授权", "Restart authorization")}</Button>}
    {(props.disabled || !props.expectedRevision) && <Notice>{t("请先保存服务商及当前配置。", "Save the provider and current configuration first.")}</Notice>}
    {!props.target && <Notice tone="danger">{t("Full Vertex 目标无效，请核对已保存配置。", "Invalid Full Vertex target; check saved configuration.")}</Notice>}
    {stale && <Notice tone="warning">{t("配置版本已变化；不会自动换版本。", "Configuration revision changed; no automatic rebase.")}</Notice>}
    {pending && <Notice>{t("正在恢复敏感信息访问权限", "Restoring secret access")}</Notice>}
    {status && <Notice tone={status === "succeeded" ? "success" : status === "failed" ? "danger" : ["save_unconfirmed", "expired"].includes(status) ? "warning" : "info"}>{messages[status]}</Notice>}
    {session && status === "waiting_user" && <>
      {session.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind="vertex" busy={Boolean(busy)} />}
      <Field label={t("授权链接", "Authorization URL")}><Input readOnly value={session.authorizationUrl ?? ""} /></Field>
      <Field label="Redirect URI"><Input readOnly value={session.redirectUri} /></Field>
      <Field label={t("手动回调地址", "Manual callback URL")}><Input type="password" autoComplete="off" value={callbackUrl}
        maxLength={16_384} disabled={mutationDisabled || Boolean(busy || pending)} onChange={(event) => setCallbackUrl(event.target.value)} /></Field>
      <Button disabled={mutationDisabled || Boolean(busy || pending) || !callbackUrl || new TextEncoder().encode(callbackUrl).length > 16_384}
        loading={busy === "complete"} onClick={() => { void flow.run({ action: "complete", callbackUrl }); setCallbackUrl(""); }}>
        {t("提交手动回调", "Submit manual callback")}</Button>
    </>}
    {status === "ready" && !unresolved && <Button variant="primary" loading={busy === "import"}
      disabled={mutationDisabled || Boolean(busy || pending)} onClick={() => void flow.run({ action: "import" })}>{t("保存到凭证池", "Save to credential pool")}</Button>}
    {active && !unresolved && status !== "importing" && <Button loading={busy === "cancel"} disabled={Boolean(busy)}
      onClick={() => void flow.run({ action: "cancel" })}>{t("取消授权", "Cancel authorization")}</Button>}
    {uncertain && !flow.error && <Notice tone="warning">{t("保存响应未确认；仅检查原会话，不重复提交。", "Save response unconfirmed; check the original session only. Do not resubmit.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
  </section>;
}
