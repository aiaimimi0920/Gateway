import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { validDimRedirect, type DimSite } from "../../api/console/dim-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { useDimOAuth, type DimOAuthProps } from "./useDimOAuth";

export function DimOAuthPanel(props: DimOAuthProps & { site: DimSite; onActivityChange?(active: boolean): void }) {
  const { t } = useUiLocale();
  const flow = useDimOAuth(props);
  const [clientId, setClientId] = useState("");
  const [redirectUri, setRedirectUri] = useState("");
  const [scopesText, setScopesText] = useState("");
  const [callbackUrl, setCallbackUrl] = useState("");
  useEffect(() => { setClientId(""); setRedirectUri(""); setScopesText(""); setCallbackUrl(""); }, [props.client, props.managementToken, props.providerId, props.credentialId, props.site]);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [props.onActivityChange, locked]);
  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const scopes = scopesText.trim().split(/\s+/);
  const atom = (s: string, max: number) => s.length > 0 && s.length <= max && /^[\x21-\x7e]+$/.test(s);
  const valid = atom(clientId, 512) && validDimRedirect(redirectUri) && scopes.includes("openid") && scopes.length <= 32
    && new Set(scopes).size === scopes.length && scopes.every((s) => atom(s, 128) && !/["\\]/.test(s));
  const messages: Record<string, string> = {
    waiting_user: t("等待浏览器回调；仅监听已明确登记的本机端口。", "Waiting for the browser callback on the explicitly registered local port."),
    exchanging: t("正在交换并验证授权材料。", "Exchanging and validating authorization material."),
    ready: t("授权材料已就绪，尚未保存；请明确确认保存。", "Authorization ready but not saved; explicitly confirm saving."),
    importing: t("正在保存；关闭面板不会重复提交。", "Saving; reopening does not submit again."),
    succeeded: t("凭据保存已确认。", "Credential save confirmed."),
    save_unconfirmed: t("保存结果未确认，请核对配置和事务恢复结果，不要重复提交。", "Save unconfirmed; check configuration and transaction recovery without resubmitting."),
    failed: t("授权失败，没有确认保存成功。", "Authorization failed; no save was confirmed."),
    cancelled: t("授权已取消。", "Authorization cancelled."), expired: t("授权已过期。", "Authorization expired."),
  };
  return <section className="n-stack" aria-label="DIM OAuth">
    <p>{props.site === "china" ? "DIM China · dimagent.cn" : "DIM Global · dimagent.com"}</p>
    <Notice>{t("两站账号与订阅独立。请使用允许 Gateway 使用的 public client；不内置或借用参考插件的注册身份。", "Sites and subscriptions are separate. Use a public client authorized for Gateway; no borrowed registration is built in.")}</Notice>
    <Field label="Client ID"><Input value={clientId} maxLength={512} disabled={locked || mutationDisabled} onChange={(e) => setClientId(e.target.value)} /></Field>
    <Field label={t("已登记的回调地址", "Registered redirect URI")}><Input value={redirectUri} maxLength={512} disabled={locked || mutationDisabled} placeholder="http://127.0.0.1:PORT/oauth/callback" onChange={(e) => setRedirectUri(e.target.value)} /></Field>
    <Field label="Scopes"><Input value={scopesText} maxLength={4096} disabled={locked || mutationDisabled} placeholder="openid …" onChange={(e) => setScopesText(e.target.value)} /></Field>
    <Button variant="primary" loading={busy === "create"} disabled={mutationDisabled || locked || !valid} onClick={() => void flow.run({ action: "create", input: {
      providerId: props.providerId, expectedRevision: props.expectedRevision, credentialId: props.credentialId, site: props.site, clientId, redirectUri, scopes,
    } })}>{t("通过 DIM 登录", "Sign in with DIM")}</Button>
    {(props.disabled || !props.expectedRevision) && <Notice>{t("请先保存服务商及当前配置，再开始授权。", "Save the provider and current configuration before authorization.")}</Notice>}
    {stale && <Notice tone="warning">{t("配置版本已变化，不会自动更换版本继续授权或保存；已接纳保存仍可检查。", "Revision changed; authorization and saving do not rebase. Accepted saves can still be checked.")}</Notice>}
    {pending && <Notice>{t("正在恢复敏感信息访问权限，仅续接本次明确操作。", "Restoring secret access for this explicit action only.")}</Notice>}
    {session && <Notice tone={session.status === "succeeded" ? "success" : unresolved ? "warning" : "info"}>{messages[session.status]}</Notice>}
    {session?.status === "waiting_user" && <>
      {session.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind={props.site === "china" ? "dim-china" : "dim-global"} busy={Boolean(busy)} />}
      <Field label={t("授权链接（可手动复制）", "Authorization URL (copy manually)")}><Input readOnly value={session.authorizationUrl ?? ""} /></Field>
      <Field label={t("完整回调地址（仅手动回退）", "Complete callback URL (manual fallback only)")}><Input type="password" autoComplete="off" value={callbackUrl} maxLength={16384} disabled={mutationDisabled || Boolean(busy || pending)} onChange={(e) => setCallbackUrl(e.target.value)} /></Field>
      <Button disabled={mutationDisabled || Boolean(busy || pending) || !callbackUrl || new TextEncoder().encode(callbackUrl).length > 16384}
        onClick={() => { void flow.run({ action: "complete", callbackUrl }); setCallbackUrl(""); }}>{t("完成授权", "Complete authorization")}</Button>
    </>}
    {session?.status === "ready" && <>
      <Notice>{session.identityAssurance === "signed_id_token" ? t("已通过 ID token 签名验证。", "ID token signature verified.") : t("身份来自 userinfo endpoint；未通过 ID token 验签，不冒充签名身份。", "Identity comes from userinfo; no signed ID token assurance is claimed.")}</Notice>
      <Notice>{t("保存时拉取当前账号目录并保存协议声明，不调用模型、不自动测试、不代表真实验收。目录读取失败不会写入账号。", "Saving reads this account's directory and saves wire declarations only; no generation, automatic test or verification claim. A failed directory read does not save the account.")}</Notice>
      {!unresolved && <Button variant="primary" disabled={mutationDisabled || Boolean(busy || pending)} onClick={() => void flow.run({ action: "import" })}>{t("拉取目录并保存到凭证池", "Read directory and save account")}</Button>}
    </>}
    {active && !unresolved && session?.status !== "importing" && <Button disabled={Boolean(busy)} onClick={() => void flow.run({ action: "cancel" })}>{t("取消登录", "Cancel login")}</Button>}
    {uncertain && <Notice tone="warning">{t("响应丢失，保存可能已被接纳；只检查同一会话，不自动重试提交。", "Response lost; the save may be accepted. Only check the same session; do not retry submission.")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
  </section>;
}
