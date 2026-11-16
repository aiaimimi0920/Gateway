import { Checkbox, Field, Input, Select } from "@neuro/ui";
import type { ProbePolicy } from "./siteHealth";
import { officialHealthAdapter, officialHealthScopeNote } from "./officialHealthAdapters";
import { SiteHealthAdapterFields } from "./SiteHealthAdapterFields";

type Props = {
  policy: ProbePolicy;
  onChange(value: ProbePolicy): void;
  groups: string;
  mapping: string;
  onText(groups: string, mapping: string): void;
  t(zh: string, en: string): string;
};

/** Policy fields are ordinary metadata; no monitoring token is accepted here. */
export function SiteHealthPolicyFields({
  policy: p,
  onChange,
  groups,
  mapping,
  onText,
  t,
}: Props) {
  const official = officialHealthAdapter(p.adapter);
  return (
    <div className="nt-health-policy-fields">
      <Field label={t("控制策略", "Policy")}>
        <Select
          value={p.mode}
          onChange={(e) =>
            onChange({ ...p, mode: e.target.value as ProbePolicy["mode"] })
          }
        >
          <option value="advisory">{t("仅提示", "Advisory")}</option>
          <option value="restrict">
            {t("低于阈值时禁用模型", "Exclude models below threshold")}
          </option>
        </Select>
      </Field>
      <Field label={t("最低成功率 %", "Minimum success rate %")}>
        <Input
          type="number"
          min={0}
          max={100}
          disabled={!!official}
          value={p.minimum_success_rate}
          onChange={(e) =>
            onChange({ ...p, minimum_success_rate: e.target.valueAsNumber })
          }
        />
      </Field>
      <SiteHealthAdapterFields policy={p} onChange={onChange} t={t} />
      <Field label={t("刷新间隔（秒）", "Refresh interval (seconds)")}>
        <Input
          type="number"
          min={30}
          max={86400}
          value={p.refresh_interval_secs}
          onChange={(e) =>
            onChange({ ...p, refresh_interval_secs: e.target.valueAsNumber })
          }
        />
      </Field>
      <Field label={t("信息有效期（秒）", "Maximum age (seconds)")}>
        <Input
          type="number"
          min={30}
          max={604800}
          value={p.max_age_secs}
          onChange={(e) =>
            onChange({ ...p, max_age_secs: e.target.valueAsNumber })
          }
        />
      </Field>
      <Field label={t("未知或过期信息", "Unknown or stale information")}>
        <Select
          value={p.unknown_policy}
          onChange={(e) =>
            onChange({
              ...p,
              unknown_policy: e.target.value as ProbePolicy["unknown_policy"],
            })
          }
        >
          <option value="allow">{t("放行", "Allow")}</option>
          <option value="block">{t("阻止", "Block")}</option>
        </Select>
      </Field>
      <div hidden={!!official}>
        <Field label={t("监控分组（逗号分隔）", "Monitor groups (comma separated)")}
          description={t(
            "这是远端监控分类，不是本地凭证组；sub2api 填数字 group ID。",
            "Remote monitor categories, not local account groups; sub2api uses numeric group IDs.",
          )}>
          <Input
            value={groups}
            onChange={(e) => onText(e.target.value, mapping)}
          />
        </Field>
      </div>
      <div hidden={!!official}>
        <Checkbox
          checked={p.allow_private_network}
          onChange={(e) =>
            onChange({ ...p, allow_private_network: e.target.checked })
          }
          label={t(
            "允许访问所选站点的内网地址",
            "Allow private network addresses for this site",
          )} />
      </div>
      {official && <p className="nt-site-health__wide">{p.adapter === "openai-status"
        ? t("固定官方地址，无需认证。Codex API 状态统一应用于本方案全部模型；状态不是成功率百分比。", "Fixed public source. Codex API status applies to all models in this plan; it is not a success percentage.")
        : t("固定官方地址，无需认证。仅有明确模型身份的状态参与模型准入，其余组件仅供参考；缺少模型数据按未知策略处理。不将状态换算成成功率。", "Fixed public source. Only explicit model identities participate in admission; other components are informational. Missing model data follows the unknown policy, without fabricated rates.")}</p>}
      {officialHealthScopeNote(p.adapter, t) && <p className="nt-site-health__wide" role="note">{officialHealthScopeNote(p.adapter, t)}</p>}
      <p className="nt-site-health__wide">
        {t(
          "禁用发生在服务商候选选择之前，不参与可用性排序。恢复只解除监控限制，绝不覆盖人工禁用。",
          "Restrictions exclude provider candidates before ranking. Recovery never overrides manual disablement.",
        )}
      </p>
    </div>
  );
}
