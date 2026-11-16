import { Button, ConfirmDialog, IconButton, Switch, Tab, TabList, TabPanel, TabsRoot } from "@neuro/ui";
import { Plus, Trash2 } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
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
  const frame = useRef<HTMLDivElement | null>(null);
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
    <TabsRoot ref={frame}
      className="nt-health-plan-layout"
      value={selectedId === null ? "legacy" : `plan:${selectedId}`}
      onValueChange={(id) => {
        setConfirm(false);
        onSelect(id === "legacy" ? null : id.slice(5));
      }}
    >
      <div className="nt-health-plan-bar">
        <div className="nt-health-plan-scroll">
          <TabList
            label={t("检测方案", "Detection plans")}
          >
            {!config.profiles.length && (
              <Tab value="legacy">
                {t("默认方案", "Default plan")}
              </Tab>
            )}
            {config.profiles.map((p) => (
              <Tab
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
                <span>{p.name}</span>
                <small className="n-muted">
                  {p.policy.enabled
                    ? t("已启用", "Enabled")
                    : t("已停用", "Paused")}
                </small>
              </Tab>
            ))}
          </TabList>
          <IconButton compact
            label={t("新增检测方案", "Add plan")}
            disabled={
              locked ||
              config.profiles.length >= 16 ||
              shared ||
              (config.profiles.length > 0 && !free.length)
            }
            onClick={add}
          >
            <Plus size={16} aria-hidden="true" />
          </IconButton>
        </div>
        <Switch
          label={t("启用此方案", "Enable this plan")}
          checked={enabled}
          disabled={locked}
          onChange={(event) => onEnable(event.target.checked)}
        />
      </div>
      <TabPanel
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
      </TabPanel>
      <footer className="nt-health-plan-footer">
        <Button wrap variant="danger"
          disabled={locked || !selected}
          aria-label={t("删除当前方案", "Remove current plan")}
          onClick={() => setConfirm(true)}
        >
          <Trash2 size={15} aria-hidden="true" />
          {t("删除方案", "Remove plan")}
        </Button>
        <ConfirmDialog open={confirm} onOpenChange={setConfirm}
          title={t("删除检测方案？", "Remove detection plan?")}
          description={t(
            `删除「${selected?.name ?? ""}」及其监控密钥；删除最后一个方案将关闭本站监控。保存后生效，不会删除服务商或凭证。`,
            `Remove “${selected?.name ?? ""}” and its monitoring token. Removing the last plan disables monitoring. Applies on save; providers and credentials are not removed.`,
          )}
          cancelLabel={t("取消", "Cancel")} confirmLabel={t("确认删除方案", "Confirm removal")}
          confirmDisabled={locked || !selected}
          resolveReturnFocus={(target) => target?.isConnected && !target.matches(":disabled") ? target
            : frame.current?.querySelector<HTMLButtonElement>('[role="tab"][data-state="active"]:not(:disabled)')
            ?? frame.current?.closest('[role="dialog"]')?.querySelector<HTMLButtonElement>('button:not(:disabled)') ?? null}
          onConfirm={() => {
            if (locked || !selected) return;
            const profiles = config.profiles.filter(
              (p) => p.id !== selectedId,
            );
            onChange({
              ...config,
              profiles,
              enabled: profiles.length ? config.enabled : false,
            });
            onSelect(profiles[0]?.id ?? null);
            setConfirm(false);
          }}
        />
        <div className="nt-health-plan-actions">{actions}</div>
      </footer>
    </TabsRoot>
  );
}
