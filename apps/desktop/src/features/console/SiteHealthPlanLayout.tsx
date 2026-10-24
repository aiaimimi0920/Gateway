import * as AlertDialog from "@radix-ui/react-alert-dialog";
import * as Tabs from "@radix-ui/react-tabs";
import { Plus, Trash2 } from "lucide-react";
import { useState, type ReactNode } from "react";
import type { RouteAccountGroup } from "./routeAccountCatalog";
import { probePolicySchema, type SiteHealthConfig } from "./siteHealth";
import "./SiteHealthPlanLayout.css";

type Props = {
  config: SiteHealthConfig;
  selectedId: string | null;
  groups: RouteAccountGroup[];
  enabled: boolean;
  locked: boolean;
  onEnable(value: boolean): void;
  onChange(config: SiteHealthConfig): void;
  onSelect(id: string | null): void;
  children: ReactNode;
  report: ReactNode;
  actions: ReactNode;
  t(zh: string, en: string): string;
};

/** Presentation only: preserve stable profile IDs, secret ownership and legacy policy storage. */
export function SiteHealthPlanLayout({
  config,
  selectedId,
  groups,
  enabled,
  locked,
  onEnable,
  onChange,
  onSelect,
  children,
  report,
  actions,
  t,
}: Props) {
  const [confirm, setConfirm] = useState(false);
  const selected = config.profiles.find((p) => p.id === selectedId);
  const assigned = new Set(config.profiles.flatMap((p) => p.account_group_ids));
  const free = groups.filter((g) => !assigned.has(g.id));
  const shared = config.profiles.some((p) => !p.account_group_ids.length);
  const add = () => {
    const id = `health-${crypto.randomUUID()}`;
    const policy = probePolicySchema.parse(selected?.policy ?? config);
    onChange({
      ...config,
      profiles: [
        ...config.profiles,
        {
          id,
          name: t(
            `检测方案 ${config.profiles.length + 1}`,
            `Plan ${config.profiles.length + 1}`,
          ),
          account_group_ids: free[0] ? [free[0].id] : [],
          policy: {
            ...policy,
            enabled: true,
            auth: { kind: "none", token: null },
          },
        },
      ],
    });
    onSelect(id);
  };
  return (
    <Tabs.Root
      className="nt-health-plan-layout"
      value={selectedId === null ? "legacy" : `plan:${selectedId}`}
      onValueChange={(id) => {
        setConfirm(false);
        onSelect(id === "legacy" ? null : id.slice(5));
      }}
    >
      <div className="nt-health-plan-bar">
        <div className="nt-health-plan-scroll">
          <Tabs.List
            aria-label={t("检测方案", "Detection plans")}
            className="nt-health-plan-tabs"
          >
            {!config.profiles.length && (
              <Tabs.Trigger value="legacy" className="nt-health-plan-tab">
                {t("默认方案", "Default plan")}
              </Tabs.Trigger>
            )}
            {config.profiles.map((p) => (
              <Tabs.Trigger
                key={p.id}
                value={`plan:${p.id}`}
                data-profile-id={p.id}
                className="nt-health-plan-tab"
                title={p.name}
                aria-label={p.name}
                aria-description={
                  p.policy.enabled
                    ? t("已启用", "Enabled")
                    : t("已停用", "Paused")
                }
              >
                {p.name}
                <small className="nt-health-plan-state">
                  {p.policy.enabled
                    ? t("已启用", "Enabled")
                    : t("已停用", "Paused")}
                </small>
              </Tabs.Trigger>
            ))}
          </Tabs.List>
          <button
            type="button"
            className="nt-icon-action nt-health-add-plan"
            aria-label={t("新增检测方案", "Add plan")}
            disabled={
              locked ||
              config.profiles.length >= 16 ||
              shared ||
              (config.profiles.length > 0 && !free.length)
            }
            onClick={add}
          >
            <Plus size={16} />
          </button>
        </div>
        <label className="nt-health-enable">
          <span>{t("启用此方案", "Enable this plan")}</span>
          <input
            type="checkbox"
            role="switch"
            aria-label={t("启用此方案", "Enable this plan")}
            checked={enabled}
            disabled={locked}
            onChange={(event) => onEnable(event.target.checked)}
          />
        </label>
      </div>
      <Tabs.Content
        value={selectedId === null ? "legacy" : `plan:${selectedId}`}
        className="nt-health-plan-body"
        aria-label={t("当前检测方案", "Current detection plan")}
      >
        <fieldset
          disabled={locked || !enabled}
          className="nt-health-plan-fields"
        >
          {children}
        </fieldset>
        <section
          className="nt-health-report"
          aria-label={t("模型可用性", "Model availability")}
        >
          {report}
        </section>
      </Tabs.Content>
      <footer className="nt-health-plan-footer">
        <AlertDialog.Root open={confirm} onOpenChange={setConfirm}>
          <AlertDialog.Trigger asChild>
            <button
              type="button"
              className="nt-btn nt-btn--outline nt-health-remove"
              disabled={locked || !selected}
              aria-label={t("删除当前方案", "Remove current plan")}
            >
              <Trash2 size={15} />
              {t("删除方案", "Remove plan")}
            </button>
          </AlertDialog.Trigger>
          <AlertDialog.Portal>
            <AlertDialog.Overlay className="dialog-overlay" />
            <AlertDialog.Content className="dialog-content nt-health-confirm">
              <AlertDialog.Title>
                {t("删除检测方案？", "Remove detection plan?")}
              </AlertDialog.Title>
              <AlertDialog.Description>
                {t(
                  `删除「${selected?.name ?? ""}」及其监控密钥；删除最后一个方案将关闭本站监控。保存后生效，不会删除服务商或凭证。`,
                  `Remove “${selected?.name ?? ""}” and its monitoring token. Removing the last plan disables monitoring. Applies on save; providers and credentials are not removed.`,
                )}
              </AlertDialog.Description>
              <div className="dialog-actions">
                <AlertDialog.Cancel className="nt-btn nt-btn--outline">
                  {t("取消", "Cancel")}
                </AlertDialog.Cancel>
                <AlertDialog.Action
                  className="nt-btn nt-btn--outline nt-health-remove"
                  disabled={locked}
                  onClick={() => {
                    const profiles = config.profiles.filter(
                      (p) => p.id !== selectedId,
                    );
                    onChange({
                      ...config,
                      profiles,
                      enabled: profiles.length ? config.enabled : false,
                    });
                    onSelect(profiles[0]?.id ?? null);
                  }}
                >
                  {t("确认删除方案", "Confirm removal")}
                </AlertDialog.Action>
              </div>
            </AlertDialog.Content>
          </AlertDialog.Portal>
        </AlertDialog.Root>
        <div className="nt-health-plan-actions">{actions}</div>
      </footer>
    </Tabs.Root>
  );
}
