import { BarChart3, CalendarClock } from "lucide-react";
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
    <div className="nt-stack">
      <article className="nt-pilot-dialog__hero">
        <div className="nt-pilot-dialog__hero-icon">
          <BarChart3 size={20} aria-hidden="true" />
        </div>
        <div className="nt-pilot-dialog__hero-copy">
          <strong>{account.displayName}</strong>
          <span>
            {t(
              "凭据健康来自实时健康表，请求与费用来自该服务商账号的聚合",
              "Credential health comes from the live health table; requests and cost come from this provider account's aggregates",
            )}
          </span>
        </div>
        <span
          className={
            account.dispatchEnabled
              ? "nt-badge nt-badge--success"
              : "nt-badge nt-badge--warning"
          }
        >
          {account.dispatchEnabled ? "active" : "paused"}
        </span>
      </article>

      {telemetryError ? (
        <div className="nt-banner nt-banner--danger" role="alert">
          {telemetryError}
        </div>
      ) : null}
      {activePilotStatsView ? (
        <>
          <div className="nt-pilot-stats-overview-grid">
            {activePilotStatsView.headline.map((metric) => (
              <article className="nt-pilot-stat-card" key={metric.key}>
                <span>{metric.label}</span>
                <strong>{metric.value}</strong>
                {metric.hint ? <small>{metric.hint}</small> : null}
              </article>
            ))}
          </div>
          <div className="nt-pilot-stats-detail-grid">
            <article className="nt-pilot-metric-card">
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
                <p className="nt-pilot-stats-note">
                  {t(
                    "该凭据尚无健康记录，说明网关还没有用它发起过上游请求。",
                    "This credential has no health row yet, so the gateway has never dispatched an upstream request with it.",
                  )}
                </p>
              )}
            </article>
            <article className="nt-pilot-metric-card">
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
                  <p className="nt-pilot-stats-note">
                    {t(
                      `网关只按服务商账号记账，这些数字由该账号下 ${activePilotStatsView.sharedCredentialCount} 个凭据共享。`,
                      `The gateway only meters per provider account, so these numbers are shared by ${activePilotStatsView.sharedCredentialCount} credentials.`,
                    )}
                  </p>
                ) : null
              ) : (
                <p className="nt-pilot-stats-note">
                  {t(
                    "该服务商账号还没有产生可聚合的请求。",
                    "This provider account has no aggregated traffic yet.",
                  )}
                </p>
              )}
            </article>
          </div>
          <article className="nt-pilot-schedule-panel">
            <div className="nt-pilot-schedule-panel__head">
              <CalendarClock size={18} aria-hidden="true" />
              <h3>{t("定时测试", "Scheduled tests")}</h3>
            </div>
            <label className="nt-pilot-schedule-toggle">
              <input
                type="checkbox"
                checked={pilotScheduleEnabled}
                disabled={editorLocked}
                onChange={(event) => setPilotScheduleEnabled(event.currentTarget.checked)}
              />
              <span>{t("启用连接测试计划", "Enable connectivity test schedule")}</span>
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
            <button
              className="nt-btn nt-btn--primary"
              type="button"
              disabled={editorLocked}
              onClick={applyPilotProbeSchedule}
            >
              {t("更新计划", "Update schedule")}
            </button>
          </article>
        </>
      ) : null}

      <div className="dialog-actions">
        <button
          className="nt-btn nt-btn--secondary"
          type="button"
          onClick={closePilotActionDialog}
        >
          {t("关闭", "Close")}
        </button>
      </div>
    </div>
  );
}
