import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { GatewayApiError } from "../../api/errors";
import { createQuickAuthApi, type QuickAuthInput, type QuickAuthSession } from "../../api/console/quick-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { useManagedOAuth, type ManagedOAuthProps, type OAuthOperations } from "./useManagedOAuth";

export type QuickOAuthOperations = OAuthOperations<QuickAuthSession>;
export type QuickOAuthProps = ManagedOAuthProps<QuickAuthSession>;

const QUICK_POLICY = {
  polling: (status: string) => ["waiting_user", "exchanging", "importing"].includes(status),
  rejected: ["quick_oauth_login_rejected", "quick_oauth_login_save_rejected", "console_revision_conflict"],
  unavailable: (cause: unknown) =>
    cause instanceof GatewayApiError && cause.code === "quick_oauth_login_rejected" && cause.message.includes("unavailable"),
} as const;

export function QuickOAuthPanel(props: QuickOAuthProps & { onActivityChange?(active: boolean): void }) {
  const { t } = useUiLocale();
  const flow = useManagedOAuth<QuickAuthInput, QuickAuthSession>(props, createQuickAuthApi, QUICK_POLICY);
  const [callbackUrl, setCallbackUrl] = useState("");

  useEffect(() => {
    setCallbackUrl("");
  }, [props.client, props.managementToken, props.providerId, props.credentialId]);

  const { session, busy, pending, active, uncertain, stale } = flow;
  const unresolved = uncertain || session?.status === "save_unconfirmed";
  const locked = Boolean(busy || pending || active || unresolved);

  useEffect(() => {
    props.onActivityChange?.(locked);
    return () => props.onActivityChange?.(false);
  }, [props.onActivityChange, locked]);

  const mutationDisabled = props.disabled || stale || !props.expectedRevision || !props.managementToken;
  const messages: Record<string, string> = {
    waiting_user:
      session?.step === "authorize"
        ? t(
            "步骤 2/2：请在 AWS IAM Identity Center 页面批准授权；本机浏览器将自动完成回调，远程浏览器可粘贴回调 URL。",
            "Step 2/2: Approve access in AWS IAM Identity Center; local browsers finish automatically, or paste the callback URL manually.",
          )
        : t(
            "步骤 1/2：请打开 Amazon Quick 登录页面；本机浏览器将自动跳转并完成两步授权，远程浏览器可粘贴回调 URL。",
            "Step 1/2: Open Amazon Quick sign-in; local browsers redirect and finish automatically, or paste the callback URL manually.",
          ),
    exchanging: t("正在交换并校验 Amazon Quick OAuth 凭证。", "Exchanging and validating Amazon Quick OAuth tokens."),
    ready: t("授权已就绪，尚未写入配置；请确认保存到凭证池。", "Authorization is ready; click Save to commit the credential."),
    importing: t("正在保存凭证；重新打开面板不会重复提交。", "Saving credential; closing this dialog will not submit twice."),
    succeeded: t("Amazon Quick 凭证已保存。", "Amazon Quick credential saved."),
    save_unconfirmed: t(
      "保存回执未确认，请核对当前配置与恢复状态，不要重复提交。",
      "Save receipt unconfirmed; verify route configuration before retrying.",
    ),
    failed: t("Amazon Quick 授权失败，未写入任何凭证。", "Amazon Quick authorization failed; nothing was saved."),
    cancelled: t("授权已取消。", "Authorization cancelled."),
    expired: t("授权会话已过期，请重新发起登录。", "Authorization session expired; start sign-in again."),
  };

  return (
    <section className="n-stack" aria-label="Amazon Quick OAuth">
      <p>Amazon Quick · quick.aws.com (Social OAuth)</p>
      <Notice>
        {t(
          "仅使用社交账号动态注册公共 OIDC 客户端（quicksight:read_write）。本机浏览器自动完成 127.0.0.1 回调；无桌面/远程环境可手动粘贴两步回调地址。",
          "Registers a dynamic public OIDC client (quicksight:read_write) via social sign-in. Local browsers finish automatically via 127.0.0.1; remote browsers can paste callback URLs.",
        )}
      </Notice>
      <Button
        variant="primary"
        loading={busy === "create"}
        disabled={mutationDisabled || locked}
        onClick={() =>
          void flow.run({
            action: "create",
            input: {
              providerId: props.providerId,
              expectedRevision: props.expectedRevision,
              credentialId: props.credentialId,
            },
          })
        }
      >
        {t("通过 Amazon Quick 登录", "Sign in with Amazon Quick")}
      </Button>
      {(props.disabled || !props.expectedRevision) && (
        <Notice>{t("请先保存服务商及当前配置，再发起授权。", "Save the provider and current configuration before signing in.")}</Notice>
      )}
      {stale && (
        <Notice tone="warning">
          {t("配置版本已变化，请重新开始授权；已提交保存仍可继续查询。", "Configuration revision changed; restart sign-in.")}
        </Notice>
      )}
      {pending && <Notice>{t("正在恢复敏感信息访问权限…", "Restoring secret access…")}</Notice>}
      {session && (
        <Notice tone={session.status === "succeeded" ? "success" : unresolved ? "warning" : "info"}>
          {messages[session.status]}
        </Notice>
      )}
      {session?.status === "waiting_user" && (
        <>
          {session.authorizationUrl && (
            <ChatgptAuthorizationLink value={session.authorizationUrl} kind="quick" busy={Boolean(busy)} />
          )}
          <Field label={t("当前授权链接（可手动复制）", "Current authorization URL (copy manually)")}>
            <Input readOnly value={session.authorizationUrl ?? ""} />
          </Field>
          <Field
            label={t(
              "完整回调地址（仅远程/手动回退时粘贴 http://127.0.0.1:...）",
              "Complete callback URL (paste http://127.0.0.1:... for manual fallback only)",
            )}
          >
            <Input
              type="password"
              autoComplete="off"
              value={callbackUrl}
              maxLength={16384}
              placeholder={`${session.redirectUri}/callback?nonce=...`}
              disabled={mutationDisabled || Boolean(busy || pending)}
              onChange={(e) => setCallbackUrl(e.target.value)}
            />
          </Field>
          <Button
            loading={busy === "complete"}
            disabled={
              mutationDisabled ||
              Boolean(busy || pending) ||
              !callbackUrl.trim() ||
              new TextEncoder().encode(callbackUrl).length > 16384
            }
            onClick={() => {
              void flow.run({ action: "complete", callbackUrl: callbackUrl.trim() });
              setCallbackUrl("");
            }}
          >
            {t("提交回调地址", "Submit callback URL")}
          </Button>
        </>
      )}
      {session?.status === "ready" && (
        <>
          {session.accountSummary && (
            <Notice>{t(`已识别账号：${session.accountSummary}`, `Authenticated account: ${session.accountSummary}`)}</Notice>
          )}
          {!unresolved && (
            <Button
              variant="primary"
              loading={busy === "import"}
              disabled={mutationDisabled || Boolean(busy || pending)}
              onClick={() => void flow.run({ action: "import" })}
            >
              {t("保存到凭证池", "Save to credential pool")}
            </Button>
          )}
        </>
      )}
      {active && !unresolved && session?.status !== "importing" && (
        <Button disabled={Boolean(busy)} onClick={() => void flow.run({ action: "cancel" })}>
          {t("取消登录", "Cancel sign-in")}
        </Button>
      )}
      {uncertain && (
        <Notice tone="warning">
          {t(
            "保存响应未确认，可能已被接纳；请仅检查当前会话状态，勿重复提交。",
            "Save response lost; check the current session without resubmitting.",
          )}
        </Notice>
      )}
      {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
      {flow.error && <Notice tone="danger">{flow.error}</Notice>}
    </section>
  );
}
