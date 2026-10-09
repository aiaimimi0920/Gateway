import { useState } from "react";
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
  const [query, setQuery] = useState("");
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
  return (
    <fieldset className="nt-key-group-picker" disabled={disabled}>
      <legend>
        {t("权益组", "Entitlement groups")} <span>{selected.length}</span>
      </legend>
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
        <div className="nt-key-group-picker__list">
          {visible.map((group) => (
            <label key={group.id} data-selected={selected.includes(group.id)}>
              <input
                type="checkbox"
                checked={selected.includes(group.id)}
                disabled={
                  !selected.includes(group.id) &&
                  (!group.enabled || selected.length >= 128)
                }
                onChange={(event) =>
                  onChange(
                    event.target.checked
                      ? [...selected, group.id]
                      : selected.filter((id) => id !== group.id),
                  )
                }
              />
              <span>
                <strong>{group.name || group.id}</strong>
                <small>{group.id}</small>
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
            </label>
          ))}
          {visible.length === 0 ? (
            <p role="status">{t("没有匹配的权益组", "No matching groups")}</p>
          ) : null}
        </div>
      )}
    </fieldset>
  );
}
