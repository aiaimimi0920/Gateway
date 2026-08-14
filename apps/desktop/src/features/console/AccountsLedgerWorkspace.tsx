import {
  BarChart3,
  CalendarClock,
  Copy,
  Ellipsis,
  Pencil,
  Play,
  Plus,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";

import { ActionTooltip } from "../../components/ActionTooltip";
import type {
  ConsoleCredentialPoolAutomationProvider,
  ConsoleCredentialRefillDemand,
} from "../../api/contracts";
import type {
  AccountEnabledFilter,
  AccountLedgerRow,
  AccountMembershipFilter,
} from "./accountManagementViewModel";
import {
  providerCatalogClassification,
  type ProviderCompatibility,
} from "./providerCatalog";

type TranslateFn = (zh: string, en: string) => string;

type Option = {
  value: string;
  label: string;
};

export type AccountsLedgerPilotAccount = {
  accountId: string;
  providerId: string;
  displayName: string;
  mode: "credential" | "provider-default";
  enabled: boolean;
  logicalLabels: string[];
  capacityLabel: string;
  statusLabel: string;
  dispatchEnabled: boolean;
  dispatchEditable: boolean;
  previewOnly: boolean;
  usageWindowBadges: string[];
  recentUseLabel: string;
  verificationStatus: "verified" | "failed" | "blocked" | "not-tested";
  verificationFamilies: string[];
  verificationCheckedAt: string | null;
  verificationEvidenceRef: string | null;
  verificationNote: string;
};

export type AccountsLedgerPilotCategory = {
  id: string;
  label: string;
  count: number;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  accounts: AccountsLedgerPilotAccount[];
};

export type AccountsLedgerPilotSection = {
  providerId: string;
  providerIds: string[];
  providerLabel: string;
  vendorLabel: string;
  providerPreset: string | null;
  hostLabel: string | null;
  defaultAccountId: string | null;
  manualAddFamily:
    | "gemini-canvas"
    | "gemini-canvas-chat"
    | "gemini-business"
    | "gemini-web"
    | null;
  hasExplicitAccounts: boolean;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  supportsIdentityCategories: boolean;
  identityCategories: AccountsLedgerPilotCategory[];
  directAccounts: AccountsLedgerPilotAccount[];
  providerCompatibility?: ProviderCompatibility | null;
};

export type AccountsLedgerWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  totalAccounts: number;
  totalGroups: number;
  ungroupedCount: number;
  providerCount: number;
  visibleCount: number;
  rows: AccountLedgerRow[];
  query: string;
  membership: AccountMembershipFilter;
  enabled: AccountEnabledFilter;
  selectedGroupId: string;
  selectedProviderKey: string;
  groupOptions: Option[];
  providerOptions: Option[];
  pilotSections: AccountsLedgerPilotSection[];
  automationByProvider: ReadonlyMap<string, ConsoleCredentialPoolAutomationProvider>;
  automationBusyProviderId: string | null;
  refillByProvider: ReadonlyMap<string, ConsoleCredentialRefillDemand>;
  refillBusyProviderId: string | null;
  onQueryChange: (value: string) => void;
  onMembershipChange: (value: AccountMembershipFilter) => void;
  onEnabledChange: (value: AccountEnabledFilter) => void;
  onGroupChange: (value: string) => void;
  onProviderChange: (value: string) => void;
  onOpenAddProvider: () => void;
  onOpenAddAccount: () => void;
  onOpenGroups: () => void;
  onBackToEditor: () => void;
  onAddIdentityCategory: (providerId: string) => void;
  onOpenGeminiManualAdd: (
    targetFamily:
      | "gemini-canvas"
      | "gemini-canvas-chat"
      | "gemini-business"
      | "gemini-web",
    providerId: string,
  ) => void;
  onEdit: (providerId: string, accountId: string) => void;
  onRemove: (providerId: string, accountId: string, displayName: string) => void;
  onAddExplicit: (providerId: string) => void;
  onToggleDispatch: (providerId: string, accountId: string, nextEnabled: boolean) => void;
  onUpdatePoolTargetSize: (providerId: string, categoryId: string, nextTargetSize: number) => void;
  onToggleAutoRefill: (providerId: string, categoryId: string, nextEnabled: boolean) => void;
  onToggleAutoPrune: (providerId: string, categoryId: string, nextEnabled: boolean) => void;
  onUpdateProviderPoolTargetSize: (providerId: string, nextTargetSize: number) => void;
  onToggleProviderAutoRefill: (providerId: string, nextEnabled: boolean) => void;
  onToggleProviderAutoPrune: (providerId: string, nextEnabled: boolean) => void;
  onRunProviderAutomation: (providerId: string) => void;
  onRequestProviderRefill: (providerId: string) => void;
  onOpenProbe: (providerId: string, account: AccountsLedgerPilotAccount) => void;
  onOpenStats: (providerId: string, account: AccountsLedgerPilotAccount) => void;
  onOpenSchedule: (providerId: string, account: AccountsLedgerPilotAccount) => void;
  onDuplicate: (providerId: string, account: AccountsLedgerPilotAccount) => void;
};

type PilotMenuActionId = "probe" | "stats" | "schedule" | "duplicate";

type PilotAggregateSummary = {
  totalCount: number;
  capacityAvailable: number;
  capacityTotal: number;
  normalCount: number;
  dispatchCount: number;
  usageRequestCount: number;
  usageSecondaryCount: number;
  usageStandardCost: number;
  usageUserCost: number;
  latestRecentUseLabel: string | null;
};

