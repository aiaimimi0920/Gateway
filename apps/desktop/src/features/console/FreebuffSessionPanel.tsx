import { useState } from "react";
import { Button, Field, Notice, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { FreebuffCatalogProps } from "./useFreebuffCatalog";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";
import { useFreebuffInspection } from "./useFreebuffInspection";
type Props = FreebuffCatalogProps & { target: FreebuffBusinessTarget; modelKeys: string[] };
/** Only the persisted target/models are eligible, never the catalog tools' unsaved selection. */
export function FreebuffSessionPanel(props: Props) {
  const { t } = useUiLocale(); const [model, setModel] = useState("");
  const selected = props.modelKeys.includes(model) ? model : "";
  const flow = useFreebuffInspection(props, props.target, selected);
  const value = flow.value; const meter = value?.meter;
  return <section className="nt-stack" aria-label={t("FreeBuff 会话与余额", "FreeBuff session and balance")}>
    <h3>{t("会话与余额", "Session and balance")}</h3>
    <Field label={t("检查模型", "Model to inspect")}><Select compact value={selected} disabled={props.disabled || flow.busy} onChange={(e) => setModel(e.target.value)}>
      <option value="">{t("请选择", "Select")}</option>{props.modelKeys.map((key) => <option key={key} value={key}>{key}</option>)}
    </Select></Field>
    <div><Button compact disabled={!flow.enabled || flow.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("检查会话与余额", "Check session and balance")}</Button>
      {flow.busy && <Button compact onClick={flow.cancel}>{t("取消检查", "Cancel check")}</Button>}</div>
    {flow.error && <Notice tone="danger">{t("检查失败，请核对已保存账号、设备注册与权限后重试。", "Check failed. Verify the saved account, device registration and access, then retry.")}</Notice>}
    {value && <div role="status">
      <div>{value.session.status === "active" ? t(`检测到活动会话（未认领）：${value.session.modelKey}`, `Active session (not claimed): ${value.session.modelKey}`)
        : value.session.status === "none" ? t("无活动会话", "No active session")
          : value.session.inGrace ? t("会话已结束，处于收尾期（未认领）", "Session ended, in grace (not claimed)") : t("会话已结束", "Session ended")}</div>
      {value.session.status === "active" && <div>{t("到期时间", "Expires at")}：{value.session.expiresAt}</div>}
      {meter?.state === "absent" && <div>{t("未提供 Freebucks 余额", "Freebucks meter not provided")}</div>}
      {meter?.state === "unavailable" && <div>{t("Freebucks 余额暂不可用", "Freebucks meter unavailable")}</div>}
      {meter?.state === "available" && <>
        <div>{t("可花余额", "Spendable balance")}：{meter.balance} Freebucks</div>
        <div>{t("每日剩余", "Daily remaining")}：{meter.dailyRemaining} · {t("钱包余额", "Wallet balance")}：{meter.walletBalance}</div>
        <div>{meter.sessionPrice === null ? t("模型未提供 Freebucks 定价", "No Freebucks price for this model")
          : t(`新会话参考价：${meter.sessionPrice} Freebucks（非锁价或扣款凭据）`, `New-session estimate: ${meter.sessionPrice} Freebucks (not locked or a debit receipt)`)}</div>
      </>}
      <div>{t("检查时间", "Observed at")}：{value.observedAt}</div>
    </div>}
  </section>;
}
