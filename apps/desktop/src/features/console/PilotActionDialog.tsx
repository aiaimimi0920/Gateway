import { X } from "lucide-react";
import type { ConsoleCredentialProbeResult, ConsoleProviderProbeResponse, ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import type { RouteManagedAccount } from "./routeAccountCatalog";
import type { buildPilotStatsView } from "./pilotStatsView";
import { PilotAccountProbePanel } from "./PilotAccountProbePanel";
import { PilotAccountStatsPanel } from "./PilotAccountStatsPanel";


import { ProviderTestDialog } from "./ProviderTestDialog";
import { CredentialTestManager } from "./CredentialTestManager";
import type { TestPlanChange } from "./credentialTestPlansDocument";
import type { TestProbeAction, TestResultLoader } from "./useCredentialTestRuns";

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
  startProviderProbe: TestProbeAction;
  testPolicyDocument?: ConsoleRouteDocument | null;
  accountPilotSections?: AccountsLedgerPilotSection[];
  applyTestPolicy?: (scope: CredentialTestScope | CredentialTestScope[], policy: CredentialTestPolicy | null) => boolean;
  loadProviderProbeResults?: TestResultLoader;
  applyTestPlanChanges?: (changes: TestPlanChange[]) => boolean;
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
  testPolicyDocument,
  accountPilotSections,
  applyTestPolicy,
  loadProviderProbeResults,
  applyTestPlanChanges,
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
  const testSection = pilotActionDialog?.kind === "probe"
    ? accountPilotSections?.find((section) => section.providerIds.includes(pilotActionDialog.providerId))
    : pilotActionDialog?.kind === "provider-probe" || pilotActionDialog?.kind === "provider-schedule" ? pilotActionDialog.section : null;
  if (testSection && pilotActionDialog) {
    const TestDialog = pilotActionDialog.kind === "probe" ? ProviderTestDialog : CredentialTestManager;
    return <TestDialog key={pilotActionDialog.kind === "probe" ? JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId]) : testSection.providerId}
      onPlanChange={applyTestPlanChanges}
      initialTab={pilotActionDialog.kind === "provider-schedule" ? "auto" : "manual"}
      initialScope={pilotActionDialog.kind === "probe" ? { kind: "account", providerId: pilotActionDialog.providerId, id: pilotActionDialog.account.accountId } : undefined}
      scopeLocked={pilotActionDialog.kind === "probe"}
      document={testPolicyDocument ?? null} onSave={applyTestPolicy} onLoadResults={loadProviderProbeResults}
      onProbe={startProviderProbe}
      probe={{ t, section: testSection, closePilotActionDialog, providerProbeBusy,
        providerProbeError, providerProbeResponse, draftDirty, draftMatchesActiveRevision }}
      schedule={{ t, section: testSection, closePilotActionDialog, editorLocked,
        pilotScheduleEnabled, setPilotScheduleEnabled, pilotScheduleIntervalMinutes,
        setPilotScheduleIntervalMinutes, applyProviderProbeSchedule }} />;
  }
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
