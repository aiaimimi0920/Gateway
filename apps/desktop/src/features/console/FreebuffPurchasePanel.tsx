import { useEffect } from "react";
import { Button, ConfirmDialog, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { FreebuffPendingPurchase } from "../../api/console/freebuff-purchases";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";
import { useFreebuffPurchases } from "./useFreebuffPurchases";
import "./FreebuffPurchasePanel.css";
type Props = FreebuffCatalogProps & { target: FreebuffBusinessTarget; onBusyChange(busy: boolean): void };

function Quote({ row }: { row: FreebuffPendingPurchase }) {
  const { t } = useUiLocale(); const { meter } = row.quote;
  const flag = (value: boolean | undefined) => value === undefined ? t("未知", "Unknown") : value ? t("是", "Yes") : t("否", "No");
  const amount = (value: number | undefined) => value === undefined ? t("未知", "Unknown") : `${value} Freebucks`;
  return <dl className="nt-freebuff-purchase-quote">
    <dt>{t("模型", "Model")}</dt><dd>{row.quote.modelKey}</dd>
    <dt>{t("会话价格", "Session price")}</dt><dd>{amount(meter.sessionPrice)}</dd>
    <dt>{t("总余额", "Balance")}</dt><dd>{amount(meter.balance)}</dd>
    <dt>{t("每日剩余", "Daily remaining")}</dt><dd>{amount(meter.dailyRemaining)}</dd>
    <dt>{t("钱包余额", "Wallet balance")}</dt><dd>{amount(meter.walletBalance)}</dd>
    <dt>{t("预计钱包支出", "Estimated wallet spend")}</dt><dd>{amount(row.quote.walletSpend)}</dd>
    <dt>{t("可领取赠额", "Claimable grant")}</dt><dd>{amount(meter.claimableGrant)}</dd>
    <dt>{t("Solo 折扣", "Solo discount")}</dt><dd>{amount(meter.soloDiscount)}</dd>
    <dt>{t("配额豁免", "Quota exempt")}</dt><dd>{flag(meter.quotaExempt)}</dd>
    <dt>{t("上游免询问标记", "Upstream spend-without-asking flag")}</dt><dd>{flag(meter.spendWithoutAsking)}</dd>
    <dt>{t("预计领取赠额", "Estimated grant claim")}</dt><dd>{flag(row.quote.claimEarned)}</dd>
    <dt>{t("请求 ID", "Request ID")}</dt><dd><code>{row.requestId}</code></dd>
    <dt>{t("调用方指纹", "Caller fingerprint")}</dt><dd><code>{row.callerFingerprint}</code></dd>
    <dt>{t("请求摘要", "Request SHA-256")}</dt><dd><code>{row.requestSha256}</code></dd>
  </dl>;
}

/** Normal saved-account UI only delivers a decision to the original live request; it never buys. */
export function FreebuffPurchasePanel(props: Props) {
  const { t } = useUiLocale(); const flow = useFreebuffPurchases(props, props.target);
  useEffect(() => props.onBusyChange(flow.busy), [flow.busy, props.onBusyChange]);
  useEffect(() => () => props.onBusyChange(false), [props.onBusyChange]);
  const approving = flow.selected?.decision === "approve";
  return <section className="nt-stack" aria-label={t("FreeBuff 待确认购买", "FreeBuff pending purchases")}>
    <h3>{t("待确认购买", "Pending purchases")}</h3>
    <div><Button compact disabled={!flow.enabled || flow.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("读取待确认购买", "Read pending purchases")}</Button>
      {flow.busy && <Button compact onClick={flow.cancel}>{t("停止购买确认等待", "Stop waiting for purchase decision")}</Button>}</div>
    {flow.feedback?.error && <Notice tone="danger">{t(flow.feedback.error, "Purchase decision is unconfirmed. Read pending purchases again; no automatic retry.")}</Notice>}
    {flow.feedback?.delivered && <Notice>{flow.feedback.delivered === "approve" ? t("批准决定已交付；购买与生成结果由原请求返回。", "Approval delivered; the original request reports purchase and generation results.")
      : t("取消决定已交付。", "Cancellation delivered.")}</Notice>}
    {flow.rows?.length === 0 && <div role="status">{t("无待确认购买", "No pending purchases")}</div>}
    {!!flow.rows?.length && <ul className="nt-freebuff-purchases" aria-label={t("待确认请求", "Pending requests")}>{flow.rows.map(row => <li key={row.pendingId}>
      <strong>{row.quote.modelKey}</strong> · {t("会话价格", "Session price")} {row.quote.meter.sessionPrice} Freebucks
      <div>{t("预计钱包支出", "Estimated wallet spend")} {row.quote.walletSpend} Freebucks · <code>{row.requestId}</code></div>
      <Button compact disabled={!flow.enabled || flow.busy} onClick={() => flow.prepare(row, "approve")}>{t("查看并批准", "Review and approve")}</Button>
      <Button compact disabled={!flow.enabled || flow.busy} onClick={() => flow.prepare(row, "cancel")}>{t("取消此次购买", "Cancel this purchase")}</Button>
    </li>)}</ul>}
    <ConfirmDialog open={!!flow.selected} title={approving ? t("确认本次购买", "Confirm this purchase") : t("取消此次购买", "Cancel this purchase")}
      description={t(`账号 ${props.credentialId} · ${props.target.origin} · ${props.target.client}。报价短时有效，不锁价；价格、余额或授权变化会拒绝购买。此处仅向原活请求交付决定，不表示购买或生成成功。`,
        `Account ${props.credentialId} · ${props.target.origin} · ${props.target.client}. Quotes expire and do not lock prices. Price, balance or authority changes reject purchase. This only delivers a decision to the original live request, not purchase or generation success.`)}
      danger={false} confirmLabel={approving ? t("批准此次购买", "Approve this purchase") : t("确认取消购买", "Confirm cancellation")}
      cancelLabel={t("返回", "Back")} confirmDisabled={!flow.enabled || flow.busy || !flow.selected}
      onOpenChange={open => { if (!open) flow.dismiss(); }} onConfirm={() => void flow.confirm()}>
      {flow.selected && <Quote row={flow.selected.row} />}
    </ConfirmDialog>
  </section>;
}
