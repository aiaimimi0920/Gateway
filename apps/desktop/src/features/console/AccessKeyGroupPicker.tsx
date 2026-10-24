import { useId, useState } from "react";
import { Check, ChevronDown } from "lucide-react";
import type { ConsoleAccessKeyGroup } from "../../api/contracts";
import type { TranslateFn } from "./accessKeysTypes";
import { useKeyGroupDropdown } from "./useKeyGroupDropdown";
import "./accessKeyGroupDropdown.css";

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
  const [query, setQuery] = useState("");
  const labelId = useId();
  const panelId = useId();
  const { open, setOpen, trigger, panel } = useKeyGroupDropdown(disabled);
  const missing = selected.filter(
    (id) => !groups?.some((group) => group.id === id),
  );
  const rows = [
    ...(groups ?? []),
    ...missing.map((id) => ({ id, name: id, enabled: false, memberCount: 0 })),
  ];
  const visible = rows.filter((group) =>
    `${group.name} ${group.id}`
      .toLocaleLowerCase()
      .includes(query.trim().toLocaleLowerCase()),
  );
  const summary = selected.map((id) => rows.find((group) => group.id === id)?.name || id).join("、");
  return (
    <div className="nt-key-form__group-field">
      <span id={labelId}>{t("权益组", "Entitlement groups")}</span>
      <button
        ref={trigger}
        type="button"
        className="nt-input nt-key-group-trigger"
        aria-labelledby={labelId}
        aria-haspopup="menu"
        aria-expanded={open && !disabled}
        aria-controls={panelId}
        disabled={disabled}
        title={summary || t("选择权益组", "Select groups")}
        onClick={() => { setQuery(""); setOpen(!open); }}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            setQuery("");
            if (open) panel.current?.querySelector<HTMLElement>("input, button:not(:disabled)")?.focus();
            setOpen(true);
          }
        }}
      >
        <span>{summary || t("选择权益组", "Select groups")}</span>
        <ChevronDown size={14} aria-hidden="true" />
      </button>
      <div
        ref={panel}
        id={panelId}
        popover="manual"
        hidden={!open || disabled}
        className="nt-key-group-dropdown"
        onKeyDown={(event) => {
          if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
          if (event.target instanceof HTMLInputElement && !event.key.startsWith("Arrow")) return;
          const items = Array.from(panel.current?.querySelectorAll<HTMLButtonElement>('[role="menuitemcheckbox"]:not(:disabled)') ?? []);
          if (!items.length) return;
          event.preventDefault();
          const index = items.indexOf(document.activeElement as HTMLButtonElement);
          const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : index < 0 ? (event.key === "ArrowUp" ? items.length - 1 : 0) : (index + (event.key === "ArrowUp" ? -1 : 1) + items.length) % items.length;
          items[next].focus();
        }}
      >
        {rows.length > 6 ? (
          <input
            className="nt-input"
            aria-label={t("搜索权益组", "Search entitlement groups")}
            placeholder={t("搜索权益组", "Search entitlement groups")}
            value={query}
            maxLength={256}
            onChange={(event) => setQuery(event.target.value)}
          />
        ) : null}
        {groups === undefined ? (
          <p role="alert">
            {t(
              "当前网关不支持权益组密钥，请升级网关。",
              "Upgrade this gateway to use group-bound keys.",
            )}
          </p>
        ) : rows.length === 0 ? (
          <p role="status">{t("暂无权益组", "No entitlement groups")}</p>
        ) : (
          <div role="menu" aria-labelledby={labelId} className="nt-key-group-dropdown__list">
            {visible.map((group) => (
              <button
                key={group.id}
                type="button"
                role="menuitemcheckbox"
                aria-checked={selected.includes(group.id)}
                data-unavailable={!group.enabled}
                disabled={
                  disabled || (!selected.includes(group.id) &&
                    (!group.enabled || selected.length >= 128))
                }
                onClick={() =>
                  onChange(
                    selected.includes(group.id)
                      ? selected.filter((id) => id !== group.id)
                      : [...selected, group.id],
                  )
                }
              >
                <Check className="nt-key-group-dropdown__check" size={15} aria-hidden="true" />
                <span>
                  <strong>{group.name || group.id}</strong>
                  {group.name && group.name !== group.id ? <small>{group.id}</small> : null}
                </span>
                <small>
                  {missing.includes(group.id)
                    ? t("已移除", "Removed")
                    : !group.enabled
                      ? t("已停用", "Disabled")
                      : t(
                        `${group.memberCount} 个账号`,
                        `${group.memberCount} accounts`,
                      )}
                </small>
              </button>
            ))}
            {visible.length === 0 ? (
              <p role="status">{t("没有匹配的权益组", "No matching groups")}</p>
            ) : null}
          </div>
        )}
      </div>
    </div>
  );
}
