import { useEffect } from "react";
import { Button, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useLumaRefresh } from "./useLumaRefresh";
import type { LumaOAuthProps } from "./useLumaOAuth";

export function LumaRefreshPanel(props: LumaOAuthProps & { onActivityChange(active: boolean): void }) {
  const { t } = useUiLocale();
  const flow = useLumaRefresh(props), status = flow.session?.status;
  const active = flow.busy || flow.uncertain || status === "ready" || status === "importing" || status === "save_unconfirmed";
  useEffect(() => { props.onActivityChange(active); return () => props.onActivityChange(false); }, [active, props.onActivityChange]);
  const disabled = props.disabled || flow.busy || !props.managementToken || !props.expectedRevision;
  return <section className="n-stack" aria-label={t("Luma 账号刷新", "Luma account refresh")}>
    {(!flow.session || status === "succeeded") && <Button disabled={disabled} onClick={() => void flow.run("prepare")}>{t("刷新账号授权", "Refresh account authorization")}</Button>}
    {status === "ready" && !flow.uncertain && <>
      <Notice>{t("将刷新并保存此账号授权，不生成作品。", "Refreshes and saves this account authorization; no generation.")}</Notice>
      <Button disabled={disabled} onClick={() => void flow.run("import")}>{t("确认刷新授权", "Confirm authorization refresh")}</Button>
      <Button disabled={flow.busy} onClick={() => void flow.run("cancel")}>{t("取消刷新", "Cancel refresh")}</Button>
    </>}
    {status === "importing" && <Notice>{t("正在刷新并保存", "Refreshing and saving")}</Notice>}
    {status === "succeeded" && <Notice tone="success">{t("账号授权已更新", "Account authorization updated")}</Notice>}
    {(status === "save_unconfirmed" || flow.uncertain) && <>
      <Notice tone="warning">{t("刷新结果未确认，不会重新发送刷新请求。", "Refresh unconfirmed. The refresh request will not be resent.")}</Notice>
      <Button disabled={flow.busy} onClick={() => void flow.run("resume")}>{t("恢复保存", "Resume saving")}</Button>
    </>}
    {flow.pollStopped && <Button onClick={flow.resumePolling}>{t("继续检查状态", "Continue checking status")}</Button>}
    {flow.error && <Notice tone="danger">{t("操作未完成，请检查授权或继续核对状态。", "Operation incomplete. Check authorization or operation status.")}</Notice>}
  </section>;
}
