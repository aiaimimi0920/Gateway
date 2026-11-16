import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { createNousAuthApi, type NousAuthSession } from "../../api/console/nous-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";
export type NousOAuthOperations = OAuthOperations<NousAuthSession>;
const policy = {
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "importing"].includes(status), devicePolling: true,
  rejected: ["console_secret_access_required", "nous_login_save_rejected", "nous_login_identity_mismatch", "console_revision_conflict", "console_mutation_not_supported", "nous_login_rejected", "invalid_nous_portal_oauth", "gateway_provider_line_compiled_out"],
};
export function NousOAuthPanel(props: ManagedOAuthProps<NousAuthSession> & { onActivityChange?(active: boolean): void }) {
  const { t } = useUiLocale();
  const flow = useManagedOAuth(props, createNousAuthApi, policy);
  const [clientId, setClientId] = useState("");
  useEffect(() => { setClientId(""); }, [props.client, props.managementToken, props.providerId, props.credentialId]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [props.onActivityChange, locked]);
  const disabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const valid = clientId.length > 0 && clientId.length <= 512 && /^[\x21-\x7e]+$/.test(clientId);
  const messages: Record<string, string> = {
    waiting_user: t("等待设备授权", "Waiting for device authorization"), exchanging: t("正在检查授权", "Checking authorization"),
    ready: t("授权就绪，尚未保存", "Authorization ready, not saved"), importing: t("正在保存", "Saving"),
    succeeded: t("账号已保存", "Account saved"), save_unconfirmed: t("保存结果未确认，请核对配置；勿重复提交。", "Save unconfirmed. Check configuration; do not resubmit."),
    failed: t("授权未通过，未保存账号", "Authorization not admitted; account not saved"), cancelled: t("已取消", "Cancelled"), expired: t("授权已过期", "Authorization expired"),
  };
  return <section className="nt-stack" aria-label="Nous Portal OAuth">
    <Field label={t("获准的 Client ID", "Authorized Client ID")}><Input value={clientId} maxLength={512} disabled={locked || disabled} onChange={(e) => setClientId(e.target.value)} /></Field>
    <Button variant={active ? "secondary" : "primary"} loading={busy === "create"} disabled={disabled || locked || !valid}
      onClick={() => void flow.run({ action: "create", input: { providerId: props.providerId, credentialId: props.credentialId, expectedRevision: props.expectedRevision, clientId } })}>
      {t("通过 Nous 登录", "Sign in with Nous")}</Button>
    {(props.disabled || !props.expectedRevision) && <Notice>{t("请先保存当前配置。", "Save the current configuration first.")}</Notice>}
    {stale && <Notice tone="warning">{t("配置版本已变化，请重新授权；已接纳保存仍可查询。", "Revision changed. Restart authorization; accepted saves remain queryable.")}</Notice>}
    {pending && <Notice>{t("等待敏感信息访问确认", "Waiting for secret access consent")}</Notice>}
    {session && <Notice tone={session.status === "succeeded" ? "success" : unresolved ? "warning" : "info"}>{messages[session.status]}</Notice>}
    {session?.status === "waiting_user" && <>
      {session.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind="nous" busy={Boolean(busy)} />}
      <Field label={t("授权链接", "Authorization URL")}><Input readOnly value={session.authorizationUrl ?? ""} /></Field>
      <Field label={t("用户授权码", "User code")}><Input readOnly value={session.deviceAuthorization?.userCode ?? ""} /></Field>
      <Button variant="primary" disabled={disabled || Boolean(busy || pending) || !session.deviceAuthorization || session.deviceAuthorization.pollAfterSecs > 0}
        onClick={() => void flow.run({ action: "poll" })}>{t("检查授权", "Check authorization")}</Button>
    </>}
    {session?.status === "ready" && !unresolved && <Button variant="primary" disabled={disabled || Boolean(busy || pending)} onClick={() => void flow.run({ action: "import" })}>{t("保存到凭证池", "Save to credential pool")}</Button>}
    {active && !unresolved && session?.status !== "importing" && <Button disabled={Boolean(busy)} onClick={() => void flow.run({ action: "cancel" })}>{t("取消登录", "Cancel login")}</Button>}
    {uncertain && <Notice tone="warning">{t("响应未确认，仅查询原保存结果，不自动重试。", "Response unconfirmed. Check the original save; no automatic retry.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
  </section>;
}
