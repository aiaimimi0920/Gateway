import type { ProbePolicy } from "./siteHealth";
import { officialHealthAdapter, officialHealthAdapters, selectHealthAdapter } from "./officialHealthAdapters";
import { gatewayHealthAdapters, gatewayHealthFamily, legacyHealthReader } from "./gatewayHealthAdapters";

type Props = {
  policy: ProbePolicy;
  onChange(value: ProbePolicy): void;
  t(zh: string, en: string): string;
};

/** Changing a family is explicit; merely displaying an old plan must not broaden its auth audience. */
export function SiteHealthAdapterFields({ policy, onChange, t }: Props) {
  const family = gatewayHealthFamily(policy.adapter);
  const legacy = legacyHealthReader(policy.adapter);
  const official = officialHealthAdapter(policy.adapter);
  const select = (adapter: ProbePolicy["adapter"]) => onChange(selectHealthAdapter(policy, adapter));
  return <>
    <label className="nt-field">
      <span>{t("适配器", "Adapter")}</span>
      <select className="nt-select" value={family}
        onChange={(event) => select(event.target.value as ProbePolicy["adapter"])}>
        <option value="auto">{t("自动（仅公开接口，不推断网关身份）", "Automatic (public capabilities, no gateway identity claim)")}</option>
        <optgroup label={t("网关系统", "Gateway systems")}>
          {Object.entries(gatewayHealthAdapters).map(([id, label]) => <option key={id} value={id}>
            {id === "compatible-gateway" ? t(label, "Compatible gateway (unconfirmed type)") : label}
          </option>)}
        </optgroup>
        <optgroup label={t("官方服务", "Official services")}>
          {Object.entries(officialHealthAdapters).map(([id, item]) => <option key={id} value={id}>{item.name}</option>)}
        </optgroup>
      </select>
    </label>
    <label className="nt-field">
      <span>{t("监控站地址（留空使用本站）", "Monitor URL (empty: this site)")}</span>
      <input className="nt-input" type="url" value={official?.url ?? policy.site_url ?? ""}
        readOnly={!!official} onChange={(event) => onChange({ ...policy, site_url: event.target.value || null })} />
    </label>
    {!official && <p className="nt-site-health__wide" role="note">
      {t("按网关系统复用适配器，内部识别有限的健康接口；具体读取器只在诊断中显示。没有模型健康接口时保留未知，不用站点存活冒充模型可用。", "Adapters are shared by gateway system and discover bounded health capabilities. Readers appear in diagnostics. Liveness is not model availability.")}
      {family === "one-api" && t(" One API 的模型健康依赖站点扩展，不保证原版提供。", " One API model health depends on optional site extensions; upstream support is not assumed.")}
    </p>}
    {legacy && <div className="nt-site-health__wide" role="note">
      <small>{t("兼容旧方案：保留已确认读取器和认证目标", "Legacy plan: retain the confirmed reader and auth destination")}: <code>{legacy}</code></small>
      <button type="button" className="nt-button" onClick={() => select(family)}>
        {t("改为按网关自动识别", "Use gateway capability discovery")}
      </button>
      <small>{t("切换后已有监控密钥需要重新替换或清空；不会自动扩大密钥使用范围。", "Switching requires replacing or clearing saved monitoring credentials; secret scope never expands implicitly.")}</small>
    </div>}
  </>;
}
