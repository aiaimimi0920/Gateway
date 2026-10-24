import * as Select from "@radix-ui/react-select";
import { ArrowDownWideNarrow, Check, ChevronDown, Search, X } from "lucide-react";
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

export function AccessKeyToolbar({ query, sort, onQuery, onSort, t }: {
  query: string; sort: AccessKeySort; onQuery: (value: string) => void;
  onSort: (value: AccessKeySort) => void; t: (zh: string, en: string) => string;
}) {
  const options: [AccessKeySort, string][] = [
    ["newest", t("最新创建", "Newest first")], ["oldest", t("最早创建", "Oldest first")],
    ["name", t("名称升序", "Name ascending")], ["name-desc", t("名称降序", "Name descending")],
  ];
  return <div className="nt-provider-pool-toolbar" role="search" aria-label={t("搜索密钥", "Search keys")}>
    <div className="nt-provider-pool-toolbar__search">
      <Search size={15} aria-hidden="true" />
      <input type="search" className="nt-input" maxLength={256} value={query}
        aria-label={t("搜索密钥", "Search keys")} placeholder={t("搜索名称、归属或 ID", "Search name, owner or ID")}
        onChange={(event) => onQuery(event.target.value)} />
      {query && <button className="nt-icon-action" type="button" aria-label={t("清除密钥搜索", "Clear key search")}
        onClick={() => onQuery("")}><X size={14} aria-hidden="true" /></button>}
    </div>
    <Select.Root value={sort} onValueChange={(value: AccessKeySort) => onSort(value)}>
      <Select.Trigger className="nt-btn nt-btn--outline nt-provider-pool-toolbar__sort" aria-label={t("密钥排序", "Sort keys")}>
        <ArrowDownWideNarrow size={15} aria-hidden="true" /><Select.Value />
        <Select.Icon><ChevronDown size={13} aria-hidden="true" /></Select.Icon>
      </Select.Trigger>
      <Select.Portal><Select.Content className="nt-account-routing-pool-select__popup" position="popper" align="start" sideOffset={4} collisionPadding={8}>
        <Select.Viewport className="nt-account-routing-pool-select__viewport">
          {options.map(([value, label]) => <Select.Item className="nt-account-routing-pool-select__item" key={value} value={value} textValue={label}>
            <Select.ItemText>{label}</Select.ItemText><Select.ItemIndicator><Check size={13} aria-hidden="true" /></Select.ItemIndicator>
          </Select.Item>)}
        </Select.Viewport>
      </Select.Content></Select.Portal>
    </Select.Root>
  </div>;
}
