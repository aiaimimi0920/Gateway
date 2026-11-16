import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { KiroOAuthFields, emptyKiroOAuthFields, kiroDirectory, validKiroOAuthFields } from "./KiroOAuthFields";
import { useKiroOAuth, type KiroOAuthProps } from "./useKiroOAuth";

export function KiroOAuthPanel(props: KiroOAuthProps) {
  const { t } = useUiLocale();
  const flow = useKiroOAuth(props);
  const [fields, setFields] = useState(emptyKiroOAuthFields);
  useEffect(() => { setFields(emptyKiroOAuthFields); }, [props.client, props.managementToken, props.providerId, props.credentialId]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [props.onActivityChange, locked]);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const status = session?.status;
  const messages: Record<string, string> = {
    registering: t("正在注册本次授权客户端。", "Registering this authorization client."),
    waiting_user: t("等待浏览器授权；完成后点击检查授权。", "Awaiting browser authorization; check authorization when done."),
    polling: t("正在检查授权，遵守上游轮询间隔。", "Checking authorization with the upstream polling interval."),
    ready: t("授权材料已取得；请确认保存。订阅和模型权益尚未验证。", "Authorization material received; confirm saving. Subscription and model access remain unverified."),
    importing: t("正在提交保存；尚未确认成功。", "Saving; success is not yet confirmed."),
    succeeded: t("凭据保存已确认。", "Credential save confirmed."),
    save_unconfirmed: t("保存结果未确认；请核对当前配置与事务恢复结果，不要重复提交。", "Save unconfirmed; check current configuration and transaction recovery, without resubmitting."),
    failed: t("授权失败；没有确认保存成功。", "Authorization failed; no save was confirmed."),
    cancelled: t("登录已取消。", "Sign-in cancelled."), expired: t("登录会话已过期。", "Sign-in session expired."),
  };
  const verification = session?.verificationUriComplete ?? session?.verificationUri;
  return <section className="n-stack" aria-label="Kiro OAuth">
    {props.credentialId && <p>{t("重新授权目标账号", "Reauthorization target")}: <code>{props.credentialId}</code></p>}
    <KiroOAuthFields value={fields} disabled={locked || mutationDisabled} editing={Boolean(props.credentialId)} onChange={setFields} />
    <Button variant={active ? "secondary" : "primary"} loading={busy === "create"}
      disabled={mutationDisabled || locked || !validKiroOAuthFields(fields, Boolean(props.credentialId))}
      onClick={() => void flow.run({ action: "create", input: {
        providerId: props.providerId, expectedRevision: props.expectedRevision, credentialId: props.credentialId,
        directory: kiroDirectory(fields), scopes: fields.scopes, ...(props.credentialId ? {} : { machineId: fields.machineId }),
      } })}>{t("通过 Kiro 登录", "Sign in with Kiro")}</Button>
    {(props.disabled || !props.expectedRevision) && <Notice role="note">{t("请先保存服务商及当前配置，再开始授权。", "Save the provider and configuration before authorization.")}</Notice>}
    {stale && <Notice tone="warning" role="alert">{t("配置版本已变化；不自动更换版本。已接纳的保存仍可检查结果。", "Configuration revision changed; no automatic rebase. An accepted save can still be checked.")}</Notice>}
    {pending && <Notice>{t("正在恢复敏感信息访问权限；仅续接本次明确操作。", "Restoring secret access; only this explicit action will resume.")}</Notice>}
    {status && <Notice tone={status === "succeeded" ? "success" : status === "save_unconfirmed" ? "warning" : "info"}>{messages[status]}</Notice>}
    {session && ["waiting_user", "polling"].includes(session.status) && <>
      {verification && <ChatgptAuthorizationLink value={verification} kind="kiro" busy={Boolean(busy)} />}
      <Field label={t("授权链接（可手动复制）", "Authorization URL (copy manually)")}><Input readOnly value={verification ?? ""} /></Field>
      <Field label={t("用户授权码", "User code")}><Input readOnly value={session.userCode ?? ""} /></Field>
      <p className="n-muted">{t("上游轮询间隔", "Upstream polling interval")}: {session.pollIntervalSecs ?? "—"}s · {t("剩余有效期", "Remaining lifetime")}: {session.expiresInSecs ?? "—"}s</p>
      <Button variant="primary" loading={busy === "poll"} disabled={mutationDisabled || Boolean(busy || pending) || status !== "waiting_user"}
        onClick={() => void flow.run({ action: "poll" })}>{t("检查授权", "Check authorization")}</Button>
    </>}
    {status === "ready" && !unresolved && <Button variant="primary" loading={busy === "import"} disabled={mutationDisabled || Boolean(busy || pending)}
      onClick={() => void flow.run({ action: "import" })}>{t("保存到凭证池", "Save to credential pool")}</Button>}
    {active && !unresolved && status !== "importing" && <Button loading={busy === "cancel"} disabled={Boolean(busy)} onClick={() => void flow.run({ action: "cancel" })}>{t("取消登录", "Cancel login")}</Button>}
    {uncertain && <Notice tone="warning" role="alert">{t("响应丢失，保存可能已被接纳；仅检查同一会话，不自动重试提交。", "Response lost; saving may have been accepted. Only check the same session, never resubmit automatically.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
  </section>;
}