function parseCapacityLabel(label: string): { available: number; total: number } {
  const match = label.match(/(\d+)\s*\/\s*(\d+)/);
  if (!match) {
    return { available: 0, total: 0 };
  }
  return {
    available: Number(match[1]),
    total: Number(match[2]),
  };
}

function parseUsageWindowBadges(badges: string[]): {
  requestCount: number;
  secondaryCount: number;
  standardCost: number;
  userCost: number;
} {
  let requestCount = 0;
  let secondaryCount = 0;
  let standardCost = 0;
  let userCost = 0;

  for (const badge of badges) {
    const normalized = badge.trim();
    if (/req$/i.test(normalized)) {
      const match = normalized.match(/(\d+)/);
      requestCount += match ? Number(match[1]) : 0;
      continue;
    }
    if (/^A\s+\$/i.test(normalized)) {
      const match = normalized.match(/\$([0-9]+(?:\.[0-9]+)?)/);
      standardCost += match ? Number(match[1]) : 0;
      continue;
    }
    if (/^U\s+\$/i.test(normalized)) {
      const match = normalized.match(/\$([0-9]+(?:\.[0-9]+)?)/);
      userCost += match ? Number(match[1]) : 0;
      continue;
    }
    if (/^\d+$/.test(normalized)) {
      secondaryCount += Number(normalized);
    }
  }

  return {
    requestCount,
    secondaryCount,
    standardCost,
    userCost,
  };
}

function recentUseRank(label: string): number | null {
  const normalized = label.trim();
  if (normalized.includes("刚刚")) {
    return 0;
  }
  if (normalized.includes("从未")) {
    return Number.POSITIVE_INFINITY;
  }
  let match = normalized.match(/(\d+)\s*分钟前/);
  if (match) {
    return Number(match[1]);
  }
  match = normalized.match(/(\d+)\s*小时前/);
  if (match) {
    return Number(match[1]) * 60;
  }
  match = normalized.match(/(\d+)\s*天前/);
  if (match) {
    return Number(match[1]) * 60 * 24;
  }
  return null;
}

function summarizePilotAccounts(accounts: AccountsLedgerPilotAccount[]): PilotAggregateSummary {
  const summary: PilotAggregateSummary = {
    totalCount: accounts.length,
    capacityAvailable: 0,
    capacityTotal: 0,
    normalCount: 0,
    dispatchCount: 0,
    usageRequestCount: 0,
    usageSecondaryCount: 0,
    usageStandardCost: 0,
    usageUserCost: 0,
    latestRecentUseLabel: null,
  };
  let latestRecentUseRank = Number.POSITIVE_INFINITY;

  for (const account of accounts) {
    const capacity = parseCapacityLabel(account.capacityLabel);
    summary.capacityAvailable += capacity.available;
    summary.capacityTotal += capacity.total;
    if (account.statusLabel === "正常") {
      summary.normalCount += 1;
    }
    if (account.dispatchEnabled) {
      summary.dispatchCount += 1;
    }
    const usage = parseUsageWindowBadges(account.usageWindowBadges);
    summary.usageRequestCount += usage.requestCount;
    summary.usageSecondaryCount += usage.secondaryCount;
    summary.usageStandardCost += usage.standardCost;
    summary.usageUserCost += usage.userCost;

    const rank = recentUseRank(account.recentUseLabel);
    if (rank !== null && rank < latestRecentUseRank) {
      latestRecentUseRank = rank;
      summary.latestRecentUseLabel = account.recentUseLabel;
    }
  }

  return summary;
}

function formatAggregateMoney(value: number): string {
  return value.toFixed(2);
}

function countAvailablePilotAccounts(accounts: AccountsLedgerPilotAccount[]): number {
  return accounts.filter(
    (account) => account.dispatchEnabled && account.statusLabel.trim() === "正常",
  ).length;
}

