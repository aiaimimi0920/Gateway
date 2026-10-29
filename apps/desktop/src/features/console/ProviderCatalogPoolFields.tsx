import { useState, type Dispatch, type SetStateAction } from "react";
import type { ConsoleCredentialPoolAutomationDriver } from "../../api/contracts";
import type { ProviderCatalogDraft } from "./providerCatalog";
import { compatibleCatalogDrivers } from "./providerCatalogWorkflow";
import { storageConnectionLabel, type StorageTarget } from "./providerStorageConnection";
import { CatalogField, type CatalogFieldsProps } from "./ProviderCatalogFields";
import { ProviderCatalogStorageFields } from "./ProviderCatalogStorageFields";

export function ProviderCatalogPoolFields(props: CatalogFieldsProps & {
  draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>; drivers: readonly ConsoleCredentialPoolAutomationDriver[];
}) {
  const { t, workflow, update, locked, draft, drivers, setDraft } = props;
  const [target, setTarget] = useState<StorageTarget | null>(null);
  const compatible = compatibleCatalogDrivers(drivers, draft.providerId);
  const driver = compatible.length === 1 ? compatible[0].id : compatible.find((entry) => entry.id === workflow.driverId)?.id ?? "";
  const toggle = (name: string, key: "autoRefill" | "autoPrune" | "permanentDelete", disabled = false) => <CatalogField name={name}>
    <select className="nt-select" value={String(workflow[key] && !disabled)} disabled={locked || disabled} onChange={(event) => update({ [key]: event.target.value === "true" })}>
      <option value="false">{t("关闭", "Off")}</option><option value="true">{t("开启", "On")}</option>
    </select>
  </CatalogField>;
  if (target) return <ProviderCatalogStorageFields {...props} target={target} onBack={() => setTarget(null)} />;
  return <div className="nt-catalog-settings-grid">
    <CatalogField name={t("号池驱动", "Pool driver")} wide>{compatible.length <= 1 ? <input className="nt-input" readOnly value={driver || t("未配置", "Not configured")} /> :
      <select className="nt-select" value={driver} disabled={locked} onChange={(event) => update({ driverId: event.target.value })}>
        <option value="">{t("请选择", "Select")}</option>{compatible.map((entry) => <option key={entry.id} value={entry.id}>{entry.id}</option>)}
      </select>}</CatalogField>
    <CatalogField name={t("最小可用池", "Minimum pool size")}><input className="nt-input" type="number" min={0} value={workflow.poolMin} disabled={locked} onChange={(event) => update({ poolMin: Number(event.target.value) })} /></CatalogField>
    <CatalogField name={t("最大可用池", "Maximum pool size")}><input className="nt-input" type="number" min={1} value={workflow.poolMax} disabled={locked} onChange={(event) => update({ poolMax: Number(event.target.value) })} /></CatalogField>
    {toggle(t("自动补号", "Automatic refill"), "autoRefill", !driver)}
    {toggle(t("自动清理失效号", "Prune invalid credentials"), "autoPrune", !driver)}
    {toggle(t("彻底删除", "Permanent deletion"), "permanentDelete")}
    <CatalogField name={t("补号存储密码", "Refill storage password")}><input className="nt-input" type="password" autoComplete="new-password" disabled={locked}
      value={draft.secrets?.credential_storage_password ?? ""} onChange={(event) => setDraft((current) => ({ ...current, secrets: { ...current.secrets, credential_storage_password: event.target.value } }))} /></CatalogField>
    {(["storage", "archive"] as const).map((key) => <div key={key} className="nt-catalog-field"><span>{key === "storage" ? t("存储连接", "Storage connection") : t("归档连接", "Archive connection")}</span>
      <div className="nt-catalog-inline"><span title={storageConnectionLabel(workflow[key], "")}>{storageConnectionLabel(workflow[key], t("未配置", "Not configured"))}</span>
        <button type="button" className="nt-btn nt-btn--outline" disabled={locked} onClick={() => setTarget(key)}>{t("配置", "Configure")}</button></div>
    </div>)}
    <p className="nt-catalog-risk" role={workflow.permanentDelete ? "status" : undefined}>{workflow.permanentDelete ? t("彻底删除开启后，清理失效账号不保留归档。", "Permanent deletion prunes credentials without an archive.") : ""}</p>
  </div>;
}
