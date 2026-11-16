import { useEffect, useState } from "react";
import { Button, Checkbox, ConfirmDialog, Field, Notice, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useFreebuffCatalog, type FreebuffCatalogProps } from "./useFreebuffCatalog";
import { FreebuffDeviceTools } from "./FreebuffDeviceTools";
import { FreebuffSessionPanel } from "./FreebuffSessionPanel";
import { FreebuffClaimPanel } from "./FreebuffClaimPanel";
import { FreebuffPurchasePanel } from "./FreebuffPurchasePanel";
import { useFreebuffDeviceRegistration } from "./useFreebuffDeviceRegistration";
import type { FreebuffBusinessTarget } from "./freebuffBusiness";
import type { FreebuffCatalog } from "./freebuffCatalog";
import { validModelId } from "./providerModelId";
import "./FreebuffCatalogPanel.css";
export type { FreebuffCatalogProps } from "./useFreebuffCatalog";
type Props = FreebuffCatalogProps & { savedBusiness?: FreebuffBusinessTarget; savedModelKeys?: string[]; onApply?(catalog: FreebuffCatalog, keys: string[]): boolean };
/** No automatic network or publication: application only stages an ordinary account edit. */
export function FreebuffCatalogPanel(props: Props) {
  const { t } = useUiLocale();
  const [claimBusy, setClaimBusy] = useState(false);
  const [purchaseBusy, setPurchaseBusy] = useState(false);
  const [origin, setOrigin] = useState(props.savedBusiness?.origin ?? ""); const [client, setClient] = useState(props.savedBusiness?.client ?? "");
  useEffect(() => { setOrigin(props.savedBusiness?.origin ?? ""); setClient(props.savedBusiness?.client ?? ""); }, [props.savedBusiness?.origin, props.savedBusiness?.client]);
  const flow = useFreebuffCatalog(props, origin, client);
  const device = useFreebuffDeviceRegistration(props, origin, client, flow.busy);
  const [selection, setSelection] = useState<{ catalog: FreebuffCatalog; keys: string[] } | null>(null);
  const selected = selection?.catalog === flow.catalog ? selection.keys : [];
  const [approved, setApproved] = useState<typeof selection>(null);
  const canApply = !props.disabled && !flow.busy && !device.busy && !claimBusy && !purchaseBusy && !!props.secretGrant;
  const confirmation = canApply && approved?.catalog === flow.catalog && approved === selection ? approved : null;
  return <section className="nt-stack" aria-label={t("FreeBuff 模型目录", "FreeBuff model catalog")}>
    {props.savedBusiness && <FreebuffClaimPanel {...props} target={props.savedBusiness} onBusyChange={setClaimBusy}
      disabled={props.disabled || purchaseBusy || flow.busy || device.busy || origin !== props.savedBusiness.origin || client !== props.savedBusiness.client} />}
    {props.savedBusiness && <FreebuffPurchasePanel {...props} target={props.savedBusiness} onBusyChange={setPurchaseBusy}
      disabled={props.disabled || claimBusy || flow.busy || device.busy || origin !== props.savedBusiness.origin || client !== props.savedBusiness.client} />}
    {props.savedBusiness && !!props.savedModelKeys?.length && <FreebuffSessionPanel {...props} target={props.savedBusiness} modelKeys={props.savedModelKeys}
      disabled={props.disabled || claimBusy || purchaseBusy || flow.busy || device.busy || origin !== props.savedBusiness.origin || client !== props.savedBusiness.client} />}
    <h3>{t("FreeBuff 模型目录", "FreeBuff model catalog")}</h3>
    <Field label={t("业务站点", "Business site")}><Select compact value={origin} disabled={props.disabled || claimBusy || purchaseBusy || flow.busy || device.busy} onChange={(e) => setOrigin(e.target.value)}>
      <option value="">{t("请选择", "Select")}</option><option value="https://codebuff.com">https://codebuff.com</option>
      <option value="https://www.codebuff.com">https://www.codebuff.com</option>
    </Select></Field>
    <Field label={t("客户端", "Client")}><Select compact value={client} disabled={props.disabled || claimBusy || purchaseBusy || flow.busy || device.busy} onChange={(e) => setClient(e.target.value)}>
      <option value="">{t("请选择", "Select")}</option><option value="cli">CLI</option><option value="desktop">Desktop</option>
    </Select></Field>
    <p className="n-muted">{t("读取将向所选业务站点发送此账号令牌；仅查看目录，不购买会话或修改路由。", "Reading sends this account token to the selected site; no session purchase or route changes.")}</p>
    <div><Button compact disabled={!flow.enabled || claimBusy || purchaseBusy || flow.busy || device.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("读取目录", "Read catalog")}</Button>
      {flow.busy && <Button compact onClick={flow.cancel}>{t("取消读取", "Cancel read")}</Button>}</div>
    {props.disabled && <Notice>{t("请先保存或放弃当前修改。", "Save or discard current changes first.")}</Notice>}
    {flow.error && <Notice tone="danger">{t(flow.error, "Catalog read failed or did not match this account. Check access and target, then retry.")}</Notice>}
    <FreebuffDeviceTools props={props} origin={origin} client={client} catalogBusy={flow.busy || claimBusy || purchaseBusy} flow={device} onRegister={flow.clear} />
    {flow.catalog && <ul className="nt-freebuff-catalog" aria-label={t("目录结果", "Catalog results")}>
      {flow.catalog.models.map((row) => <li key={row.key}>
        {props.onApply && <Checkbox label={t(`选择 ${row.key}`, `Select ${row.key}`)} checked={selected.includes(row.key)}
          disabled={props.disabled || claimBusy || purchaseBusy || flow.busy || device.busy || row.access !== "open" || !validModelId(row.key)}
          onChange={(event) => setSelection({ catalog: flow.catalog!, keys: event.target.checked ? [...selected, row.key] : selected.filter((key) => key !== row.key) })} />}
        <strong>{row.displayName}</strong> <code>{row.key}</code>
        <div>{row.access === "locked" ? t("已锁定", "Locked") : t("开放", "Open")} · {row.dataUse === "training" ? t("用于训练", "Used for training") : t("仅服务用途", "Service use")}
          {row.premium ? ` · ${t("Premium", "Premium")}` : ""}{row.multimodal ? ` · ${t("多模态", "Multimodal")}` : ""}</div>
        {row.tagline && <div>{row.tagline}</div>}{row.warning && <Notice tone="warning">{row.warning}</Notice>}
        {row.badges.length > 0 && <div>{row.badges.map((badge) => badge.label).join(" · ")}</div>}
      </li>)}
    </ul>}
    {flow.catalog && props.onApply && <Button compact disabled={!canApply || !selected.length}
      onClick={() => setApproved(selection)}>{t("应用到账号", "Apply to account")}</Button>}
    <ConfirmDialog open={!!confirmation} title={t("确认目录模式", "Confirm catalog mode")}
      description={t(`账号 ${props.credentialId} · ${origin} · ${client}：将把业务站点和所选模型写入账号表单，仍需保存账号；本操作不购买会话，实际调用仍需逐次批准。`,
        `Account ${props.credentialId} · ${origin} · ${client}: stages this business site and selected models; save the account to persist them. This does not purchase a session; actual calls still require per-request approval.`)}
      confirmLabel={t("确认应用", "Confirm application")} cancelLabel={t("取消", "Cancel")}
      confirmDisabled={!confirmation} onOpenChange={(open) => { if (!open) setApproved(null); }}
      onConfirm={() => { if (confirmation && props.onApply?.(confirmation.catalog, confirmation.keys)) { setApproved(null); flow.clear(); } }} />
  </section>;
}
