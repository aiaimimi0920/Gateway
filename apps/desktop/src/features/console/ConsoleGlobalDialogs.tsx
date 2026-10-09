import { useEffect } from "react";
import { supportsAccountDiscovery } from "./accountDiscovery";
import { AccountGroupDialog } from "./AccountGroupDialog";
import { CredentialDialog } from "./CredentialDialog";
import { useCredentialKeyReveal } from "./useCredentialKeyReveal";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";
import { GeminiManualAddDialog } from "./GeminiManualAddDialog";
import { PilotActionDialog } from "./PilotActionDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { ProviderModelMappingDialog } from "./ProviderModelMappingDialog";
import type { useConsoleController } from "./useConsoleController";

type ConsoleGlobalDialogsProps = {
  controller: Pick<
    ReturnType<typeof useConsoleController>,
    | "activePilotManagedAccount"
    | "discoverAccount"
    | "accountGroupDialogOpen"
    | "displayedAccountCatalog"
    | "groupDirectory"
    | "setAccountGroupDialogOpen"
    | "submitAccountGroup"
    | "activePilotProbeResult"
    | "activePilotStatsView"
    | "accountPilotSections"
    | "testPolicyDocument"
    | "applyTestPolicy"
    | "applyTestPlanChanges"
    | "loadProviderProbeResults"
    | "applyCredentialDialogValue"
    | "applyPilotProbeSchedule"
    | "applyProviderCatalogDraft"
    | "applyProviderProbeSchedule"
    | "closeGeminiManualAddDialog"
    | "closePilotActionDialog"
    | "credentialDialogState"
    | "client"
    | "refresh"
    | "routeConfig"
    | "credentialProbeBusy"
    | "credentialProviderOptions"
    | "draftDirty"
    | "draftDocumentState"
    | "draftMatchesActiveRevision"
    | "editorLocked"
    | "error"
    | "autosavePending"
    | "explicitCredentialIds"
    | "geminiManualAddDialogState"
    | "hasSecretAccess"
    | "pilotActionDialog"
    | "pilotScheduleEnabled"
    | "pilotScheduleIntervalMinutes"
    | "providerCatalogDialogOpen"
    | "providerModelMappingTarget"
    | "providerProbeBusy"
    | "providerProbeError"
    | "providerProbeResponse"
    | "requestGeminiManualAddCompletion"
    | "secretDialogOpen"
    | "session"
    | "setCredentialDialogState"
    | "setPilotScheduleEnabled"
    | "setPilotScheduleIntervalMinutes"
    | "setProviderCatalogDialogOpen"
    | "setProviderModelMappingProviderId"
    | "setSecretDialogOpen"
    | "startPilotProbe"
    | "startProviderProbe"
    | "submitProviderModelMapping"
    | "t"
    | "telemetryError"
  >;
};

