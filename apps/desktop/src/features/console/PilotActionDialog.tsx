import { X } from "lucide-react";
import type { ConsoleCredentialProbeResult, ConsoleProviderProbeResponse } from "../../api/contracts";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import type { RouteManagedAccount } from "./routeAccountCatalog";
import type { buildPilotStatsView } from "./pilotStatsView";
import { PilotAccountProbePanel } from "./PilotAccountProbePanel";
import { PilotAccountStatsPanel } from "./PilotAccountStatsPanel";
import { PilotProviderProbePanel } from "./PilotProviderProbePanel";
import { PilotProviderSchedulePanel } from "./PilotProviderSchedulePanel";

export type PilotActionDialogState =
  | {
      kind: "probe";
      providerId: string;
      account: AccountsLedgerPilotAccount;
    }
  | {
      kind: "stats";
      providerId: string;
      account: AccountsLedgerPilotAccount;
    }
  | {
      kind: "provider-probe";
      section: AccountsLedgerPilotSection;
    }
  | {
      kind: "provider-schedule";
      section: AccountsLedgerPilotSection;
    };

function pilotActionDialogTitle(
  dialog: PilotActionDialogState,
  t: (zh: string, en: string) => string,
): string {
  switch (dialog.kind) {
    case "probe":
      return t("账号单点测试", "Account single-point test");
    case "stats":
      return t("查看账号统计", "View account stats");
    case "provider-probe":
      return t("服务商测试", "Provider test");
    case "provider-schedule":
      return t("自动定时测试", "Automatic scheduled tests");
  }
}

type PilotActionDialogProps = {
  t: (zh: string, en: string) => string;
  pilotActionDialog: PilotActionDialogState | null;
  closePilotActionDialog: () => void;
  credentialProbeBusy: string | null;
  activePilotManagedAccount: RouteManagedAccount | null;
  activePilotProbeResult: Pick<ConsoleCredentialProbeResult, "message" | "probePoint"> | null;
  draftDirty: boolean;
  draftMatchesActiveRevision: boolean;
  startPilotProbe: () => Promise<void>;
  providerProbeBusy: boolean;
  providerProbeError: string | null;
  providerProbeResponse: ConsoleProviderProbeResponse | null;
  startProviderProbe: () => Promise<void>;
  editorLocked: boolean;
  pilotScheduleEnabled: boolean;
  setPilotScheduleEnabled: (value: boolean) => void;
  pilotScheduleIntervalMinutes: string;
  setPilotScheduleIntervalMinutes: (value: string) => void;
  applyProviderProbeSchedule: () => void;
  telemetryError: string | null;
  activePilotStatsView: ReturnType<typeof buildPilotStatsView> | null;
  applyPilotProbeSchedule: () => void;
};

// Session, probe, and draft hooks retain state and effects; panels only render it.
export function PilotActionDialog({
  t,
  pilotActionDialog,
  closePilotActionDialog,
  credentialProbeBusy,
  activePilotManagedAccount,
  activePilotProbeResult,
  draftDirty,
  draftMatchesActiveRevision,
  startPilotProbe,
  providerProbeBusy,
  providerProbeError,
  providerProbeResponse,
  startProviderProbe,
  editorLocked,
  pilotScheduleEnabled,
  setPilotScheduleEnabled,
  pilotScheduleIntervalMinutes,
  setPilotScheduleIntervalMinutes,
  applyProviderProbeSchedule,
  telemetryError,
  activePilotStatsView,
  applyPilotProbeSchedule,
}: PilotActionDialogProps) {
  return (pilotActionDialog ? (
        <>
          <div className="dialog-overlay" onClick={closePilotActionDialog} />
          <section
            className="dialog-content nt-pilot-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={pilotActionDialogTitle(pilotActionDialog, t)}
          >
            <div className="nt-pilot-dialog__header">
              <h2>{pilotActionDialogTitle(pilotActionDialog, t)}</h2>
              <button
                className="nt-icon-close"
                type="button"
                aria-label={t("关闭", "Close")}
                onClick={closePilotActionDialog}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>

            {pilotActionDialog.kind === "probe" ? (
              <PilotAccountProbePanel
                t={t}
                account={pilotActionDialog.account}
                closePilotActionDialog={closePilotActionDialog}
                credentialProbeBusy={credentialProbeBusy}
                activePilotManagedAccount={activePilotManagedAccount}
                activePilotProbeResult={activePilotProbeResult}
                draftDirty={draftDirty}
                draftMatchesActiveRevision={draftMatchesActiveRevision}
                startPilotProbe={startPilotProbe}
              />
            ) : null}

            {pilotActionDialog.kind === "provider-probe" ? (
              <PilotProviderProbePanel
                t={t}
                section={pilotActionDialog.section}
                closePilotActionDialog={closePilotActionDialog}
                providerProbeBusy={providerProbeBusy}
                providerProbeError={providerProbeError}
                providerProbeResponse={providerProbeResponse}
                draftDirty={draftDirty}
                draftMatchesActiveRevision={draftMatchesActiveRevision}
                startProviderProbe={startProviderProbe}
              />
            ) : null}

            {pilotActionDialog.kind === "provider-schedule" ? (
              <PilotProviderSchedulePanel
                t={t}
                section={pilotActionDialog.section}
                closePilotActionDialog={closePilotActionDialog}
                editorLocked={editorLocked}
                pilotScheduleEnabled={pilotScheduleEnabled}
                setPilotScheduleEnabled={setPilotScheduleEnabled}
                pilotScheduleIntervalMinutes={pilotScheduleIntervalMinutes}
                setPilotScheduleIntervalMinutes={setPilotScheduleIntervalMinutes}
                applyProviderProbeSchedule={applyProviderProbeSchedule}
              />
            ) : null}

            {pilotActionDialog.kind === "stats" ? (
              <PilotAccountStatsPanel
                t={t}
                account={pilotActionDialog.account}
                closePilotActionDialog={closePilotActionDialog}
                telemetryError={telemetryError}
                activePilotStatsView={activePilotStatsView}
                editorLocked={editorLocked}
                pilotScheduleEnabled={pilotScheduleEnabled}
                setPilotScheduleEnabled={setPilotScheduleEnabled}
                pilotScheduleIntervalMinutes={pilotScheduleIntervalMinutes}
                setPilotScheduleIntervalMinutes={setPilotScheduleIntervalMinutes}
                applyPilotProbeSchedule={applyPilotProbeSchedule}
              />
            ) : null}
          </section>
        </>
      ) : null);
}
