import { CalendarClock } from "lucide-react";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";

type PilotProviderSchedulePanelProps = {
  t: (zh: string, en: string) => string;
  section: AccountsLedgerPilotSection;
  closePilotActionDialog: () => void;
  editorLocked: boolean;
  pilotScheduleEnabled: boolean;
  setPilotScheduleEnabled: (value: boolean) => void;
  pilotScheduleIntervalMinutes: string;
  setPilotScheduleIntervalMinutes: (value: string) => void;
  applyProviderProbeSchedule: () => void;
};

export function PilotProviderSchedulePanel({
  t,
  section,
  closePilotActionDialog,
  editorLocked,
  pilotScheduleEnabled,
  setPilotScheduleEnabled,
  pilotScheduleIntervalMinutes,
  setPilotScheduleIntervalMinutes,
  applyProviderProbeSchedule,
}: PilotProviderSchedulePanelProps) {
  return (
    <div className="nt-stack">
      <article className="nt-pilot-dialog__hero">
        <div className="nt-pilot-dialog__hero-icon">
          <CalendarClock size={20} aria-hidden="true" />
        </div>
        <div className="nt-pilot-dialog__hero-copy">
          <strong>{section.providerLabel}</strong>
          <span>
            {t(
              "该计划覆盖此服务商的全部账号。Gateway 使用分布式时间槽去重，并顺序执行非生成单点测试。",
              "This schedule covers every provider account. Gateway deduplicates distributed time slots and runs non-generative single-point tests sequentially.",
            )}
          </span>
        </div>
      </article>
      <article className="nt-pilot-schedule-panel">
        <div className="nt-pilot-schedule-panel__head">
          <CalendarClock size={18} aria-hidden="true" />
          <h3>{t("服务商级自动测试", "Provider-wide automatic tests")}</h3>
        </div>
        <label className="nt-pilot-schedule-toggle">
          <input
            type="checkbox"
            checked={pilotScheduleEnabled}
            disabled={editorLocked}
            onChange={(event) => setPilotScheduleEnabled(event.currentTarget.checked)}
          />
          <span>{t("启用全部账号的自动单点测试", "Enable automatic tests for all accounts")}</span>
        </label>
        <label className="nt-field">
          <span>{t("执行间隔（分钟）", "Interval (minutes)")}</span>
          <input
            className="nt-input"
            type="number"
            min={1}
            max={10_080}
            step={1}
            value={pilotScheduleIntervalMinutes}
            disabled={editorLocked}
            onChange={(event) =>
              setPilotScheduleIntervalMinutes(event.currentTarget.value)
            }
          />
        </label>
      </article>
      <p className="nt-copy">
        {t(
          "更新只写入当前路由草稿；保存路由配置后，后台调度器才会读取新计划。",
          "Updating writes only to the current route draft; the background scheduler reads it after the route config is saved.",
        )}
      </p>
      <div className="dialog-actions">
        <button
          className="nt-btn nt-btn--secondary"
          type="button"
          onClick={closePilotActionDialog}
        >
          {t("取消", "Cancel")}
        </button>
        <button
          className="nt-btn nt-btn--primary"
          type="button"
          disabled={editorLocked}
          onClick={applyProviderProbeSchedule}
        >
          {t("更新自动测试计划", "Update automatic test schedule")}
        </button>
      </div>
    </div>
  );
}
