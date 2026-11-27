import { Button, Dialog, Notice } from "@neuro/ui";
import type { ConsoleCredentialProbeResult, ConsoleProviderProbeResponse, ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import type { RouteManagedAccount } from "./routeAccountCatalog";
import type { buildPilotStatsView } from "./pilotStatsView";
import { PilotAccountProbePanel } from "./PilotAccountProbePanel";
import { PilotAccountStatsPanel } from "./PilotAccountStatsPanel";
import { LumaMediaProbeDialog } from "./LumaMediaProbeDialog";
import { ChataibotMediaProbeDialog } from "./ChataibotMediaProbeDialog";
import { SunoMediaProbeDialog } from "./SunoMediaProbeDialog";
import { UdioMediaProbeDialog } from "./UdioMediaProbeDialog";
import { LumalabsMediaProbeDialog } from "./LumalabsMediaProbeDialog";
import { isLumalabsMediaProvider } from "./lumalabsProbeModels";
import { isUdioMediaProvider } from "./udioProbeModels";
import { isSunoMediaProvider } from "./sunoProbeModels";
import { isChataibotMediaProvider } from "./chataibotProbeModels";
import { isLumaAgentsProvider } from "./lumaAgents";
import { isRecord } from "./routeDocument";
import { isCursorProvider } from "./cursorCatalog";


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
  activeRevision?: string | null;
  authorizationContext?: string | null;
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
  activeRevision,
  authorizationContext,
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
  const targetIds = pilotActionDialog?.kind === "probe" ? [pilotActionDialog.providerId] : testSection?.providerIds;
  if (targetIds?.length && targetIds.every(id => {
    const provider = testPolicyDocument?.providers.find(p => isRecord(p) && p.id === id);
    return isRecord(provider) && isCursorProvider(provider);
  })) {
    return <Dialog open title={t("Cursor 账号测试", "Cursor account test")} closeLabel={t("关闭弹窗", "Close dialog")}
      onOpenChange={open => { if (!open) closePilotActionDialog(); }}>
      <Notice>{t("Cursor 不支持通用探测或定时测试，未向 Cursor 发送请求。是否已启用请查看账号的调度开关。",
        "Cursor does not support generic or scheduled tests. No request was sent to Cursor. Check the account's dispatch switch to see whether it is enabled.")}</Notice>
      <div className="n-dialog-actions"><Button onClick={closePilotActionDialog}>{t("关闭", "Close")}</Button></div>
    </Dialog>;
  }
  if (testSection && pilotActionDialog) {
    if (pilotActionDialog.kind === "probe" && isLumalabsMediaProvider(testPolicyDocument?.providers.find(provider => isRecord(provider) && provider.id === pilotActionDialog.providerId))) {
      return <LumalabsMediaProbeDialog key={JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId])}
        document={testPolicyDocument ?? null} providerId={pilotActionDialog.providerId} credentialId={pilotActionDialog.account.accountId}
        t={t} onProbe={startProviderProbe} onClose={closePilotActionDialog} locked={editorLocked || providerProbeBusy}
        activeRevision={activeRevision ?? null} authorizationContext={authorizationContext ?? null} draftReady={!draftDirty && draftMatchesActiveRevision} error={providerProbeError} />;
    }
    if (pilotActionDialog.kind === "probe" && isUdioMediaProvider(testPolicyDocument?.providers.find(provider => isRecord(provider) && provider.id === pilotActionDialog.providerId))) {
      return <UdioMediaProbeDialog key={JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId])}
        document={testPolicyDocument ?? null} providerId={pilotActionDialog.providerId} credentialId={pilotActionDialog.account.accountId}
        t={t} onProbe={startProviderProbe} onClose={closePilotActionDialog} locked={editorLocked || providerProbeBusy}
        activeRevision={activeRevision ?? null} authorizationContext={authorizationContext ?? null} draftReady={!draftDirty && draftMatchesActiveRevision} error={providerProbeError} />;
    }
    if (pilotActionDialog.kind === "probe" && isSunoMediaProvider(testPolicyDocument?.providers.find(provider => isRecord(provider) && provider.id === pilotActionDialog.providerId))) {
      return <SunoMediaProbeDialog key={JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId])}
        document={testPolicyDocument ?? null} providerId={pilotActionDialog.providerId} credentialId={pilotActionDialog.account.accountId}
        t={t} onProbe={startProviderProbe} onClose={closePilotActionDialog} locked={editorLocked || providerProbeBusy}
        activeRevision={activeRevision ?? null} authorizationContext={authorizationContext ?? null} draftReady={!draftDirty && draftMatchesActiveRevision} error={providerProbeError} />;
    }
    if (pilotActionDialog.kind === "probe" && isLumaAgentsProvider(testPolicyDocument?.providers.find(provider => isRecord(provider) && provider.id === pilotActionDialog.providerId))) {
      return <LumaMediaProbeDialog key={JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId])}
        document={testPolicyDocument ?? null} providerId={pilotActionDialog.providerId} credentialId={pilotActionDialog.account.accountId}
        t={t} onProbe={startProviderProbe} onClose={closePilotActionDialog} locked={editorLocked || providerProbeBusy}
        draftReady={!draftDirty && draftMatchesActiveRevision} error={providerProbeError} />;
    }
    if (pilotActionDialog.kind === "probe" && isChataibotMediaProvider(testPolicyDocument?.providers.find(provider => isRecord(provider) && provider.id === pilotActionDialog.providerId))) {
      return <ChataibotMediaProbeDialog key={JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId])}
        document={testPolicyDocument ?? null} providerId={pilotActionDialog.providerId} credentialId={pilotActionDialog.account.accountId}
        t={t} onProbe={startProviderProbe} onClose={closePilotActionDialog} locked={editorLocked || providerProbeBusy}
        draftReady={!draftDirty && draftMatchesActiveRevision} error={providerProbeError} />;
    }
    const TestDialog = pilotActionDialog.kind === "probe" ? ProviderTestDialog : CredentialTestManager;
    return <TestDialog key={pilotActionDialog.kind === "probe" ? JSON.stringify([pilotActionDialog.providerId, pilotActionDialog.account.accountId]) : testSection.providerId}
      onPlanChange={applyTestPlanChanges}
      initialTab={pilotActionDialog.kind === "provider-schedule" ? "auto" : "manual"}
      initialScope={pilotActionDialog.kind === "probe" ? { kind: "account", providerId: pilotActionDialog.providerId, id: pilotActionDialog.account.accountId } : undefined}
      scopeLocked={pilotActionDialog.kind === "probe"}
      document={testPolicyDocument ?? null} onSave={applyTestPolicy} onLoadResults={loadProviderProbeResults}
      onProbe={startProviderProbe}
      probe={{
        t, section: testSection, closePilotActionDialog, providerProbeBusy,
        providerProbeError, providerProbeResponse, draftDirty, draftMatchesActiveRevision
      }}
      schedule={{
        t, section: testSection, closePilotActionDialog, editorLocked,
        pilotScheduleEnabled, setPilotScheduleEnabled, pilotScheduleIntervalMinutes,
        setPilotScheduleIntervalMinutes, applyProviderProbeSchedule
      }} />;
  }
  return (pilotActionDialog ? (
    <Dialog open title={pilotActionDialogTitle(pilotActionDialog, t)} closeLabel={t("关闭弹窗", "Close dialog")}
      className="nt-pilot-dialog" onOpenChange={(open) => { if (!open) closePilotActionDialog(); }}>

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
    </Dialog>
  ) : null);
}
