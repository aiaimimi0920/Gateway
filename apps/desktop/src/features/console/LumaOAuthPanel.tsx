import { useEffect, useState } from "react";
import { Button, Checkbox, Field, Input, Notice } from "@neuro/ui";
import { validLumaRedirect } from "../../api/console/luma-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import type { LumaRegistration } from "./lumaMcp";
import { useLumaOAuth, type LumaOAuthProps } from "./useLumaOAuth";
import { LumaMcpToolsPanel } from "./LumaMcpToolsPanel";
import { LumaModelsPanel } from "./LumaModelsPanel";
import { LumaRegistrationPanel } from "./LumaRegistrationPanel";
import { LumaRefreshPanel } from "./LumaRefreshPanel";

export function LumaOAuthPanel(props: LumaOAuthProps & { registration?: LumaRegistration; onActivityChange?(active: boolean): void }) {
  const { t } = useUiLocale();
  const flow = useLumaOAuth(props);
  const [clientId, setClientId] = useState(props.registration?.clientId ?? "");
  const [redirectUri, setRedirectUri] = useState(props.registration?.redirectUri ?? "");
  const [callbackUrl, setCallbackUrl] = useState("");
  const [offlineAccess, setOfflineAccess] = useState(props.registration?.offlineAccess ?? false);
  const [refreshActive, setRefreshActive] = useState(false);
  useEffect(() => {
    setClientId(props.registration?.clientId ?? ""); setRedirectUri(props.registration?.redirectUri ?? ""); setCallbackUrl("");
    setOfflineAccess(props.registration?.offlineAccess ?? false);
  }, [props.client, props.managementToken, props.providerId, props.credentialId, props.registration?.clientId, props.registration?.redirectUri, props.registration?.offlineAccess]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const loginLocked = Boolean(busy || pending || active || unresolved);
  const locked = loginLocked || refreshActive;
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [props.onActivityChange, locked]);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const valid = clientId.length > 0 && clientId.length <= 512 && /^[\x21-\x7e]+$/.test(clientId) && validLumaRedirect(redirectUri);
  const messages: Record<string, string> = {
    waiting_user: t("等待授权回调", "Waiting for authorization"), exchanging: t("正在完成授权", "Completing authorization"),
    ready: t("授权完成，尚未保存", "Authorized, not saved"), importing: t("正在保存", "Saving"),
    succeeded: t("账号已保存", "Account saved"), save_unconfirmed: t("保存结果未确认，请核对配置，不要重复提交。", "Save unconfirmed. Check configuration; do not resubmit."),
    failed: t("授权失败", "Authorization failed"), cancelled: t("授权已取消", "Authorization cancelled"), expired: t("授权已过期", "Authorization expired"),
  };
  return <section className="n-stack" aria-label="Luma MCP OAuth">
    <Checkbox label={t("允许刷新账号授权", "Allow account authorization refresh")} checked={offlineAccess}
      disabled={locked || mutationDisabled || Boolean(props.credentialId && !props.registration?.offlineAccess)} onChange={(event) => setOfflineAccess(event.target.checked)} />
    {!props.credentialId && <LumaRegistrationPanel {...props} offlineAccess={offlineAccess} disabled={Boolean(mutationDisabled || locked)}
      onRegistered={(value) => { setClientId(value.clientId); setRedirectUri(value.redirectUri); setOfflineAccess(value.offlineAccess ?? false); }} />}
    <Field label="Client ID"><Input value={clientId} maxLength={512} disabled={locked || mutationDisabled || Boolean(props.credentialId)} onChange={(e) => setClientId(e.target.value)} /></Field>
    <Field label={t("已登记的回调地址", "Registered redirect URI")}><Input value={redirectUri} maxLength={512} disabled={locked || mutationDisabled || Boolean(props.credentialId)} placeholder="http://127.0.0.1:PORT/callback" onChange={(e) => setRedirectUri(e.target.value)} /></Field>
    {!active && !unresolved && session?.status !== "succeeded" && <Button variant="primary" loading={busy === "create"} disabled={mutationDisabled || locked || !valid}
      onClick={() => void flow.run({ action: "create", input: { providerId: props.providerId, expectedRevision: props.expectedRevision,
        credentialId: props.credentialId, clientId, redirectUri, ...(offlineAccess ? { offlineAccess: true } : {}) } })}>{t("通过 Luma 登录", "Sign in with Luma")}</Button>}
    {(props.disabled || !props.expectedRevision) && <Notice>{t("请先保存当前配置。", "Save the current configuration first.")}</Notice>}
    {stale && <Notice tone="warning">{t("配置版本已变化；请核对配置。", "Configuration changed; review it before continuing.")}</Notice>}
    {pending && <Notice>{t("等待敏感信息访问确认", "Waiting for secret access confirmation")}</Notice>}
    {session && <Notice tone={session.status === "succeeded" ? "success" : unresolved ? "warning" : "info"}>{messages[session.status]}</Notice>}
    {session?.status === "waiting_user" && <>
      {session.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind="luma" busy={Boolean(busy)} />}
      <Field label={t("授权链接", "Authorization URL")}><Input readOnly value={session.authorizationUrl ?? ""} /></Field>
      <Notice>{t("请在 Gateway 所在电脑完成授权。", "Complete authorization on the computer running Gateway.")}</Notice>
      <Field label={t("回调地址（手动完成）", "Callback URL (manual completion)")}><Input type="password" autoComplete="off" value={callbackUrl} maxLength={16384} disabled={mutationDisabled || Boolean(busy || pending)} onChange={(e) => setCallbackUrl(e.target.value)} /></Field>
      <Button disabled={mutationDisabled || Boolean(busy || pending) || !callbackUrl || new TextEncoder().encode(callbackUrl).length > 16384}
        onClick={() => { void flow.run({ action: "complete", callbackUrl }); setCallbackUrl(""); }}>{t("完成授权", "Complete authorization")}</Button>
    </>}
    {session?.status === "ready" && !unresolved && <Button variant="primary" disabled={mutationDisabled || Boolean(busy || pending)}
      onClick={() => void flow.run({ action: "import" })}>{t("保存只读 MCP 账号", "Save read-only MCP account")}</Button>}
    {active && !unresolved && session?.status !== "importing" && <Button disabled={Boolean(busy || pending)} onClick={() => void flow.run({ action: "cancel" })}>{t("取消授权", "Cancel authorization")}</Button>}
    {uncertain && <Notice tone="warning">{t("保存响应未确认；仅检查状态，不会重复提交。", "Save response unconfirmed; checking status only, without resubmitting.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
    {props.credentialId && <LumaModelsPanel {...props} disabled={Boolean(mutationDisabled || locked)} />}
    {props.credentialId && <LumaMcpToolsPanel {...props} disabled={Boolean(mutationDisabled || locked)} />}
    {props.credentialId && props.registration?.offlineAccess && <LumaRefreshPanel {...props}
      disabled={Boolean(mutationDisabled || loginLocked)} onActivityChange={setRefreshActive} />}
  </section>;
}
