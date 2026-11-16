import { Button, Field, Input, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { LumaOAuthProps } from "./useLumaOAuth";
import type { LumaRegistration } from "./lumaMcp";
import { useLumaRegistration } from "./useLumaRegistration";

export function LumaRegistrationPanel(props: LumaOAuthProps & { offlineAccess?: boolean; onRegistered(value: LumaRegistration): void }) {
  const { t } = useUiLocale(); const flow = useLumaRegistration(props);
  const view = flow.receipt?.view;
  const labels = { prepared: t("待确认", "Awaiting confirmation"), registering: t("正在注册", "Registering"),
    registered: t("已登记", "Registered"), unconfirmed: t("结果未确认", "Unconfirmed"),
    cancelled: t("已取消准备", "Preparation cancelled"), expired: t("准备已过期", "Preparation expired") };
  const disabled = props.disabled || !props.managementToken || !props.expectedRevision || flow.busy;
  return <section className="n-stack" aria-label={t("Luma 客户端注册", "Luma client registration")}>
    {!view && <Button disabled={disabled} onClick={() => void flow.run("prepare")}>{t("准备独立客户端注册", "Prepare client registration")}</Button>}
    {view && <>
      <Field label={t("注册回调地址", "Registration redirect URI")}><Input readOnly value={view.redirectUri} /></Field>
      <Notice>{t("注册状态", "Registration status")}: {labels[view.status]}</Notice>
      {!flow.receipt?.submitted && (view.expectedRevision !== props.expectedRevision || ["expired", "cancelled"].includes(view.status))
        && <Button disabled={disabled} onClick={() => void flow.run("prepare")}>{t("重新准备注册", "Prepare registration again")}</Button>}
      {view.status === "prepared" && !flow.receipt?.submitted && <>
        <Notice tone="warning">{view.offlineAccess ? t("将注册支持刷新授权的独立客户端；不生成作品。", "Registers a separate refresh-capable client; no generation.") : t("将向 Luma 注册独立客户端；不生成作品、不读取余额。", "Registers a new client with Luma; no generation or balance reads.")}</Notice>
        <Button disabled={disabled || view.expectedRevision !== props.expectedRevision} onClick={() => void flow.run("start")}>{t("确认注册客户端", "Confirm client registration")}</Button>
      </>}
      {(flow.receipt?.submitted || view.status === "registering") && view.status !== "registered" && <>
        <Notice tone="warning">{t("仅查询此注册结果，不会重复注册。", "Only checks this registration; never resubmits it.")}</Notice>
        <Button disabled={flow.busy || !props.managementToken} onClick={() => void flow.run("read")}>{t("检查注册结果", "Check registration")}</Button>
      </>}
      {view.status === "registered" && view.clientId && <>
        <Field label={t("已登记 Client ID", "Registered Client ID")}><Input readOnly value={view.clientId} /></Field>
        <Button disabled={disabled} onClick={() => props.onRegistered({ clientId: view.clientId!, redirectUri: view.redirectUri, ...(view.offlineAccess ? { offlineAccess: true } : {}) })}>{t("使用此客户端授权", "Use client for authorization")}</Button>
      </>}
    </>}
    {flow.error && <Notice tone="danger">{t("注册响应未确认；请检查已有结果，不要重复注册。", "Registration response unconfirmed; check the existing result, do not register again.")}</Notice>}
  </section>;
}
