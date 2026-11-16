import { Badge, Button, Checkbox, Field, Input, Notice, Panel, Stack } from "@neuro/ui";
import type { AccountsLedgerPilotAccount } from "./accountsLedgerTypes";
import type { buildPilotStatsView } from "./pilotStatsView";

type PilotAccountStatsPanelProps = {
  t: (zh: string, en: string) => string;
  account: AccountsLedgerPilotAccount;
  closePilotActionDialog: () => void;
  telemetryError: string | null;
  activePilotStatsView: ReturnType<typeof buildPilotStatsView> | null;
  editorLocked: boolean;
  pilotScheduleEnabled: boolean;
  setPilotScheduleEnabled: (value: boolean) => void;
  pilotScheduleIntervalMinutes: string;
  setPilotScheduleIntervalMinutes: (value: string) => void;
  applyPilotProbeSchedule: () => void;
};

export function PilotAccountStatsPanel({
  t,
  account,
  closePilotActionDialog,
  telemetryError,
  activePilotStatsView,
  editorLocked,
  pilotScheduleEnabled,
  setPilotScheduleEnabled,
  pilotScheduleIntervalMinutes,
  setPilotScheduleIntervalMinutes,
  applyPilotProbeSchedule,
}: PilotAccountStatsPanelProps) {
  return (
    <Stack>
      <Panel className="nt-pilot-identity">
        <div>
          <strong>{account.displayName}</strong>
        </div>
        <Badge tone={account.dispatchEnabled ? "success" : "warning"}>
          {account.dispatchEnabled ? t("启用", "Active") : t("暂停", "Paused")}
        </Badge>
      </Panel>

      {telemetryError ? (
        <Notice tone="danger">
          {telemetryError}
        </Notice>
      ) : null}
      {activePilotStatsView ? (
        <>
          <div className="nt-pilot-stats-overview-grid">
            {activePilotStatsView.headline.map((metric) => (
              <Panel className="nt-pilot-stat" key={metric.key}>
                <span>{metric.label}</span>
                <strong>{metric.value}</strong>
                {metric.hint ? <small>{metric.hint}</small> : null}
              </Panel>
            ))}
          </div>
          <div className="nt-pilot-stats-detail-grid">
            <Panel className="nt-pilot-metric">
              <h3>{t("凭据实况", "Credential state")}</h3>
              <dl className="nt-pilot-metric-list">
                {activePilotStatsView.credentialRows.map((metric) => (
                  <div key={metric.key}>
                    <dt>{metric.label}</dt>
                    <dd title={metric.hint ?? undefined}>{metric.value}</dd>
                  </div>
                ))}
              </dl>
              {activePilotStatsView.hasCredentialHealth ? null : (
                <Notice tone="warning">
                  {t(
                    "该凭据尚无健康记录，说明网关还没有用它发起过上游请求。",
                    "This credential has no health row yet, so the gateway has never dispatched an upstream request with it.",
                  )}
                </Notice>
              )}
            </Panel>
            <Panel className="nt-pilot-metric">
              <h3>{t("服务商账号聚合", "Provider account aggregates")}</h3>
              <dl className="nt-pilot-metric-list">
                {activePilotStatsView.providerRows.map((metric) => (
                  <div key={metric.key}>
                    <dt>{metric.label}</dt>
                    <dd title={metric.hint ?? undefined}>{metric.value}</dd>
                  </div>
                ))}
              </dl>
              {activePilotStatsView.hasProviderTelemetry ? (
                activePilotStatsView.sharedCredentialCount > 1 ? (
                  <Notice tone="warning">
                    {t(
                      `网关只按服务商账号记账，这些数字由该账号下 ${activePilotStatsView.sharedCredentialCount} 个凭据共享。`,
                      `The gateway only meters per provider account, so these numbers are shared by ${activePilotStatsView.sharedCredentialCount} credentials.`,
                    )}
                  </Notice>
                ) : null
              ) : (
                <Notice tone="warning">
                  {t(
                    "该服务商账号还没有产生可聚合的请求。",
                    "This provider account has no aggregated traffic yet.",
                  )}
                </Notice>
              )}
            </Panel>
          </div>
          <Panel className="nt-pilot-metric">
            <div>
              <h3>{t("定时测试", "Scheduled tests")}</h3>
            </div>
            <Checkbox label={t("启用连接测试计划", "Enable connectivity test schedule")}
              checked={pilotScheduleEnabled}
              disabled={editorLocked}
              onChange={(event) => setPilotScheduleEnabled(event.currentTarget.checked)}
            />
            <Field label={t("执行间隔（分钟）", "Interval (minutes)")}>
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
            <Button variant="primary" wrap
              disabled={editorLocked}
              onClick={applyPilotProbeSchedule}
            >
              {t("更新计划", "Update schedule")}
            </Button>
          </Panel>
        </>
      ) : null}

      <div className="n-dialog-actions">
        <Button wrap
          onClick={closePilotActionDialog}
        >
          {t("关闭", "Close")}
        </Button>
      </div>
    </Stack>
  );
}
