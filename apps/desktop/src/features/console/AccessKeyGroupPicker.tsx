import { EmptyState, MultiSelect, Notice } from "@neuro/ui";
import type { ConsoleAccessKeyGroup } from "../../api/contracts";
import type { TranslateFn } from "./accessKeysTypes";

export function AccessKeyGroupPicker({
  groups,
  selected,
  onChange,
  disabled,
  t,
}: {
  groups: ConsoleAccessKeyGroup[] | undefined;
  selected: string[];
  onChange(ids: string[]): void;
  disabled: boolean;
  t: TranslateFn;
}) {
  const missing = selected.filter(
    (id) => !groups?.some((group) => group.id === id),
  );
  const rows = [
    ...(groups ?? []),
    ...missing.map((id) => ({ id, name: id, enabled: false, memberCount: 0 })),
  ];
  const summary = selected.map((id) => rows.find((group) => group.id === id)?.name || id).join("、");
  return (
    <MultiSelect className="nt-key-form__group-field" label={t("权益组", "Entitlement groups")}
      values={selected} onValuesChange={onChange} disabled={disabled}
      formatSummary={() => summary || t("选择权益组", "Select groups")}
      search={rows.length > 6 ? { label: t("搜索权益组", "Search entitlement groups"), emptyMessage: t("没有匹配的权益组", "No matching groups") } : undefined}
      empty={groups === undefined
        ? <Notice tone="danger">{t("当前网关不支持权益组密钥，请升级网关。", "Upgrade this gateway to use group-bound keys.")}</Notice>
        : <EmptyState compact role="status" title={t("暂无权益组", "No entitlement groups")} />}
      options={rows.map(group => ({
        value: group.id,
        label: group.name || group.id,
        description: group.name && group.name !== group.id ? group.id : undefined,
        searchText: `${group.name} ${group.id}`,
        meta: missing.includes(group.id) ? t("已移除", "Removed") : !group.enabled ? t("已停用", "Disabled") : t(`${group.memberCount} 个账号`, `${group.memberCount} accounts`),
        // Already-selected unavailable groups must remain removable.
        disabled: !selected.includes(group.id) && (!group.enabled || selected.length >= 128),
      }))} />
  );
}
