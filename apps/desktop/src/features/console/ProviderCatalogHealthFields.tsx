import type { Dispatch, SetStateAction } from "react";
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import type { SiteHealthConfig } from "./siteHealth";
import { catalogHealthAdapter } from "./providerCatalogWorkflow";
import { officialHealthAdapter, officialHealthAdapters, selectHealthAdapter } from "./officialHealthAdapters";
import { gatewayHealthAdapters } from "./gatewayHealthAdapters";
import { CatalogField, type CatalogFieldsProps } from "./ProviderCatalogFields";

export function ProviderCatalogHealthFields({ t, workflow, update, locked, template, draft, setDraft }: CatalogFieldsProps & {
  template: ProviderCatalogTemplate; draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>;
}) {
  const health = workflow.health;
  const preset = catalogHealthAdapter(template);
  const fixed = !template.custom;
  const official = officialHealthAdapter(health.adapter);
  const change = (patch: Partial<SiteHealthConfig>) => update({ health: { ...health, ...patch } });
  const disabled = locked || !health.enabled;
  const clearToken = () => setDraft((current) => ({ ...current, secrets: Object.fromEntries(Object.entries(current.secrets ?? {}).filter(([key]) => key !== "site_health/auth/token")) }));
  const number = (key: "minimum_success_rate" | "refresh_interval_secs" | "max_age_secs", name: string, min: number, max: number) => <CatalogField name={name}>
    <input className="nt-input" type="number" min={min} max={max} value={health[key]} disabled={disabled} onChange={(event) => change({ [key]: Number(event.target.value) })} />
  </CatalogField>;
  return <div className="nt-catalog-settings-grid">
    <CatalogField name={t("站点健康", "Site health")}><select className="nt-select" disabled={locked || !preset} value={String(health.enabled)}
      onChange={(event) => { change({ enabled: event.target.value === "true" }); if (event.target.value === "false") clearToken(); }}><option value="false">{t("关闭", "Off")}</option><option value="true">{t("开启", "On")}</option></select></CatalogField>
    <CatalogField name={t("检测范围", "Probe scope")}><input className="nt-input" readOnly value={t("整个服务商共享检测", "Shared across provider")} /></CatalogField>
    <CatalogField name={t("适配器", "Adapter")}>{fixed ? <input className="nt-input" readOnly value={preset ?? t("未匹配", "No matching adapter")} /> :
      <select className="nt-select" disabled={disabled} value={health.adapter} onChange={(event) => {
        update({ health: { ...health, ...selectHealthAdapter(health, event.target.value as SiteHealthConfig["adapter"]) } }); clearToken();
      }}><option value="auto">{t("自动（公开接口）", "Automatic (public endpoints)")}</option>
        <optgroup label={t("网关系统", "Gateway systems")}>{Object.entries(gatewayHealthAdapters).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</optgroup>
        <optgroup label={t("官方状态", "Official status")}>{Object.entries(officialHealthAdapters).map(([id, item]) => <option key={id} value={id}>{item.name}</option>)}</optgroup>
      </select>}
    </CatalogField>
    <CatalogField name={t("监控站地址", "Monitor URL")}><input className="nt-input" type="url" readOnly={fixed || Boolean(official)} disabled={!fixed && disabled}
      value={official?.url ?? health.site_url ?? ""} onChange={(event) => { change({ site_url: event.target.value || null }); clearToken(); }} /></CatalogField>
    <CatalogField name={t("控制策略", "Control policy")}><select className="nt-select" disabled={disabled} value={health.mode} onChange={(event) => change({ mode: event.target.value as SiteHealthConfig["mode"] })}>
      <option value="advisory">{t("仅提示", "Advisory")}</option><option value="restrict">{t("低于阈值时禁用模型", "Restrict below threshold")}</option></select></CatalogField>
    {number("minimum_success_rate", t("最低成功率 %", "Minimum success rate %"), 0, 100)}
    {number("refresh_interval_secs", t("刷新间隔（秒）", "Refresh interval (seconds)"), 30, 86400)}
    {number("max_age_secs", t("信息有效期（秒）", "Maximum age (seconds)"), 30, 604800)}
    <CatalogField name={t("未知或过期信息", "Unknown or stale data")}><select className="nt-select" disabled={disabled} value={health.unknown_policy} onChange={(event) => change({ unknown_policy: event.target.value as SiteHealthConfig["unknown_policy"] })}>
      <option value="allow">{t("放行", "Allow")}</option><option value="block">{t("阻止", "Block")}</option></select></CatalogField>
    {!fixed && <>
      <CatalogField name={t("监控分组", "Monitor groups")}><input className="nt-input" disabled={disabled || Boolean(official)} value={health.monitor_groups.join(", ")} onChange={(event) => change({ monitor_groups: event.target.value.split(",").map((entry) => entry.trim()).filter(Boolean) })} /></CatalogField>
      <CatalogField name={t("监控认证", "Monitor authentication")}><select className="nt-select" disabled={disabled || Boolean(official) || health.adapter === "auto"} value={health.auth.kind}
        onChange={(event) => { change({ auth: { kind: event.target.value as SiteHealthConfig["auth"]["kind"], header: "x-api-key", token: null } }); clearToken(); }}>
        <option value="none">{t("无需认证", "None")}</option><option value="bearer">Bearer</option><option value="api_key">API key</option></select></CatalogField>
      <CatalogField name={t("认证 Header", "Authentication header")}><select className="nt-select" disabled={disabled || health.auth.kind !== "api_key"} value={health.auth.header ?? "x-api-key"}
        onChange={(event) => { change({ auth: { ...health.auth, header: event.target.value } }); clearToken(); }}><option value="x-api-key">x-api-key</option><option value="api-key">api-key</option></select></CatalogField>
      <CatalogField name={t("监控密钥", "Monitor token")}><input type="password" autoComplete="new-password" className="nt-input" disabled={disabled || health.auth.kind === "none"}
        value={draft.secrets?.["site_health/auth/token"] ?? ""} onChange={(event) => setDraft((current) => ({ ...current, secrets: { ...current.secrets, "site_health/auth/token": event.target.value } }))} /></CatalogField>
      <CatalogField name={t("允许内网地址", "Allow private addresses")}><input type="checkbox" disabled={disabled || Boolean(official)} checked={health.allow_private_network} onChange={(event) => change({ allow_private_network: event.target.checked })} /></CatalogField>
    </>}
  </div>;
}
