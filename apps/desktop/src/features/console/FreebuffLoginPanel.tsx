import { useEffect } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { useFreebuffLogin, type FreebuffLoginProps } from "./useFreebuffLogin";

/** Approval and explicit server-side saving never pass tokens through the manual account form. */
export function FreebuffLoginPanel(props: FreebuffLoginProps) {
  const { t } = useUiLocale();
  const flow = useFreebuffLogin(props);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const status = session?.status;
  const unresolved = uncertain || status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [props.onActivityChange, locked]);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const messages: Record<string, string> = {
    starting: t("正在创建登录链接。", "Creating sign-in link."),
    waiting_user: t("等待浏览器批准；完成后点击检查授权。", "Awaiting browser approval; check authorization when done."),
    polling: t("正在检查授权。", "Checking authorization."),
    ready: t("登录材料已取得，尚未保存。", "Sign-in material received, not saved yet."),
    importing: t("正在保存，尚未确认成功。", "Saving; success is not yet confirmed."),
    succeeded: t("凭据保存已确认。", "Credential save confirmed."),
    save_unconfirmed: t("保存结果未确认；请核对当前配置与事务恢复结果，不要重复提交。", "Save unconfirmed; check current configuration and transaction recovery without resubmitting."),
    failed: t("登录失败；没有确认保存成功。", "Sign-in failed; no save was confirmed."),
    cancelled: t("登录已取消。", "Sign-in cancelled."), expired: t("登录会话已过期。", "Sign-in session expired."),
  };
  return <section className="n-stack" aria-label="FreeBuff 登录">
    {props.credentialId && <p>{t("重新授权目标账号", "Reauthorization target")}: <code>{props.credentialId}</code></p>}
    <Button variant={active ? "secondary" : "primary"} loading={busy === "create"}
      disabled={mutationDisabled || locked} onClick={() => void flow.run({ action: "create", input: {
        providerId: props.providerId, expectedRevision: props.expectedRevision, credentialId: props.credentialId,
      } })}>{t("通过 FreeBuff 登录", "Sign in with FreeBuff")}</Button>
    {(props.disabled || !props.expectedRevision) && <Notice>{t("请先保存服务商及当前配置。", "Save the provider and current configuration first.")}</Notice>}
    {stale && <Notice tone="warning">{t("配置版本已变化；仅可检查已接纳保存的结果，不自动更换版本。", "Configuration changed; accepted saves can still be checked, without rebasing.")}</Notice>}
    {pending && <Notice>{t("正在恢复敏感信息访问权限。", "Restoring secret access.")}</Notice>}
    {status && (!uncertain || status === "save_unconfirmed") && <Notice tone={status === "succeeded" ? "success" : status === "save_unconfirmed" ? "warning" : status === "failed" ? "danger" : "info"}>{messages[status]}</Notice>}
    {session && ["waiting_user", "polling"].includes(session.status) && <>
      {session.loginUrl && <ChatgptAuthorizationLink value={session.loginUrl} kind="freebuff" busy={Boolean(busy)} />}
      <Field label={t("授权链接（可手动复制）", "Authorization URL (copy manually)")}><Input readOnly value={session.loginUrl ?? ""} /></Field>
      <p className="n-muted">{t("检查间隔", "Check interval")}: {session.pollIntervalSecs ?? "—"}s · {t("剩余有效期", "Remaining lifetime")}: {session.expiresInSecs ?? "—"}s</p>
      <Button variant="primary" loading={busy === "poll"} disabled={mutationDisabled || Boolean(busy || pending) || status !== "waiting_user"}
        onClick={() => void flow.run({ action: "poll" })}>{t("检查授权", "Check authorization")}</Button>
    </>}
    {status === "ready" && !unresolved && <Button variant="primary" loading={busy === "import"} disabled={mutationDisabled || Boolean(busy || pending)}
      onClick={() => void flow.run({ action: "import" })}>{t("保存到凭证池", "Save to credential pool")}</Button>}
    {active && !unresolved && status !== "importing" && <Button loading={busy === "cancel"} disabled={Boolean(busy)}
      onClick={() => void flow.run({ action: "cancel" })}>{t("取消登录", "Cancel login")}</Button>}
    {uncertain && !flow.error && status !== "save_unconfirmed" && <Notice tone="warning">{t("保存响应丢失；只检查同一会话，不会自动重新提交。", "Save response lost; only check the same session, without resubmitting.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && status !== "save_unconfirmed" && <Notice tone={unresolved ? "warning" : "danger"}>{flow.error}</Notice>}
  </section>;
}
