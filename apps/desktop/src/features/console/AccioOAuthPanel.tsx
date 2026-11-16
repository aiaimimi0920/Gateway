import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { AccioOAuthFields, emptyAccioOAuthFields, validAccioOAuthFields } from "./AccioOAuthFields";
import { useAccioOAuth, type AccioOAuthProps } from "./useAccioOAuth";

export function AccioOAuthPanel(props: AccioOAuthProps) {
  const { t } = useUiLocale();
  const flow = useAccioOAuth(props);
  const [fields, setFields] = useState(emptyAccioOAuthFields);
  const [callbackUrl, setCallbackUrl] = useState("");
  useEffect(() => { setFields(emptyAccioOAuthFields); setCallbackUrl(""); }, [props.client, props.managementToken, props.providerId, props.credentialId]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  // Keep mode exits from bypassing an accepted/unknown save; closing the dialog remains allowed.
  useEffect(() => {
    props.onActivityChange?.(locked);
    return () => props.onActivityChange?.(false);
  }, [props.onActivityChange, locked]);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const status = session?.status;
  const messages: Record<string, string> = {
    waiting_user: t("等待手动授权回调。", "Waiting for a manual authorization callback."),
    exchanging: t("正在交换授权材料。", "Exchanging authorization material."),
    ready: t("授权已完成，请明确确认保存到凭据池。", "Authorization ready. Explicitly confirm saving to the credential pool."),
    importing: t("正在提交保存；尚未确认成功。", "Saving; success is not yet confirmed."),
    succeeded: t("凭据保存已确认。", "Credential save confirmed."),
    save_unconfirmed: t("保存结果未确认。请核对当前配置与事务恢复结果；不要重复创建或提交账号。", "Save unconfirmed. Check current configuration and transaction recovery; do not create or submit duplicate accounts."),
    failed: t("登录失败；没有确认保存成功。", "Sign-in failed; no save was confirmed."),
    cancelled: t("登录已取消。", "Sign-in cancelled."), expired: t("登录会话已过期。", "Sign-in session expired."),
  };
  return <section className="n-stack" aria-label="Accio OAuth">
    <p className="n-muted">{t("China/Global 与旧导入之间的额度关系未知，不合并余额。登录不证明与旧账号身份相同；新授权缺 SG 时会清除旧 SG，并替换旧账号上下文。", "Quota sharing between China/Global and legacy imports is unknown; balances are not combined. Sign-in does not prove the same account identity: a new grant without SG clears old SG and replaces old account context.")}</p>
    {props.credentialId && <p>{t("重新授权目标账号", "Reauthorization target")}: <code>{props.credentialId}</code></p>}
    <AccioOAuthFields value={fields} disabled={locked || mutationDisabled} editing={Boolean(props.credentialId)} onChange={setFields} />
    <Button variant={active ? "secondary" : "primary"} loading={busy === "create"}
      disabled={mutationDisabled || locked || !validAccioOAuthFields(fields, Boolean(props.credentialId))}
      onClick={() => {
        if (!fields.loginSite) return; void flow.run({
          action: "create", input: {
            providerId: props.providerId, expectedRevision: props.expectedRevision, credentialId: props.credentialId,
            clientId: fields.clientId, loginSite: fields.loginSite, callbackPort: Number(fields.callbackPort),
            ...(fields.utdid === "" ? {} : { utdid: fields.utdid }),
          }
        });
      }}>{t("通过 Accio 登录", "Sign in with Accio")}</Button>
    {(props.disabled || !props.expectedRevision) && <Notice role="note">{t("请先保存服务商及当前配置，再开始授权。", "Save the provider and current configuration before authorization.")}</Notice>}
    {stale && <Notice tone="warning" role="alert">{t("配置版本已变化；不会自动更换版本继续授权或保存。已接纳的保存仍可检查结果。", "Configuration revision changed. Authorization and saving will not rebase. An accepted save can still be checked.")}</Notice>}
    {pending && <Notice>{t("正在恢复敏感信息访问权限；仅续接本次明确操作。", "Restoring secret access; only this explicit action will resume.")}</Notice>}
    {status && <Notice tone={status === "succeeded" ? "success" : status === "save_unconfirmed" ? "warning" : "info"}>{messages[status]}</Notice>}
    {session && status === "waiting_user" && <>
      {session.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind="accio" busy={Boolean(busy)} />}
      <Field label={t("授权链接（可手动复制）", "Authorization URL (copy manually)")}>
        <Input readOnly value={session.authorizationUrl ?? ""} />
      </Field>
      <p className="n-muted">{t("从浏览器地址栏复制完整回调地址，即使页面无法连接；没有自动回调监听。", "Copy the complete callback URL from the browser address bar even if the page cannot connect; there is no automatic callback listener.")}</p>
      <Field label={t("完整回调地址", "Complete callback URL")}>
        <Input autoComplete="off" type="password" value={callbackUrl} maxLength={16_384}
          disabled={mutationDisabled || Boolean(busy || pending)} onChange={(event) => setCallbackUrl(event.target.value)} />
      </Field>
      <Button variant="primary" loading={busy === "complete"} disabled={mutationDisabled || Boolean(busy || pending) || !callbackUrl || new TextEncoder().encode(callbackUrl).length > 16_384}
        onClick={() => { void flow.run({ action: "complete", callbackUrl }); setCallbackUrl(""); }}>{t("完成授权", "Complete authorization")}</Button>
    </>}
    {status === "ready" && !unresolved && <Button variant="primary" loading={busy === "import"} aria-label={busy === "import" ? t("正在保存…", "Saving…") : undefined} disabled={mutationDisabled || Boolean(busy || pending)}
      onClick={() => void flow.run({ action: "import" })}>{t("保存到凭证池", "Save to credential pool")}</Button>}
    {active && !unresolved && status !== "importing" && <Button loading={busy === "cancel"} disabled={Boolean(busy)}
      onClick={() => void flow.run({ action: "cancel" })}>{t("取消登录", "Cancel login")}</Button>}
    {uncertain && <Notice tone="warning" role="alert">{t("响应丢失，保存可能已被接纳；仅检查同一会话，不自动重试提交。", "Response lost; saving may have been accepted. Only the same session is checked; submission is never retried automatically.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
  </section>;
}
