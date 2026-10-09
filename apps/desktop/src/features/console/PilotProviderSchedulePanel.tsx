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
  closePilotActionDialog,
  editorLocked,
  pilotScheduleEnabled,
  setPilotScheduleEnabled,
  pilotScheduleIntervalMinutes,
  setPilotScheduleIntervalMinutes,
  applyProviderProbeSchedule,
}: PilotProviderSchedulePanelProps) {
  const interval = Number(pilotScheduleIntervalMinutes);
  const valid = Number.isInteger(interval) && interval >= 1 && interval <= 10_080;
  return (
    <div className="nt-stack">
      <div className="nt-pool-test-schedule">
        <label className="nt-pilot-schedule-toggle">
          <input
            type="checkbox"
            checked={pilotScheduleEnabled}
            disabled={editorLocked}
            onChange={(event) => setPilotScheduleEnabled(event.currentTarget.checked)}
          />
          <span>{t("启用自动测试", "Enable automatic tests")}</span>
        </label>
        <label className="nt-field">
          <span>{t("执行间隔（分钟）", "Interval (minutes)")}</span>
          <input
            className="nt-input"
            type="number"
            min={1}
            max={10_080}
            step={1}
            aria-invalid={!valid}
            aria-describedby={!valid ? "pool-test-interval-error" : undefined}
            value={pilotScheduleIntervalMinutes}
            disabled={editorLocked}
            onChange={(event) =>
              setPilotScheduleIntervalMinutes(event.currentTarget.value)
            }
          />
        </label>
      </div>
      {!valid ? <p id="pool-test-interval-error" role="status" className="nt-copy">
        {t("请输入 1–10080 的整数。", "Enter an integer from 1 to 10080.")}
      </p> : null}
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
          disabled={editorLocked || !valid}
          onClick={applyProviderProbeSchedule}
        >
          {t("保存", "Save")}
        </button>
      </div>
    </div>
  );
}
