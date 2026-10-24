import type { RouteAccountGroup } from "./routeAccountCatalog";
import type { SiteHealthConfig } from "./siteHealth";

type Props = {
  config: SiteHealthConfig;
  selectedId: string | null;
  groups: RouteAccountGroup[];
  onChange(config: SiteHealthConfig): void;
  t(zh: string, en: string): string;
};

export function SiteHealthProfileFields({
  config,
  selectedId,
  groups,
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
  return (
    <div className="nt-health-profile-fields">
      {selected ? (
        <>
          <label className="nt-field">
            <span>{t("方案名称", "Plan name")}</span>
            <input
              className="nt-input"
              maxLength={128}
              value={selected.name}
              onChange={(e) => update({ name: e.target.value })}
            />
          </label>
          <small>
            {t("稳定 ID", "Stable ID")}: {selected.id}
          </small>
          <label>
            <input
              type="checkbox"
              checked={!selected.account_group_ids.length}
              disabled={config.profiles.length !== 1 || !groups.length}
              onChange={(e) =>
                update({
                  account_group_ids: e.target.checked ? [] : [groups[0].id],
                })
              }
            />
            {t("全部分组共享一次检测", "Share one probe across all groups")}
          </label>
          {selected.account_group_ids.length > 0 && (
            <fieldset className="nt-site-health__groups">
              <legend>
                {t(
                  "绑定本地凭证组（可多选共享）",
                  "Local account groups (select multiple to share)",
                )}
              </legend>
              {groups.map((g) => (
                <label key={g.id}>
                  <input
                    type="checkbox"
                    checked={selected.account_group_ids.includes(g.id)}
                    disabled={
                      occupied.has(g.id) ||
                      (selected.account_group_ids.length === 1 &&
                        selected.account_group_ids.includes(g.id))
                    }
                    onChange={(e) =>
                      update({
                        account_group_ids: e.target.checked
                          ? [...selected.account_group_ids, g.id]
                          : selected.account_group_ids.filter(
                              (id) => id !== g.id,
                            ),
                      })
                    }
                  />
                  {g.name || g.id}{" "}
                  <small>
                    ({g.id})
                    {occupied.has(g.id)
                      ? t(" · 已绑定其他方案", " · assigned elsewhere")
                      : ""}
                  </small>
                </label>
              ))}
              {selected.account_group_ids
                .filter((id) => !groups.some((g) => g.id === id))
                .map((id) => (
                  <label key={id}>
                    <input
                      type="checkbox"
                      checked
                      onChange={() =>
                        update({
                          account_group_ids: selected.account_group_ids.filter(
                            (g) => g !== id,
                          ),
                        })
                      }
                    />
                    {t(
                      `已删除的分组：${id}（取消绑定后才能保存）`,
                      `Missing group: ${id} (unbind before saving)`,
                    )}
                  </label>
                ))}
            </fieldset>
          )}
        </>
      ) : (
        <p>
          {t(
            "当前全站只检测一次并共享结果。新增方案后，可按本地凭证组分别检测；旧密钥不会自动复制到新方案。",
            "Whole-site monitoring shares one result. Add plans to probe local groups independently; legacy tokens are never copied to new plans.",
          )}
        </p>
      )}
      <small>
        {t(
          "未绑定的组不继承其他方案结果；同一凭证属于多个组时，必须同时满足对应限制。全部分组共享时需先改为指定分组，才能新增方案。",
          "Unbound groups never inherit other plans. Credentials in multiple groups must satisfy all limits. Uncheck all-groups sharing before adding another plan.",
        )}
      </small>
    </div>
  );
}
