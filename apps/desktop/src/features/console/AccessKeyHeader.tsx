import { IconButton, Input, PopupSelect } from "@neuro/ui";
import { ArrowDownWideNarrow, Search, X } from "lucide-react";
import { createContext, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { ConsoleAccessKey } from "../../api/contracts";
import "./ProviderPoolToolbar.css";
import "./AccessKeyHeader.css";

type Slot = "leading" | "actions";
type Slots = Record<Slot, HTMLDivElement | null>;
const HeaderContext = createContext<{
  targets: Slots;
  setLeading: (node: HTMLDivElement | null) => void;
  setActions: (node: HTMLDivElement | null) => void;
} | null>(null);

/** Portals keep key dialog/filter state in its owner while placing controls in the shell. */
export function AccessKeyHeaderProvider({ children }: { children: ReactNode }) {
  const [leading, setLeading] = useState<HTMLDivElement | null>(null);
  const [actions, setActions] = useState<HTMLDivElement | null>(null);
  return <HeaderContext.Provider value={{ targets: { leading, actions }, setLeading, setActions }}>
    {children}
  </HeaderContext.Provider>;
}

export function AccessKeyHeaderSlot({ slot }: { slot: Slot }) {
  const context = useContext(HeaderContext);
  return <div className={`nt-key-header-slot nt-key-header-slot--${slot}`}
    ref={slot === "leading" ? context?.setLeading : context?.setActions} />;
}

export function AccessKeyHeaderContent({ slot, children }: { slot: Slot; children: ReactNode }) {
  const context = useContext(HeaderContext);
  if (!context) return <>{children}</>;
  const target = context.targets[slot];
  return target ? createPortal(children, target) : null;
}

export type AccessKeySort = "newest" | "oldest" | "name" | "name-desc";

/** Sort public metadata only; never read or expose a key's secret. */
export function compareAccessKeys(left: ConsoleAccessKey, right: ConsoleAccessKey, sort: AccessKeySort) {
  if (sort === "oldest") return left.createdAt.localeCompare(right.createdAt);
  if (sort === "newest") return right.createdAt.localeCompare(left.createdAt);
  const order = left.displayName.localeCompare(right.displayName, undefined, { numeric: true, sensitivity: "base" });
  return sort === "name-desc" ? -order : order;
}

export function AccessKeyToolbar({ query, sort, status, onStatus, onQuery, onSort, t }: {
  status: string; onStatus: (value: string) => void;
  query: string; sort: AccessKeySort; onQuery: (value: string) => void;
  onSort: (value: AccessKeySort) => void; t: (zh: string, en: string) => string;
}) {
  const options: [AccessKeySort, string][] = [
    ["newest", t("最新创建", "Newest first")], ["oldest", t("最早创建", "Oldest first")],
    ["name", t("名称升序", "Name ascending")], ["name-desc", t("名称降序", "Name descending")],
  ];
  return <div className="nt-provider-pool-toolbar nt-key-toolbar" role="search" aria-label={t("搜索密钥", "Search keys")}>
    <div className="nt-provider-pool-toolbar__search">
      <Search size={15} aria-hidden="true" />
      <Input type="search" className="nt-key-search-input" compact maxLength={256} value={query}
        aria-label={t("搜索密钥", "Search keys")} placeholder={t("搜索名称、归属或 ID", "Search name, owner or ID")}
        onChange={(event) => onQuery(event.target.value)} />
      {query && <IconButton className="nt-key-search-clear" compact variant="quiet" label={t("清除密钥搜索", "Clear key search")}
        onClick={() => onQuery("")}><X size={14} aria-hidden="true" /></IconButton>}
    </div>
    <PopupSelect value={status} onValueChange={onStatus} aria-label={t("密钥状态", "Key status")}
      options={[["all", t("全部状态", "All statuses")], ["active", t("启用", "Active")], ["disabled", t("停用", "Disabled")], ["expired", t("已过期", "Expired")], ["revoked", t("已撤销", "Revoked")]].map(([value, label]) => ({ value, label }))} />
    <PopupSelect value={sort} onValueChange={onSort} options={options.map(([value, label]) => ({ value, label }))}
      leading={<ArrowDownWideNarrow size={15} aria-hidden="true" />}
      className="nt-provider-pool-toolbar__sort" aria-label={t("密钥排序", "Sort keys")} />
  </div>;
}
