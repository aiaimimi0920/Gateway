import { Checkbox, CheckboxGroup, Field, Input, MultiSelect } from "@neuro/ui";
import { useEffect, useRef, useState } from "react";
import type { RouteAccountGroup } from "./routeAccountCatalog";
import type { SiteHealthConfig } from "./siteHealth";

type Props = {
  config: SiteHealthConfig;
  selectedId: string | null;
  groups: RouteAccountGroup[];
  disabled?: boolean;
  onChange(config: SiteHealthConfig): void;
  t(zh: string, en: string): string;
};

export function SiteHealthProfileFields({
  config,
  selectedId,
  groups,
  disabled,
  onChange,
  t,
}: Props) {
  const selected = config.profiles.find((p) => p.id === selectedId);
  const occupied = new Set(
    config.profiles
      .filter((p) => p.id !== selectedId)
      .flatMap((p) => p.account_group_ids),
  );
  const update = (patch: Partial<NonNullable<typeof selected>>) =>
    onChange({
      ...config,
      profiles: config.profiles.map((p) =>
        p.id === selectedId ? { ...p, ...patch } : p,
      ),
    });
  const name = useRef<HTMLInputElement | null>(null);
  const sharing = useRef<HTMLInputElement | null>(null);
  // Retain removed candidates for this provider session: unchecking must not remove its DOM node.
  const [knownIds, setKnownIds] = useState(() => selected?.account_group_ids ?? []);
  useEffect(() => {
    setKnownIds((previous) => {
      const next = [...new Set([...previous, ...selected?.account_group_ids ?? []])];
      return next.length === previous.length ? previous : next;
    });
  }, [selected?.account_group_ids]);
  const missingIds = [...new Set([...knownIds, ...selected?.account_group_ids ?? []])].filter((id) => !groups.some((g) => g.id === id));
  const values = selected?.account_group_ids ?? [];
  const options = [
    ...groups.map((g) => ({
      value: g.id,
      label: `${g.name || g.id} (${g.id})${occupied.has(g.id) ? t(" · 已绑定其他方案", " · assigned elsewhere") : ""}`,
      disabled: occupied.has(g.id) || (values.length === 1 && values.includes(g.id)),
    })),
    ...missingIds.map((id) => ({
      value: id,
      label: t(`已删除的分组：${id}（只能取消绑定）`, `Missing group: ${id} (can only unbind)`),
      disabled: !values.includes(id),
    })),
  ];
  const groupLabel = t("绑定本地凭证组（可多选共享）", "Local account groups (select multiple to share)");
  const changeGroups = (next: string[]) => {
    update({ account_group_ids: next });
    // The last missing binding becomes all-groups sharing; return to a usable field.
    if (!next.length) (sharing.current?.disabled ? name.current : sharing.current)?.focus();
  };
  return (
    <div className="nt-health-profile-fields">
      {selected ? (
        <>
          <Field label={t("方案名称", "Plan name")}>
            <Input ref={name}
              maxLength={128}
              value={selected.name}
              onChange={(e) => update({ name: e.target.value })}
            />
          </Field>
          <small className="n-muted">
            {t("稳定 ID", "Stable ID")}: <code>{selected.id}</code>
          </small>
          <Checkbox ref={sharing}
            checked={!selected.account_group_ids.length}
            disabled={config.profiles.length !== 1 || !groups.length}
            onChange={(e) =>
              update({
                account_group_ids: e.target.checked ? [] : [groups[0].id],
              })
            }
            label={t("全部分组共享一次检测", "Share one probe across all groups")} />
          {options.length > 6 ? <MultiSelect label={groupLabel} options={options} values={values}
            disabled={disabled || !values.length} onValuesChange={changeGroups}
            formatSummary={(count) => t(`已选 ${count}/${options.length} 项`, `${count}/${options.length} selected`)}
            search={{ label: t("搜索本地凭证组", "Search local account groups"), emptyMessage: t("未找到凭证组", "No account groups found") }} />
            : <CheckboxGroup label={groupLabel} options={options} values={values} disabled={disabled || !values.length}
              orientation="vertical" onValuesChange={changeGroups} />}
        </>
      ) : (
        <p className="n-muted">
          {t(
            "当前全站只检测一次并共享结果。新增方案后，可按本地凭证组分别检测；旧密钥不会自动复制到新方案。",
            "Whole-site monitoring shares one result. Add plans to probe local groups independently; legacy tokens are never copied to new plans.",
          )}
        </p>
      )}
      <small className="n-muted">
        {t(
          "未绑定的组不继承其他方案结果；同一凭证属于多个组时，必须同时满足对应限制。全部分组共享时需先改为指定分组，才能新增方案。",
          "Unbound groups never inherit other plans. Credentials in multiple groups must satisfy all limits. Uncheck all-groups sharing before adding another plan.",
        )}
      </small>
    </div>
  );
}
