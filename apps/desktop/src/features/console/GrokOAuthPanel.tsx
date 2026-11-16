import { useEffect, useState } from "react";
import { Button, Field, Input, Notice, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { GrokOAuthFields, emptyGrokOAuthFields } from "./GrokOAuthFields";
import { useGrokOAuth, type GrokOAuthProps } from "./useGrokOAuth";

export function GrokOAuthPanel(props: GrokOAuthProps) {
  const { t } = useUiLocale();
  const [authorizationFlow, setAuthorizationFlow] = useState<"code" | "device" | "loopback">("code");
  const device = authorizationFlow === "device";
  const flow = useGrokOAuth(props, device);
  const [fields, setFields] = useState(emptyGrokOAuthFields);
  const [callbackUrl, setCallbackUrl] = useState("");
  useEffect(() => { setFields(emptyGrokOAuthFields); setCallbackUrl(""); setAuthorizationFlow("code"); }, [props.client, props.managementToken, props.providerId, props.credentialId]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const status = session?.status;
  const messages: Record<string, string> = {
    waiting_user: device ? t("在浏览器完成设备授权后，点击检查授权。", "Complete device authorization in the browser, then check authorization.") : session?.automaticCallback ? t("等待本机授权回调。", "Waiting for a local authorization callback.") : t("等待手动授权回调。", "Waiting for a manual authorization callback."),
    exchanging: t("正在交换授权材料。", "Exchanging authorization material."),
    ready: t("授权已完成，请明确确认保存到凭据池。", "Authorization ready. Explicitly confirm saving to the credential pool."),
    claimed: t("服务端正在处理授权材料。", "Server is processing authorization material."),
    importing: t("正在提交保存；尚未确认成功。", "Saving; success is not yet confirmed."),
    succeeded: t("凭据保存已确认。", "Credential save confirmed."),
    save_unconfirmed: t("保存结果未确认。请核对当前配置与事务恢复结果；不要重复创建或提交账号。", "Save unconfirmed. Check current configuration and transaction recovery; do not create or submit duplicate accounts."),
    failed: t("登录失败；没有确认保存成功。", "Sign-in failed; no save was confirmed."),
    cancelled: t("登录已取消。", "Sign-in cancelled."), expired: t("登录会话已过期。", "Sign-in session expired."),
  };
  return <section className="n-stack" aria-label="Grok Build OAuth">
    <p className="n-muted">{t("Grok Build 的 OAuth 凭据池，与 xAI API Key、Grok Web Cookie 分开；额度关系未知，不合并显示。", "Grok Build OAuth has a separate credential pool from xAI API Key and Grok Web Cookie. Quota sharing is unknown and is not combined.")}</p>
    {props.credentialId && <p>{t("重新授权目标账号", "Reauthorization target")}: <code>{props.credentialId}</code></p>}
    <Field label={t("授权方式", "Authorization flow")}><Select value={authorizationFlow} disabled={locked || mutationDisabled} onChange={(event) => setAuthorizationFlow(event.target.value as "code" | "device" | "loopback")}>
      <option value="code">{t("代码授权（手动回调）", "Code (manual callback)")}</option><option value="device">{t("设备授权", "Device authorization")}</option>
      <option value="loopback">{t("代码授权（本机自动回调）", "Code (local automatic callback)")}</option>
    </Select></Field>
    <GrokOAuthFields value={fields} disabled={locked} onChange={setFields} device={device} />
    <Button variant={active ? "secondary" : "primary"} loading={busy === "create"} disabled={mutationDisabled || locked || !fields.clientId.trim() || !fields.scopes.trim() || (device ? Boolean(fields.selectionId.trim()) : !fields.redirectUri.trim())}
      onClick={() => void flow.run({
        action: "create", input: {
          providerId: props.providerId, expectedRevision: props.expectedRevision, credentialId: props.credentialId,
          clientId: fields.clientId.trim(), scopes: fields.scopes.trim().split(/\s+/),
          ...(device ? { authorizationFlow: "device" as const } : { redirectUri: fields.redirectUri.trim(), ...(authorizationFlow === "loopback" ? { authorizationFlow: "loopback" as const } : {}) }),
          ...(fields.selectionId.trim() ? { principalSelection: { kind: fields.selectionKind, id: fields.selectionId.trim() } } : {}),
          ...(fields.requiredId.trim() ? { requiredPrincipal: { kind: fields.requiredKind, id: fields.requiredId.trim() } } : {}),
        }
      })}>{t("通过 Grok Build 登录", "Sign in with Grok Build")}</Button>
    {(props.disabled || !props.expectedRevision) && <Notice role="note">{t("请先保存服务商及当前配置，再开始授权。", "Save the provider and current configuration before authorization.")}</Notice>}
    {stale && <Notice tone="warning" role="alert">{t("配置版本已变化；不会自动更换版本继续授权或保存。已接纳的保存仍可检查结果。", "Configuration revision changed. Authorization and saving will not rebase. An accepted save can still be checked.")}</Notice>}
    {pending && <Notice>{t("正在恢复敏感信息访问权限；仅续接本次明确操作。", "Restoring secret access; only this explicit action will resume.")}</Notice>}
    {status && <Notice tone={status === "succeeded" ? "success" : status === "save_unconfirmed" ? "warning" : "info"}>{messages[status]}</Notice>}
    {session && status === "waiting_user" && <>
      {session.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind={session.deviceAuthorization ? "grok-device" : "grok"} busy={Boolean(busy)} />}
      <Field label={t("授权链接（可手动复制）", "Authorization URL (copy manually)")}>
        <Input readOnly value={session.authorizationUrl ?? ""} />
      </Field>
      {session.deviceAuthorization ? <>
        <Field label={t("用户授权码", "User code")}><Input readOnly value={session.deviceAuthorization.userCode} /></Field>
        <p className="n-muted">{t("上游轮询间隔", "Upstream polling interval")}: {session.deviceAuthorization.pollIntervalSecs}s · {t("剩余有效期", "Remaining lifetime")}: {session.deviceAuthorization.expiresInSecs}s</p>
        <Button variant="primary" loading={busy === "poll"} disabled={mutationDisabled || Boolean(busy || pending) || session.deviceAuthorization.pollAfterSecs > 0} onClick={() => void flow.run({ action: "poll" })}>{t("检查授权", "Check authorization")}</Button>
        {session.deviceAuthorization.pollAfterSecs > 0 && <p className="n-muted">{t("可再次检查授权", "Check again in")}: {session.deviceAuthorization.pollAfterSecs}s</p>}
      </> : session.automaticCallback ? <Notice>{t("请在运行 Gateway 的同一台电脑上完成浏览器授权。", "Complete browser authorization on the same computer running Gateway.")}</Notice> : <>
      <p className="n-muted">{t("完成授权后，从浏览器地址栏复制完整回调地址，即使页面无法连接。没有自动回调监听。", "After consent, copy the complete callback URL from the browser address bar, even if the page cannot connect. There is no automatic callback listener.")}</p>
      <Field label={t("完整回调地址", "Complete callback URL")}>
        <Input autoComplete="off" type="password" value={callbackUrl} maxLength={24_000} disabled={Boolean(busy || pending)} onChange={(event) => setCallbackUrl(event.target.value)} />
      </Field>
      <Button variant="primary" loading={busy === "complete"} disabled={mutationDisabled || Boolean(busy || pending) || !callbackUrl}
        onClick={() => { void flow.run({ action: "complete", callbackUrl }); setCallbackUrl(""); }}>{t("完成授权", "Complete authorization")}</Button>
      </>}
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