export function AccountsLedgerWorkspace(props: AccountsLedgerWorkspaceProps) {
  const {
    t,
    notice,
    editorLocked,
    totalAccounts,
    totalGroups,
    ungroupedCount,
    providerCount,
    visibleCount,
    rows,
    pilotSections,
    automationByProvider,
    automationBusyProviderId,
    refillByProvider,
    refillBusyProviderId,
    onOpenAddProvider,
    onOpenAddAccount,
    onOpenGroups,
    onBackToEditor,
    onAddIdentityCategory,
    onOpenGeminiManualAdd,
    onEdit,
    onRemove,
    onAddExplicit,
    onToggleDispatch,
    onUpdatePoolTargetSize,
    onToggleAutoRefill,
    onToggleAutoPrune,
    onUpdateProviderPoolTargetSize,
    onToggleProviderAutoRefill,
    onToggleProviderAutoPrune,
    onRunProviderAutomation,
    onRequestProviderRefill,
    onOpenProbe,
    onOpenStats,
    onOpenSchedule,
    onDuplicate,
  } = props;
  const hasAccounts = totalAccounts > 0;
  const pilotProviderIds = new Set(pilotSections.flatMap((section) => section.providerIds));
  const tableRows = rows.filter((row) => !pilotProviderIds.has(row.providerId));
  const hasVisibleLedgerContent = visibleCount > 0;
  const [expandedProviderId, setExpandedProviderId] = useState<string | null>(null);
  const [expandedIdentityCategoryByProvider, setExpandedIdentityCategoryByProvider] = useState<
    Record<string, string[]>
  >({});
  const [activePilotActionMenuKey, setActivePilotActionMenuKey] = useState<string | null>(null);
  const pilotMenuTriggerRefs = useRef(new Map<string, HTMLButtonElement>());
  const [poolTargetDrafts, setPoolTargetDrafts] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!activePilotActionMenuKey) {
      return;
    }

    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!(event.target instanceof Element)) {
        return;
      }
      const menuRoot = event.target.closest<HTMLElement>(".nt-pilot-menu");
      if (menuRoot?.dataset.pilotMenuKey === activePilotActionMenuKey) {
        return;
      }
      setActivePilotActionMenuKey(null);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      const trigger = pilotMenuTriggerRefs.current.get(activePilotActionMenuKey);
      setActivePilotActionMenuKey(null);
      queueMicrotask(() => trigger?.focus());
    };

    document.addEventListener("pointerdown", closeOnOutsidePointer, true);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer, true);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [activePilotActionMenuKey]);

  const toggleProvider = (section: AccountsLedgerPilotSection) => {
    setExpandedProviderId((current) => (current === section.providerId ? null : section.providerId));
  };

  const pilotMenuItems: Array<{
    id: PilotMenuActionId;
    icon: LucideIcon;
    label: [string, string];
  }> = [
    { id: "probe", icon: Play, label: ["测试连接", "Test connection"] },
    { id: "stats", icon: BarChart3, label: ["查看统计", "View stats"] },
    { id: "schedule", icon: CalendarClock, label: ["定时测试", "Scheduled tests"] },
    { id: "duplicate", icon: Copy, label: ["复制账号", "Duplicate account"] },
  ];

  const handlePilotMenuAction = (
    action: PilotMenuActionId,
    providerId: string,
    account: AccountsLedgerPilotAccount,
  ) => {
    setActivePilotActionMenuKey(null);
    switch (action) {
      case "probe":
        onOpenProbe(providerId, account);
        break;
      case "stats":
        onOpenStats(providerId, account);
        break;
      case "schedule":
        onOpenSchedule(providerId, account);
        break;
      case "duplicate":
        onDuplicate(providerId, account);
        break;
    }
  };

  const renderAggregateSummary = (summary: PilotAggregateSummary) => (
    <div className="nt-provider-tree-item__summary">
      <span className="nt-provider-tree-item__metric">
        {t(`统计 ${summary.totalCount}`, `Count ${summary.totalCount}`)}
      </span>
      <span className="nt-provider-tree-item__metric">
        {t(`容量 ${summary.capacityAvailable}/${summary.capacityTotal}`, `Capacity ${summary.capacityAvailable}/${summary.capacityTotal}`)}
      </span>
      <span className="nt-provider-tree-item__metric">
        {t(`正常 ${summary.normalCount}`, `Normal ${summary.normalCount}`)}
      </span>
      <span className="nt-provider-tree-item__metric">
        {t(`调度中 ${summary.dispatchCount}`, `Dispatching ${summary.dispatchCount}`)}
      </span>
      <span className="nt-provider-tree-item__metric">{`${summary.usageRequestCount} req`}</span>
      <span className="nt-provider-tree-item__metric">
        {`A $${formatAggregateMoney(summary.usageStandardCost)} / U $${formatAggregateMoney(summary.usageUserCost)}`}
      </span>
    </div>
  );

  const commitPoolTargetDraft = (
    policyKey: string,
    committedValue: number,
    onCommit: (nextTargetSize: number) => void,
  ) => {
    const draftValue = poolTargetDrafts[policyKey];
    if (draftValue === undefined) {
      return;
    }

    const normalized = draftValue.trim();
    if (normalized.length === 0) {
      setPoolTargetDrafts((current) => {
        const next = { ...current };
        delete next[policyKey];
        return next;
      });
      return;
    }

    const parsed = Number(normalized);
    const nextTargetSize =
      Number.isFinite(parsed) && parsed >= 1 ? Math.floor(parsed) : committedValue;

    setPoolTargetDrafts((current) => {
      const next = { ...current };
      delete next[policyKey];
      return next;
    });

    if (nextTargetSize !== committedValue) {
      onCommit(nextTargetSize);
    }
  };

  const renderPoolPolicy = (options: {
    policyKey: string;
    ownerLabel: string;
    availablePoolCount: number;
    poolTargetSize: number;
    autoRefillEnabled: boolean;
    autoPruneEnabled: boolean;
    onUpdatePoolTargetSize: (nextTargetSize: number) => void;
    onToggleAutoRefill: (nextEnabled: boolean) => void;
    onToggleAutoPrune: (nextEnabled: boolean) => void;
    automation?: ConsoleCredentialPoolAutomationProvider;
    automationBusy?: boolean;
    onRunAutomation?: () => void;
    refill?: ConsoleCredentialRefillDemand;
    refillBusy?: boolean;
    onRequestRefill?: () => void;
  }) => {
    const inputValue = poolTargetDrafts[options.policyKey] ?? String(options.poolTargetSize);

    return (
      <div
        className="nt-provider-subtab-policy"
        aria-label={t(
          `${options.ownerLabel} 号池策略`,
          `${options.ownerLabel} pool policy`,
        )}
      >
        <label className="nt-provider-subtab-policy__field">
          <span
            className="nt-provider-subtab-policy__label"
            title={t("当前可用号池 / 目标容量", "Current available pool / target")}
          >
            {t("号池", "Pool")}
          </span>
          <div className="nt-provider-subtab-policy__capacity">
            <span className="nt-provider-subtab-policy__capacity-value">
              {options.availablePoolCount}/
            </span>
            <input
              className="nt-input nt-provider-subtab-policy__input"
              type="number"
              min={1}
              inputMode="numeric"
              aria-label={t(
                `${options.ownerLabel} 目标号池容量`,
                `${options.ownerLabel} target pool size`,
              )}
              value={inputValue}
              onChange={(event) => {
                const nextValue = event.currentTarget.value;
                setPoolTargetDrafts((current) => ({
                  ...current,
                  [options.policyKey]: nextValue,
                }));
              }}
              onBlur={() =>
                commitPoolTargetDraft(
                  options.policyKey,
                  options.poolTargetSize,
                  options.onUpdatePoolTargetSize,
                )
              }
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.currentTarget.blur();
                  return;
                }
                if (event.key === "Escape") {
                  setPoolTargetDrafts((current) => {
                    const next = { ...current };
                    delete next[options.policyKey];
                    return next;
                  });
                  event.currentTarget.blur();
                }
              }}
            />
          </div>
        </label>
        <div className="nt-provider-subtab-policy__switch">
          <span
            className="nt-provider-subtab-policy__label"
            title={t("号池低于目标时自动补号", "Auto refill when the pool drops below target")}
          >
            {t("补号", "Refill")}
          </span>
          <button
            className={options.autoRefillEnabled ? "nt-switch nt-switch--on" : "nt-switch"}
            type="button"
            role="switch"
            aria-checked={options.autoRefillEnabled}
            aria-label={t(
              `${options.ownerLabel} 自动补号`,
              `${options.ownerLabel} auto refill`,
            )}
            disabled={editorLocked}
            onClick={() => options.onToggleAutoRefill(!options.autoRefillEnabled)}
          >
            <span className="nt-switch__track" aria-hidden="true">
              <span className="nt-switch__thumb" />
            </span>
          </button>
        </div>
        <div className="nt-provider-subtab-policy__switch">
          <span
            className="nt-provider-subtab-policy__label"
            title={t(
              "仅自动剔除绝对不可用的账号",
              "Only auto prune permanently unavailable accounts",
            )}
          >
            {t("剔号", "Prune")}
          </span>
          <button
            className={options.autoPruneEnabled ? "nt-switch nt-switch--on" : "nt-switch"}
            type="button"
            role="switch"
            aria-checked={options.autoPruneEnabled}
            aria-label={t(
              `${options.ownerLabel} 自动剔号`,
              `${options.ownerLabel} auto prune`,
            )}
            disabled={editorLocked}
            onClick={() => options.onToggleAutoPrune(!options.autoPruneEnabled)}
          >
            <span className="nt-switch__track" aria-hidden="true">
              <span className="nt-switch__thumb" />
            </span>
          </button>
        </div>
        {options.automation || options.refill ? (
          <div className="nt-provider-subtab-policy__automation">
            <span
              className={
                options.automation?.driverConfigured || options.refill?.inquiryEnabled
                  ? "nt-badge nt-badge--info"
                  : "nt-badge nt-badge--warning"
              }
            >
              {options.automation?.driverConfigured
                ? t(
                    `驱动 ${options.automation.driverId} · ${options.automation.driverMode}`,
                    `Driver ${options.automation.driverId} · ${options.automation.driverMode}`,
                  )
                : options.refill?.inquiryEnabled
                  ? t("补号队列已就绪", "Refill queue ready")
                  : t("未配置补号执行通道", "Refill execution channel not configured")}
            </span>
            {options.refill ? (
              <span className="nt-provider-subtab-policy__modes">
                <span
                  className={
                    options.refill.notificationEnabled
                      ? "nt-badge nt-badge--info"
                      : "nt-badge nt-badge--muted"
                  }
                  title={t(
                    "Gateway 判断缺口后向 Redis Stream 投递补号任务",
                    "Gateway publishes a refill task to Redis Stream when it detects a deficit",
                  )}
                >
                  {options.refill.notificationEnabled
                    ? t("通知型开启", "Notify on")
                    : t("通知型关闭", "Notify off")}
                </span>
                <span
                  className={
                    options.refill.inquiryEnabled
                      ? "nt-badge nt-badge--info"
                      : "nt-badge nt-badge--muted"
                  }
                >
                  {options.refill.inquiryEnabled
                    ? t("询问型开启", "Inquiry on")
                    : t("询问型关闭", "Inquiry off")}
                </span>
                <span
                  className={
                    options.refill.userRequestEnabled
                      ? "nt-badge nt-badge--info"
                      : "nt-badge nt-badge--muted"
                  }
                >
                  {options.refill.userRequestEnabled
                    ? t("主动型开启", "User request on")
                    : t("主动型关闭", "User request off")}
                </span>
              </span>
            ) : null}
            {options.refill?.outstandingTaskId ? (
              <span className="nt-provider-subtab-policy__run-result">
                {t("补号任务", "Refill task")} {options.refill.outstandingTaskState} ·
                {options.refill.outstandingTaskId.slice(0, 8)}
              </span>
            ) : options.automation?.lastRunAt ? (
              <span className="nt-provider-subtab-policy__run-result">
                {t("上次", "Last")} {options.automation.lastRunAt} · {options.automation.state}
                {options.automation.createdCount > 0 || options.automation.prunedCount > 0
                  ? ` · +${options.automation.createdCount} / -${options.automation.prunedCount}`
                  : ""}
              </span>
            ) : null}
            {options.onRunAutomation && options.automation?.driverConfigured ? (
              <button
                className="nt-btn nt-btn--secondary nt-btn--compact"
                type="button"
                disabled={
                  editorLocked ||
                  options.automationBusy ||
                  (!options.autoRefillEnabled && !options.autoPruneEnabled)
                }
                onClick={options.onRunAutomation}
              >
                {options.automationBusy
                  ? t("执行中…", "Running…")
                  : t("运行直连", "Run direct")}
              </button>
            ) : null}
            {options.onRequestRefill && options.refill?.userRequestEnabled ? (
              <button
                className="nt-btn nt-btn--secondary nt-btn--compact"
                type="button"
                disabled={editorLocked || options.refillBusy}
                onClick={options.onRequestRefill}
              >
                {options.refillBusy
                  ? t("投递中…", "Publishing…")
                  : t("主动补号", "Request refill")}
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    );
  };

  const renderAccountTable = (
    section: AccountsLedgerPilotSection,
    accounts: AccountsLedgerPilotAccount[],
    labels: {
      emptyTitle: string;
      emptyBody: string;
      regionLabel: string;
      tableLabel: string;
    },
    showHead = true,
  ) => {
    if (accounts.length === 0) {
      return (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h3>{labels.emptyTitle}</h3>
          <p className="nt-copy">{labels.emptyBody}</p>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={editorLocked}
              onClick={() => onAddExplicit(section.providerId)}
            >
              <Plus size={15} aria-hidden="true" />
              {t("添加显式账号", "Add explicit account")}
            </button>
          </div>
        </article>
      );
    }

    return (
      <section className="nt-provider-subtab-panel" role="region" aria-label={labels.regionLabel}>
        <div className="nt-provider-account-branch">
          <div className="nt-provider-account-table" role="table" aria-label={labels.tableLabel}>
            {showHead ? (
              <div className="nt-provider-account-table__head" role="row">
                <span role="columnheader">{t("账号 ID", "Account ID")}</span>
                <span role="columnheader">{t("分组", "Group")}</span>
                <span role="columnheader">{t("容量", "Capacity")}</span>
                <span role="columnheader">{t("状态", "Status")}</span>
                <span role="columnheader">{t("验证", "Verification")}</span>
                <span role="columnheader">{t("调度", "Dispatch")}</span>
                <span role="columnheader">{t("用量窗口", "Usage window")}</span>
                <span role="columnheader">{t("最近使用", "Recent use")}</span>
                <span role="columnheader">{t("操作", "Actions")}</span>
              </div>
            ) : null}
            {accounts.map((account) => (
              <div
                className="nt-provider-account-table__row nt-provider-account-row"
                role="row"
                key={`${account.providerId}:${account.accountId}`}
              >
                <div className="nt-provider-account-cell" role="cell">
                  <span className="nt-provider-account-cell__value">
                    {account.mode === "provider-default" ? account.displayName : account.accountId}
                  </span>
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--logic"
                  role="cell"
                >
                  {account.logicalLabels.length > 0 ? (
                    <div className="nt-ledger-chip-list">
                      {account.logicalLabels.map((label) => (
                        <span className="nt-chip" key={`${account.accountId}:${label}`}>
                          {label}
                        </span>
                      ))}
                    </div>
                  ) : (
                    <span className="nt-provider-account-cell__muted">
                      {t("未分组", "Ungrouped")}
                    </span>
                  )}
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--capacity"
                  role="cell"
                >
                  <span className="nt-chip nt-chip--muted">{account.capacityLabel}</span>
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--status"
                  role="cell"
                >
                  <span
                    className={
                      account.dispatchEnabled
                        ? "nt-badge nt-badge--success"
                        : "nt-badge nt-badge--warning"
                    }
                  >
                    {account.statusLabel}
                  </span>
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--verification"
                  role="cell"
                >
                  <ActionTooltip label={account.verificationNote}>
                    <span
                      className={`nt-badge nt-badge--verification-${account.verificationStatus}`}
                      title={account.verificationNote}
                    >
                      {account.verificationStatus === "verified"
                        ? t("已验证", "Verified")
                        : account.verificationStatus === "failed"
                          ? t("失败", "Failed")
                          : account.verificationStatus === "blocked"
                            ? t("阻塞", "Blocked")
                            : t("未测试", "Not tested")}
                    </span>
                  </ActionTooltip>
                  {account.verificationFamilies.length > 0 ? (
                    <div className="nt-ledger-chip-list nt-ledger-chip-list--dense">
                      {account.verificationFamilies.map((family) => (
                        <span className="nt-chip nt-chip--muted" key={`${account.accountId}:family:${family}`}>
                          {family}
                        </span>
                      ))}
                    </div>
                  ) : null}
                  {account.verificationEvidenceRef ? (
                    <span
                      className="nt-provider-account-cell__muted nt-provider-account-cell__evidence"
                      title={account.verificationCheckedAt ?? undefined}
                    >
                      {account.verificationEvidenceRef}
                    </span>
                  ) : null}
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--dispatch"
                  role="cell"
                >
                  <button
                    className={account.dispatchEnabled ? "nt-switch nt-switch--on" : "nt-switch"}
                    type="button"
                    role="switch"
                    aria-checked={account.dispatchEnabled}
                    aria-label={t(
                      `调度 ${account.accountId}`,
                      `Dispatch ${account.accountId}`,
                    )}
                    disabled={editorLocked || !account.dispatchEditable}
                    onClick={() =>
                      onToggleDispatch(account.providerId, account.accountId, !account.dispatchEnabled)
                    }
                  >
                    <span className="nt-switch__track" aria-hidden="true">
                      <span className="nt-switch__thumb" />
                    </span>
                  </button>
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--usage"
                  role="cell"
                >
                  <div className="nt-ledger-chip-list nt-ledger-chip-list--dense">
                    {account.usageWindowBadges.map((badge, index) => (
                      <span
                        className="nt-chip nt-chip--muted"
                        key={`${account.accountId}:usage:${index}`}
                      >
                        {badge}
                      </span>
                    ))}
                  </div>
                </div>
                <div className="nt-provider-account-cell" role="cell">
                  <span className="nt-provider-account-cell__value">{account.recentUseLabel}</span>
                </div>
                <div
                  className="nt-provider-account-cell nt-provider-account-cell--actions"
                  role="cell"
                >
                  {account.mode === "provider-default" ? (
                    <div className="nt-actions">
                      <button
                        className="nt-btn nt-btn--secondary"
                        type="button"
                        disabled={editorLocked}
                        aria-label={t(
                          `为 ${section.providerLabel} 添加显式账号`,
                          `Add explicit account for ${section.providerLabel}`,
                        )}
                        onClick={() => onAddExplicit(account.providerId)}
                      >
                        {t("添加显式账号", "Add explicit account")}
                      </button>
                    </div>
                  ) : (
                    <div className="nt-provider-account-action-bar">
                      <ActionTooltip
                        label={t(
                          `编辑账号 ${account.displayName}`,
                          `Edit account ${account.displayName}`,
                        )}
                      >
                        <button
                          className="nt-icon-action"
                          type="button"
                          disabled={editorLocked || account.previewOnly}
                          aria-label={t(
                            `编辑账号 ${account.displayName}`,
                            `Edit account ${account.displayName}`,
                          )}
                          onClick={() => onEdit(account.providerId, account.accountId)}
                        >
                          <Pencil size={14} aria-hidden="true" />
                          <span>{t("编辑", "Edit")}</span>
                        </button>
                      </ActionTooltip>
                      <ActionTooltip
                        label={t(
                          `删除账号 ${account.displayName}`,
                          `Delete account ${account.displayName}`,
                        )}
                      >
                        <button
                          className="nt-icon-action"
                          type="button"
                          disabled={editorLocked || account.previewOnly}
                          aria-label={t(
                            `删除账号 ${account.displayName}`,
                            `Delete account ${account.displayName}`,
                          )}
                          onClick={() =>
                            onRemove(account.providerId, account.accountId, account.displayName)
                          }
                        >
                          <Trash2 size={14} aria-hidden="true" />
                          <span>{t("删除", "Delete")}</span>
                        </button>
                      </ActionTooltip>
                      <div
                        className="nt-pilot-menu"
                        data-pilot-menu-key={`${account.providerId}:${account.accountId}`}
                      >
                        <ActionTooltip
                          label={t(
                            `更多操作 ${account.accountId}`,
                            `More actions ${account.accountId}`,
                          )}
                        >
                          <button
                            className="nt-icon-action"
                            type="button"
                            ref={(node) => {
                              const menuKey = `${account.providerId}:${account.accountId}`;
                              if (node) {
                                pilotMenuTriggerRefs.current.set(menuKey, node);
                              } else {
                                pilotMenuTriggerRefs.current.delete(menuKey);
                              }
                            }}
                            aria-haspopup="menu"
                            aria-expanded={
                              activePilotActionMenuKey === `${account.providerId}:${account.accountId}`
                            }
                            aria-label={t(
                              `更多操作 ${account.accountId}`,
                              `More actions ${account.accountId}`,
                            )}
                            onClick={() =>
                              setActivePilotActionMenuKey((current) =>
                                current === `${account.providerId}:${account.accountId}`
                                  ? null
                                  : `${account.providerId}:${account.accountId}`,
                              )
                            }
                          >
                            <Ellipsis size={14} aria-hidden="true" />
                            <span>{t("更多", "More")}</span>
                          </button>
                        </ActionTooltip>
                        {activePilotActionMenuKey === `${account.providerId}:${account.accountId}` ? (
                          <div className="nt-pilot-menu__panel" role="menu">
                            {pilotMenuItems.map((item) => {
                              const Icon = item.icon;
                              return (
                                <button
                                  className="nt-pilot-menu__item"
                                  key={item.id}
                                  role="menuitem"
                                  type="button"
                                  onClick={() =>
                                    handlePilotMenuAction(item.id, account.providerId, account)
                                  }
                                >
                                  <Icon size={15} aria-hidden="true" />
                                  <span>{t(item.label[0], item.label[1])}</span>
                                </button>
                              );
                            })}
                          </div>
                        ) : null}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>
    );
  };

  return (
    <div className="nt-stack">
      {notice}
      <article className="nt-card nt-card--panel nt-ledger-command-bar">
        <div>
          <p className="nt-kicker">// Provider and account pool</p>
          <h2>{t("服务商与账号聚合", "Provider and account aggregation")}</h2>
          <p className="nt-copy">
            {t(
              `${providerCount} 个服务商 · ${totalAccounts} 个账号 · ${totalGroups} 个账号组 · ${ungroupedCount} 个未分组账号`,
              `${providerCount} providers · ${totalAccounts} accounts · ${totalGroups} account groups · ${ungroupedCount} ungrouped accounts`,
            )}
          </p>
        </div>
        <div className="nt-actions nt-actions--right">
          <button
            className="nt-btn nt-btn--secondary"
            type="button"
            disabled={editorLocked}
            onClick={onOpenAddProvider}
          >
            {t("添加服务商", "Add provider")}
          </button>
          <button
            className="nt-btn nt-btn--outline"
            type="button"
            disabled={editorLocked || providerCount === 0}
            onClick={onOpenAddAccount}
          >
            {t("添加账号", "Add account")}
          </button>
          <button className="nt-btn nt-btn--outline" type="button" onClick={onOpenGroups}>
            {t("管理账号组", "Manage account groups")}
          </button>
          <button className="nt-btn nt-btn--outline" type="button" onClick={onBackToEditor}>
            {t("高级编辑", "Advanced editor")}
          </button>
        </div>
      </article>
      {pilotSections.map((section) => {
        const expanded = expandedProviderId === section.providerId;
        const expandedCategoryIds = expandedIdentityCategoryByProvider[section.providerId] ?? [];
        const firstExpandedCategoryId =
          section.identityCategories.find((category) => expandedCategoryIds.includes(category.id))
            ?.id ?? null;
        const providerAccounts = section.supportsIdentityCategories
          ? section.identityCategories.flatMap((category) => category.accounts)
          : section.directAccounts;
        const providerSummary = summarizePilotAccounts(
          providerAccounts,
        );
        const providerAvailablePoolCount = countAvailablePilotAccounts(providerAccounts);
        const providerCompatibility =
          section.providerCompatibility ??
          providerCatalogClassification(section.providerId, section.providerPreset)
            ?.compatibility ??
          null;

        return (
          <article className="nt-card nt-card--panel nt-provider-pilot" key={section.providerId}>
            <div className="nt-provider-subtab-row nt-provider-pilot__row">
              <button
                className="nt-provider-pilot__toggle nt-provider-tree-item nt-provider-tree-item--level-0"
                type="button"
                aria-label={section.providerLabel}
                aria-expanded={expanded}
                onClick={() => toggleProvider(section)}
              >
                <div className="nt-provider-tree-item__main">
                  <strong>{section.providerLabel}</strong>
                  {providerCompatibility === "openai" ? (
                    <span className="nt-badge nt-badge--info">
                      {t("第三方 OpenAI 兼容", "Third-party OpenAI-compatible")}
                    </span>
                  ) : null}
                </div>
                {renderAggregateSummary(providerSummary)}
                <span className="nt-provider-pilot__toggle-indicator" aria-hidden="true">
                  {expanded ? "−" : "+"}
                </span>
              </button>
              {renderPoolPolicy({
                policyKey: `provider:${section.providerId}`,
                ownerLabel: section.providerLabel,
                availablePoolCount: providerAvailablePoolCount,
                poolTargetSize: section.poolTargetSize,
                autoRefillEnabled: section.autoRefillEnabled,
                autoPruneEnabled: section.autoPruneEnabled,
                onUpdatePoolTargetSize: (nextTargetSize) =>
                  onUpdateProviderPoolTargetSize(section.providerId, nextTargetSize),
                onToggleAutoRefill: (nextEnabled) =>
                  onToggleProviderAutoRefill(section.providerId, nextEnabled),
                onToggleAutoPrune: (nextEnabled) =>
                  onToggleProviderAutoPrune(section.providerId, nextEnabled),
                automation: automationByProvider.get(section.providerId),
                automationBusy: automationBusyProviderId === section.providerId,
                onRunAutomation: () => onRunProviderAutomation(section.providerId),
                refill: refillByProvider.get(section.providerId),
                refillBusy: refillBusyProviderId === section.providerId,
                onRequestRefill: () => onRequestProviderRefill(section.providerId),
              })}
            </div>

            {expanded ? (
              <div className="nt-provider-pilot__body">
                <div className="nt-actions nt-actions--right">
                  {section.manualAddFamily ? (
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={editorLocked}
                      onClick={() =>
                        onOpenGeminiManualAdd(section.manualAddFamily!, section.providerId)
                      }
                    >
                      {t("手动添加", "Manual add")}
                    </button>
                  ) : null}
                  {section.supportsIdentityCategories ? (
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={editorLocked}
                      onClick={() => onAddIdentityCategory(section.providerId)}
                    >
                      {t("添加账号类别", "Add identity class")}
                    </button>
                  ) : null}
                  <button
                    className="nt-btn nt-btn--secondary"
                    type="button"
                    disabled={editorLocked}
                    onClick={() => onAddExplicit(section.providerId)}
                  >
                    {t("添加显式账号", "Add explicit account")}
                  </button>
                </div>

                <div className="nt-provider-subtabs">
                  {section.identityCategories.map((category) => {
                    const active = expandedCategoryIds.includes(category.id);
                    const showAccountTableHead = category.id === firstExpandedCategoryId;
                    const categorySummary = summarizePilotAccounts(category.accounts);
                    const availablePoolCount = countAvailablePilotAccounts(category.accounts);
                    return (
                      <div className="nt-provider-subtab-stack" key={`${section.providerId}:${category.id}`}>
                        <div className="nt-provider-subtab-row">
                          <button
                            className={
                              active
                                ? "nt-provider-subtab nt-provider-subtab--active nt-provider-tree-item nt-provider-tree-item--level-1"
                                : "nt-provider-subtab nt-provider-tree-item nt-provider-tree-item--level-1"
                            }
                            type="button"
                            aria-label={category.label}
                            aria-expanded={active}
                            onClick={() =>
                              setExpandedIdentityCategoryByProvider((current) => {
                                const expandedIds = current[section.providerId] ?? [];
                                const nextExpandedIds = expandedIds.includes(category.id)
                                  ? expandedIds.filter((id) => id !== category.id)
                                  : [...expandedIds, category.id];
                                return {
                                  ...current,
                                  [section.providerId]: nextExpandedIds,
                                };
                              })
                            }
                          >
                            <div className="nt-provider-tree-item__main">
                              <strong>{category.label}</strong>
                            </div>
                            {renderAggregateSummary(categorySummary)}
                            <div className="nt-provider-subtab__meta">
                              <span className="nt-provider-subtab__indicator" aria-hidden="true">
                                {active ? "−" : "+"}
                              </span>
                            </div>
                          </button>
                          {renderPoolPolicy({
                            policyKey: `category:${section.providerId}:${category.id}`,
                            ownerLabel: category.label,
                            availablePoolCount,
                            poolTargetSize: category.poolTargetSize,
                            autoRefillEnabled: category.autoRefillEnabled,
                            autoPruneEnabled: category.autoPruneEnabled,
                            onUpdatePoolTargetSize: (nextTargetSize) =>
                              onUpdatePoolTargetSize(section.providerId, category.id, nextTargetSize),
                            onToggleAutoRefill: (nextEnabled) =>
                              onToggleAutoRefill(section.providerId, category.id, nextEnabled),
                            onToggleAutoPrune: (nextEnabled) =>
                              onToggleAutoPrune(section.providerId, category.id, nextEnabled),
                          })}
                        </div>
                        {active
                          ? renderAccountTable(
                              section,
                              category.accounts,
                              {
                                emptyTitle: category.label,
                                emptyBody: section.hasExplicitAccounts
                                  ? t(
                                      "当前还没有账号归入这个类别。",
                                      "No accounts are assigned to this class yet.",
                                    )
                                  : t(
                                      "先添加显式账号，随后再把账号归入这个子分类。",
                                      "Add explicit accounts first, then assign them to this subcategory.",
                                    ),
                                regionLabel: t(
                                  `${section.providerLabel} ${category.label} 账号`,
                                  `${section.providerLabel} ${category.label} accounts`,
                                ),
                                tableLabel: t(
                                  `${section.providerLabel} ${category.label} 账号表`,
                                  `${section.providerLabel} ${category.label} account table`,
                                ),
                              },
                              showAccountTableHead,
                            )
                          : null}
                      </div>
                    );
                  })}
                  {section.directAccounts.length > 0
                    ? renderAccountTable(
                        section,
                        section.directAccounts,
                        {
                          emptyTitle: section.providerLabel,
                          emptyBody: t(
                            "当前还没有可显示的账号。",
                            "There are no accounts to show right now.",
                          ),
                          regionLabel: t(
                            `${section.providerLabel} 账号`,
                            `${section.providerLabel} accounts`,
                          ),
                          tableLabel: t(
                            `${section.providerLabel} 账号表`,
                            `${section.providerLabel} account table`,
                          ),
                        },
                        true,
                      )
                    : null}
                </div>
              </div>
            ) : null}
          </article>
        );
      })}

      {tableRows.length > 0 ? (
        <section
          className="nt-ledger-table"
          role="table"
          aria-label={t("账号台账表", "Account ledger table")}
        >
          <div className="nt-ledger-table__head" role="row">
            <span role="columnheader">{t("账号", "Account")}</span>
            <span role="columnheader">Provider</span>
            <span role="columnheader">{t("状态", "Status")}</span>
            <span role="columnheader">{t("操作", "Actions")}</span>
          </div>

          {tableRows.map((row) => (
            <div className="nt-ledger-table__row" role="row" key={row.accountId}>
              <div className="nt-ledger-cell nt-ledger-cell--account" role="cell">
                <strong>{row.displayName}</strong>
                <span>{row.accountId}</span>
                <span>
                  {row.hostLabel ?? t("未配置地址", "No base URL")}
                  {row.providerPreset ? ` · ${row.providerPreset}` : ""}
                </span>
              </div>
              <div className="nt-ledger-cell" role="cell">
                <strong>{row.providerLabel}</strong>
                <span>{row.vendorLabel}</span>
              </div>
              <div className="nt-ledger-cell" role="cell">
                <span
                  className={
                    row.enabled
                      ? "nt-badge nt-badge--success"
                      : "nt-badge nt-badge--warning"
                  }
                >
                  {row.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}
                </span>
              </div>
              <div className="nt-ledger-cell nt-ledger-cell--actions" role="cell">
                <div className="nt-actions">
                  {row.mode === "credential" ? (
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={editorLocked}
                      aria-label={t(`编辑账号 ${row.displayName}`, `Edit account ${row.displayName}`)}
                      onClick={() => onEdit(row.providerId, row.accountId)}
                    >
                      {t("编辑", "Edit")}
                    </button>
                  ) : (
                    <button
                      className="nt-btn nt-btn--secondary"
                      type="button"
                      disabled={editorLocked}
                      aria-label={t(
                        `为 ${row.providerLabel} 添加显式账号`,
                        `Add explicit account for ${row.providerLabel}`,
                      )}
                      onClick={() => onAddExplicit(row.providerId)}
                    >
                      {t("添加显式账号", "Add explicit account")}
                    </button>
                  )}
                </div>
              </div>
            </div>
          ))}
        </section>
      ) : null}

      {!hasAccounts ? (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h2>{t("暂无账号", "No accounts yet")}</h2>
          <p className="nt-copy">
            {t(
              "当前 route-config 里还没有解析出 provider credential。当前结构化编辑器不直接编辑 credentials，请在高级 JSON 中添加 credentials，再回到这里管理账号池。",
              "No provider credentials were found in the current route config. The structured editor does not edit credentials directly; add them in Advanced JSON, then come back here to manage the account pool.",
            )}
          </p>
        </article>
      ) : null}

      {hasAccounts && !hasVisibleLedgerContent ? (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h2>{t("没有匹配账号", "No matching accounts")}</h2>
          <p className="nt-copy">
            {t(
              "当前筛选条件没有匹配任何账号，请调整关键词或账号状态。",
              "No accounts match the current filters. Adjust the query or account status.",
            )}
          </p>
        </article>
      ) : null}
    </div>
  );
}
