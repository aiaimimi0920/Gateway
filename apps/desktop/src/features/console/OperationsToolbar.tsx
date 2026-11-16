import { IconButton, Input, PopupSelect } from "@neuro/ui";
import { ArrowDownWideNarrow, Search, X } from "lucide-react";
import type { TranslateFn } from "./operations-contracts";
import type { OperationsListView, OperationsSort } from "./operationsListView";
import "./ProviderPoolToolbar.css";

/** Local list controls do not submit the server's request filters or mutate telemetry. */
export function OperationsToolbar({ value, onChange, t }: {
  value: OperationsListView;
  onChange: (value: OperationsListView) => void;
  t: TranslateFn;
}) {
  const options: [OperationsSort, string][] = [
    ["default", t("最新优先", "Newest first (default)")],
    ["newest", t("时间降序", "Newest first")],
    ["oldest", t("时间升序", "Oldest first")],
    ["name", t("名称升序", "Name ascending")],
    ["name-desc", t("名称降序", "Name descending")],
  ];
  return <div className="nt-provider-pool-toolbar" role="search" aria-label={t("搜索运维记录", "Search operations records")}>
    <div className="nt-provider-pool-toolbar__search">
      <Search size={15} aria-hidden="true" />
      <Input compact type="search" maxLength={256} value={value.query}
        aria-label={t("搜索运维记录", "Search operations records")}
        placeholder={t("搜索当前记录", "Search loaded records")}
        title={t("仅筛选已加载的明细，不改变统计、运行状态或服务端筛选", "Filter loaded details only, without changing statistics, runtime health or server filters")}
        onChange={(event) => onChange({ ...value, query: event.target.value })} />
      <IconButton compact variant="quiet" disabled={!value.query} label={t("清除运维搜索", "Clear operations search")}
        onClick={() => onChange({ ...value, query: "" })}><X size={14} aria-hidden="true" /></IconButton>
    </div>
    <PopupSelect value={value.sort} onValueChange={(sort: OperationsSort) => onChange({ ...value, sort })}
      options={options.map(([value, label]) => ({ value, label }))} leading={<ArrowDownWideNarrow size={15} aria-hidden="true" />}
      className="nt-provider-pool-toolbar__sort" aria-label={t("运维排序", "Sort operations")}
      title={t("按各列表显示的时间或名称排序；无时间的条目保持原序", "Sort by each list's displayed time or name; untimed records retain source order")} />
  </div>;
}
