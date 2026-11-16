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
    <Stack>
      <div className="nt-pool-test-schedule">
        <Checkbox label={t("启用自动测试", "Enable automatic tests")}
          checked={pilotScheduleEnabled}
          disabled={editorLocked}
          onChange={(event) => setPilotScheduleEnabled(event.currentTarget.checked)}
        />
        <Field label={t("执行间隔（分钟）", "Interval (minutes)")}
          error={!valid ? t("请输入 1–10080 的整数。", "Enter an integer from 1 to 10080.") : undefined}>
          <Input compact
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
        </Field>
      </div>
      <div className="n-dialog-actions">
        <Button wrap
          onClick={closePilotActionDialog}
        >
          {t("取消", "Cancel")}
        </Button>
        <Button variant="primary" wrap
          disabled={editorLocked || !valid}
          onClick={applyProviderProbeSchedule}
        >
          {t("保存", "Save")}
        </Button>
      </div>
    </Stack>
  );
}
import { Button, Checkbox, Field, Input, Stack } from "@neuro/ui";
