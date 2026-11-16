import { Button, Input, Notice, Select, Switch } from "@neuro/ui";
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
    <Switch checked={workflow[key] && !disabled} disabled={locked || disabled} onChange={(event) => update({ [key]: event.target.checked })} />
  </CatalogField>;
  if (target) return <ProviderCatalogStorageFields {...props} target={target} onBack={() => setTarget(null)} />;
  return <div className="nt-catalog-settings-grid">
    <CatalogField name={t("号池驱动", "Pool driver")} wide>{compatible.length <= 1 ? <Input readOnly value={driver || t("未配置", "Not configured")} /> :
      <Select value={driver} disabled={locked} onChange={(event) => update({ driverId: event.target.value })}>
        <option value="">{t("请选择", "Select")}</option>{compatible.map((entry) => <option key={entry.id} value={entry.id}>{entry.id}</option>)}
      </Select>}</CatalogField>
    <CatalogField name={t("最小可用池", "Minimum pool size")}><Input type="number" min={0} value={workflow.poolMin} disabled={locked} onChange={(event) => update({ poolMin: Number(event.target.value) })} /></CatalogField>
    <CatalogField name={t("最大可用池", "Maximum pool size")}><Input type="number" min={1} value={workflow.poolMax} disabled={locked} onChange={(event) => update({ poolMax: Number(event.target.value) })} /></CatalogField>
    {toggle(t("自动补号", "Automatic refill"), "autoRefill", !driver)}
    {toggle(t("自动清理失效号", "Prune invalid credentials"), "autoPrune", !driver)}
    {toggle(t("彻底删除", "Permanent deletion"), "permanentDelete")}
    <CatalogField name={t("补号存储密码", "Refill storage password")}><Input type="password" autoComplete="new-password" disabled={locked}
      value={draft.secrets?.credential_storage_password ?? ""} onChange={(event) => setDraft((current) => ({ ...current, secrets: { ...current.secrets, credential_storage_password: event.target.value } }))} /></CatalogField>
    {(["storage", "archive"] as const).map((key) => <div key={key} className="nt-catalog-connection"><span>{key === "storage" ? t("存储连接", "Storage connection") : t("归档连接", "Archive connection")}</span>
      <div className="nt-catalog-inline"><span title={storageConnectionLabel(workflow[key], "")}>{storageConnectionLabel(workflow[key], t("未配置", "Not configured"))}</span>
        <Button disabled={locked} onClick={() => setTarget(key)}>{t("配置", "Configure")}</Button></div>
    </div>)}
    <div className="nt-catalog-field--wide">{workflow.permanentDelete && <Notice tone="warning">{t("彻底删除开启后，清理失效账号不保留归档。", "Permanent deletion prunes credentials without an archive.")}</Notice>}</div>
  </div>;
}