// Keep cross-workspace dialogs mounted independently of the route-config gate.
export function ConsoleGlobalDialogs({ controller }: ConsoleGlobalDialogsProps) {
  const {
    activePilotManagedAccount,
    activePilotProbeResult,
    activePilotStatsView,
    applyCredentialDialogValue,
    applyPilotProbeSchedule,
    applyProviderCatalogDraft,
    applyProviderProbeSchedule,
    closeGeminiManualAddDialog,
    closePilotActionDialog,
    credentialDialogState,
    credentialProbeBusy,
    credentialProviderOptions,
    draftDirty,
    draftMatchesActiveRevision,
    editorLocked,
    explicitCredentialIds,
    geminiManualAddDialogState,
    hasSecretAccess,
    pilotActionDialog,
    pilotScheduleEnabled,
    pilotScheduleIntervalMinutes,
    providerCatalogDialogOpen,
    providerModelMappingTarget,
    providerProbeBusy,
    providerProbeError,
    providerProbeResponse,
    requestGeminiManualAddCompletion,
    secretDialogOpen,
    session,
    setCredentialDialogState,
    setPilotScheduleEnabled,
    setPilotScheduleIntervalMinutes,
    setProviderCatalogDialogOpen,
    setProviderModelMappingProviderId,
    setSecretDialogOpen,
    startPilotProbe,
    startProviderProbe,
    submitProviderModelMapping,
    t,
    telemetryError,
  } = controller;

  const revealCredentialKey = useCredentialKeyReveal(controller.client, session.managementToken,
    session.secretGrant?.grant ?? null, controller.routeConfig?.routeConfig.revision.id,
    credentialDialogState?.initialValue?.providerId, credentialDialogState?.initialValue?.credentialId);

  useEffect(() => { if (secretDialogOpen && session.secretGrant) setSecretDialogOpen(false); }, [secretDialogOpen, session.secretGrant, setSecretDialogOpen]);

  return (
    <>
      <AccountGroupDialog
        open={controller.accountGroupDialogOpen}
        locked={editorLocked}
        accounts={controller.displayedAccountCatalog.accounts}
        existingGroupIds={controller.groupDirectory.map((group) => group.groupId)}
        onOpenChange={controller.setAccountGroupDialogOpen}
        onSubmit={(value) => !editorLocked && controller.submitAccountGroup(value)}
      />
      <ProviderCatalogDialog
        open={providerCatalogDialogOpen}
        existingProviderIds={credentialProviderOptions.map((provider) => provider.id)}
        existingCredentialIds={explicitCredentialIds}
        locked={editorLocked}
        hasSecretAccess={hasSecretAccess}
        onOpenChange={setProviderCatalogDialogOpen}
        onRequestSecretAccess={() => setSecretDialogOpen(true)}
        onSubmit={applyProviderCatalogDraft}
        onDiscover={controller.discoverAccount}
      />
      {providerModelMappingTarget ? (
        <ProviderModelMappingDialog
          open
          providerId={providerModelMappingTarget.providerId}
          providerLabel={providerModelMappingTarget.providerLabel}
          modelOptions={providerModelMappingTarget.modelOptions}
          upstreamModelOptions={providerModelMappingTarget.upstreamModelOptions}
          initialEntries={providerModelMappingTarget.entries}
          locked={editorLocked}
          onOpenChange={(open) => {
            if (!open) {
              setProviderModelMappingProviderId(null);
            }
          }}
          saveError={controller.error}
          savePending={controller.autosavePending || (controller.draftDirty && controller.editorLocked)}
          onSubmit={submitProviderModelMapping}
        />
      ) : null}
      {credentialDialogState ? (
        <CredentialDialog
          open
          mode={credentialDialogState.mode}
          providerOptions={credentialProviderOptions}
          existingCredentialIds={explicitCredentialIds}
          initialValue={credentialDialogState.initialValue}
          locked={editorLocked}
          hasSecretAccess={hasSecretAccess}
          onOpenChange={(open) => {
            if (!open) {
              setCredentialDialogState(null);
            }
          }}
          onRequestSecretAccess={() => setSecretDialogOpen(true)}
          onSubmit={applyCredentialDialogValue}
          onDiscover={controller.discoverAccount}
          discoveryProvider={(id) => {
            const provider = controller.draftDocumentState.document?.providers.find((p) =>
              !!p && typeof p === "object" && (p as Record<string, unknown>).id === id) as Record<string, unknown> | undefined;
            return provider && supportsAccountDiscovery(provider) ? { baseUrl: String(provider.base_url ?? "") } : null;
          }}
          onRevealKey={revealCredentialKey}
          renderAuthentication={(providerId) => controller.routeConfig?.routeConfig.document.providers.some((provider) =>
            typeof provider === "object" && provider !== null && "id" in provider && "preset" in provider &&
            provider.id === providerId && provider.preset === "chatgpt-codex-oauth-official-api") ? (
              <ChatgptOAuthPanel client={controller.client} providerId={providerId}
                managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
                disabled={editorLocked || draftDirty} onRequestSecretAccess={() => setSecretDialogOpen(true)}
                onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />
            ) : null}
        />
      ) : null}
      <GeminiManualAddDialog
        geminiManualAddDialogState={geminiManualAddDialogState}
        closeGeminiManualAddDialog={closeGeminiManualAddDialog}
        requestGeminiManualAddCompletion={requestGeminiManualAddCompletion}
        t={t}
      />
      <PilotActionDialog
        accountPilotSections={controller.accountPilotSections}
        testPolicyDocument={controller.testPolicyDocument}
        applyTestPolicy={controller.applyTestPolicy}
        applyTestPlanChanges={controller.applyTestPlanChanges}
        loadProviderProbeResults={controller.loadProviderProbeResults}
        t={t}
        pilotActionDialog={pilotActionDialog}
        closePilotActionDialog={closePilotActionDialog}
        credentialProbeBusy={credentialProbeBusy}
        activePilotManagedAccount={activePilotManagedAccount}
        activePilotProbeResult={activePilotProbeResult}
        draftDirty={draftDirty}
        draftMatchesActiveRevision={draftMatchesActiveRevision}
        startPilotProbe={startPilotProbe}
        providerProbeBusy={providerProbeBusy}
        providerProbeError={providerProbeError}
        providerProbeResponse={providerProbeResponse}
        startProviderProbe={startProviderProbe}
        editorLocked={editorLocked}
        pilotScheduleEnabled={pilotScheduleEnabled}
        setPilotScheduleEnabled={setPilotScheduleEnabled}
        pilotScheduleIntervalMinutes={pilotScheduleIntervalMinutes}
        setPilotScheduleIntervalMinutes={setPilotScheduleIntervalMinutes}
        applyProviderProbeSchedule={applyProviderProbeSchedule}
        telemetryError={telemetryError}
        activePilotStatsView={activePilotStatsView}
        applyPilotProbeSchedule={applyPilotProbeSchedule}
      />
      {secretDialogOpen && !session.secretGrant ? <div className="nt-banner" role={session.error ? "alert" : "status"}>
        {session.error ?? t("正在恢复管理会话…", "Restoring management access…")}
        {session.error && session.managementToken ? <button type="button" className="nt-btn nt-btn--outline" disabled={session.busy}
          onClick={() => void session.confirmSecretAccess(session.managementToken!).catch(() => {})}>{t("重试", "Retry")}</button> : null}
      </div> : null}
    </>
  );
}
