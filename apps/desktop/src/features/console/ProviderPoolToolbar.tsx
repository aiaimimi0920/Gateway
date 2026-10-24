import * as Select from "@radix-ui/react-select";
import { ArrowDownWideNarrow, Check, ChevronDown, Search, X } from "lucide-react";
import type { TranslateFn } from "./accountsLedgerTypes";
import type { ProviderPoolSort, ProviderPoolView } from "./providerPoolView";
import "./ProviderPoolToolbar.css";

export function ProviderPoolToolbar({ value, onChange, t, scope = "sites" }: {
  value: ProviderPoolView; onChange(value: ProviderPoolView): void; t: TranslateFn;
  scope?: "sites" | "groups" | "models";
}) {
  const [searchLabel, clearLabel, sortLabel, countLabel] = scope === "groups"
    ? [t("搜索权益池", "Search entitlement pools"), t("清除权益池搜索", "Clear entitlement pool search"),
      t("权益池排序", "Sort entitlement pools"), t("成员账号最多", "Most member accounts")]
    : scope === "models"
      ? [t("搜索模型", "Search models"), t("清除模型搜索", "Clear model search"),
        t("模型排序", "Sort models"), t("已启用账号最多", "Most enabled accounts")]
      : [t("搜索站点", "Search sites"), t("清除站点搜索", "Clear site search"),
        t("站点排序", "Sort sites"), t("可用账号最多", "Most available accounts")];
  const options: [ProviderPoolSort, string][] = [
    ["default", t("默认顺序", "Default order")],
    ["name", t("名称升序", "Name ascending")],
    ["name-desc", t("名称降序", "Name descending")],
    ["available", countLabel],
    ["success", t("成功率最高", "Highest success rate")],
    ["requests", t("调用次数最多", "Most requests")],
  ];
  return <div className="nt-provider-pool-toolbar" role="search" aria-label={searchLabel}>
    <div className="nt-provider-pool-toolbar__search">
      <Search size={15} aria-hidden="true" />
      <input type="search" className="nt-input" placeholder={searchLabel}
        aria-label={searchLabel} value={value.query}
        onChange={(event) => onChange({ ...value, query: event.target.value })} />
      {value.query && <button className="nt-icon-action" type="button" aria-label={clearLabel}
        onClick={() => onChange({ ...value, query: "" })}><X size={14} aria-hidden="true" /></button>}
    </div>
    <Select.Root value={value.sort} onValueChange={(sort: ProviderPoolSort) => onChange({ ...value, sort })}>
      <Select.Trigger className="nt-btn nt-btn--outline nt-provider-pool-toolbar__sort" aria-label={sortLabel}>
        <ArrowDownWideNarrow size={15} aria-hidden="true" /><Select.Value />
        <Select.Icon><ChevronDown size={13} aria-hidden="true" /></Select.Icon>
      </Select.Trigger>
      <Select.Portal><Select.Content className="nt-account-routing-pool-select__popup" position="popper" align="start" sideOffset={4} collisionPadding={8}>
        <Select.Viewport className="nt-account-routing-pool-select__viewport">
          {options.map(([sort, label]) => <Select.Item className="nt-account-routing-pool-select__item" key={sort} value={sort} textValue={label}>
            <Select.ItemText>{label}</Select.ItemText><Select.ItemIndicator><Check size={13} aria-hidden="true" /></Select.ItemIndicator>
          </Select.Item>)}
        </Select.Viewport>
      </Select.Content></Select.Portal>
    </Select.Root>
  </div>;
}
