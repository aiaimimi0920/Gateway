import { useEffect } from "react";
import { Button, ConfirmDialog, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";
import { useFreebuffClaim } from "./useFreebuffClaim";
import { FreebuffRunFacts } from "./FreebuffRunFacts";
type Props = FreebuffCatalogProps & { target: FreebuffBusinessTarget; onBusyChange(busy: boolean): void };
/** Local journal is independent of live catalog/model availability, never ownership of a foreign session. */
export function FreebuffClaimPanel(props: Props) {
  const { t } = useUiLocale(); const flow = useFreebuffClaim(props, props.target);
  useEffect(() => props.onBusyChange(flow.busy), [flow.busy, props.onBusyChange]);
  useEffect(() => () => props.onBusyChange(false), [props.onBusyChange]);
  return <section className="nt-stack" aria-label={t("FreeBuff 本地会话", "FreeBuff local claims")}>
    <h3>{t("本地会话与结束记录", "Local claims and closeouts")}</h3>
    <div><Button compact disabled={!flow.enabled || flow.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("检查本地会话", "Check local claims")}</Button>
      {flow.busy && <Button compact onClick={flow.cancel}>{t("停止会话操作等待", "Stop waiting for claim operation")}</Button>}
      {flow.value?.claim && <Button compact disabled={!flow.enabled || flow.busy} onClick={flow.prepare}>{t("结束原会话", "End original claim")}</Button>}</div>
    {flow.error && <Notice tone="danger">{t(flow.error, "Claim operation is unconfirmed. Check the local claim and closeout history.")}</Notice>}
    {flow.value && <div role="status">{flow.value.claim ? <>
      <div>{t("本地待处理会话", "Unresolved local claim")}：<strong>{flow.value.claim.modelKey}</strong></div><code>{flow.value.claim.attemptId}</code>
      <FreebuffRunFacts run={flow.value.claim.run} />
    </> : t("无本地待处理会话", "No unresolved local claim")}</div>}
    {flow.ended && <Notice tone={flow.ended.localReleased ? "success" : "warning"}>{flow.ended.localReleased
      ? t(`原会话已收尾，最终退款：${flow.ended.receipt.refund} Freebucks`, `Original claim closed; final refund: ${flow.ended.receipt.refund} Freebucks`)
      : t("结束结果尚未最终确认，原记录已保留。", "End is not final; the original claim was retained.")}</Notice>}
    {!!flow.value?.history.length && <ul aria-label={t("结束记录", "Closeout history")}>{flow.value.history.map(row => <li key={row.attemptId}>
      <strong>{row.modelKey}</strong> · <code>{row.attemptId}</code> · {t("最终退款", "Final refund")}：{row.receipt.refund} Freebucks
      <FreebuffRunFacts run={row.run} />
    </li>)}</ul>}
    {flow.value?.nextCursor && <Button compact disabled={!flow.enabled || flow.busy} onClick={() => void flow.read(flow.value!.nextCursor)}>{t("更早的结束记录", "Earlier closeouts")}</Button>}
    <ConfirmDialog open={!!flow.approved} title={t("确认结束原会话", "Confirm original claim end")}
      description={t(`账号 ${props.credentialId} · ${props.target.origin} · ${props.target.client} · ${flow.approved?.modelKey ?? ""} · ${flow.approved?.attemptId ?? ""}：仅结束这个原始会话，不保证退款。停止等待不会撤销远端结束；结果未知时先检查记录，不自动重投。`,
        `Account ${props.credentialId} · ${props.target.origin} · ${props.target.client} · ${flow.approved?.modelKey ?? ""} · ${flow.approved?.attemptId ?? ""}: ends only this original claim, without a refund guarantee. Stopping the wait cannot undo remote end; inspect uncertain results before another explicit attempt.`)}
      danger confirmLabel={t("确认结束", "Confirm end")} cancelLabel={t("取消", "Cancel")} confirmDisabled={!flow.enabled || flow.busy || !flow.approved}
      onOpenChange={open => { if (!open) flow.dismiss(); }} onConfirm={() => void flow.confirm()} />
  </section>;
}
