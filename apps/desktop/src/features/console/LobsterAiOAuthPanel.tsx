import { useEffect } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { createLobsterAiAuthApi, type LobsterAiAuthSession } from "../../api/console/lobsterai-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";
export type LobsterAiOAuthOperations = OAuthOperations<LobsterAiAuthSession>;
const policy = {
  polling: (status: string) => ["waiting_user", "exchanging", "ready", "importing"].includes(status),
  rejected: ["console_secret_access_required", "lobsterai_login_save_rejected", "lobsterai_login_identity_mismatch", "console_revision_conflict", "lobsterai_local_runtime_required", "lobsterai_login_rejected"],
};
export function LobsterAiOAuthPanel(props: ManagedOAuthProps<LobsterAiAuthSession> & { onActivityChange?(active: boolean): void }) {
  const { t } = useUiLocale();
  const flow = useManagedOAuth(props, createLobsterAiAuthApi, policy);
  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [props.onActivityChange, locked]);
  const disabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const messages: Record<string, string> = {
    waiting_user: t("等待本机浏览器授权", "Waiting for local browser authorization"), exchanging: t("正在完成授权", "Completing authorization"),
    ready: t("已授权，待保存", "Authorized, awaiting save"), importing: t("正在读取目录并保存", "Reading directory and saving"),
    succeeded: t("账号已保存", "Account saved"), save_unconfirmed: t("保存结果未确认，请核对账号列表", "Save unconfirmed; check the account list"),
    failed: t("授权失败，请重新登录", "Authorization failed; sign in again"), cancelled: t("已取消", "Cancelled"), expired: t("授权已过期", "Authorization expired"),
  };
  return <section className="n-stack" aria-label="LobsterAI">
    {!active && !unresolved && <Button variant="primary" loading={busy === "create"} disabled={disabled || locked}
      onClick={() => void flow.run({ action: "create", input: { providerId: props.providerId, expectedRevision: props.expectedRevision, credentialId: props.credentialId } })}>{t("通过 LobsterAI 登录", "Sign in with LobsterAI")}</Button>}
    {(props.disabled || !props.expectedRevision) && <Notice>{t("请先保存当前配置", "Save the current configuration first")}</Notice>}
    {stale && <Notice tone="warning">{t("配置已变化，请重新授权", "Configuration changed; authorize again")}</Notice>}
    {pending && <Notice>{t("等待敏感信息访问权限", "Waiting for secret access")}</Notice>}
    {session && <Notice tone={session.status === "succeeded" ? "success" : unresolved ? "warning" : session.status === "failed" ? "danger" : "info"}>{messages[session.status]}</Notice>}
    {session?.status === "waiting_user" && session.authorizationUrl && <>
      <ChatgptAuthorizationLink value={session.authorizationUrl} kind="lobsterai" busy={Boolean(busy)} />
      <Field label={t("授权链接", "Authorization URL")}><Input readOnly value={session.authorizationUrl} /></Field>
    </>}
    {session?.status === "ready" && !unresolved && <Button variant="primary" disabled={disabled || Boolean(busy || pending)}
      onClick={() => void flow.run({ action: "import" })}>{t("读取目录并保存账号", "Read directory and save account")}</Button>}
    {active && !unresolved && session?.status !== "importing" && <Button disabled={Boolean(busy)} onClick={() => void flow.run({ action: "cancel" })}>{t("取消登录", "Cancel login")}</Button>}
    {uncertain && <Notice tone="warning">{t("保存响应丢失，正在核对结果", "Save response lost; checking the result")}</Notice>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
  </section>;
}
