import { SecretConfirmDialog } from "../auth/SecretConfirmDialog";
import { CredentialDialog } from "./CredentialDialog";
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
    | "activePilotProbeResult"
    | "activePilotStatsView"
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
    | "draftMatchesActiveRevision"
    | "editorLocked"
    | "explicitCredentialIds"
    | "geminiManualAddDialogState"
    | "hasSecretAccess"
    | "openAddCredentialDialog"
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
    openAddCredentialDialog,
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

  return (
    <>
      <ProviderCatalogDialog
        open={providerCatalogDialogOpen}
        existingProviderIds={credentialProviderOptions.map((provider) => provider.id)}
        existingCredentialIds={explicitCredentialIds}
        locked={editorLocked}
        hasSecretAccess={hasSecretAccess}
        onOpenChange={setProviderCatalogDialogOpen}
        onRequestSecretAccess={() => setSecretDialogOpen(true)}
        onAddAccount={openAddCredentialDialog}
        onSubmit={applyProviderCatalogDraft}
      />
      {providerModelMappingTarget ? (
        <ProviderModelMappingDialog
          open
          providerId={providerModelMappingTarget.providerId}
          providerLabel={providerModelMappingTarget.providerLabel}
          modelOptions={providerModelMappingTarget.modelOptions}
          initialEntries={providerModelMappingTarget.entries}
          locked={editorLocked}
          onOpenChange={(open) => {
            if (!open) {
              setProviderModelMappingProviderId(null);
            }
          }}
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
      <SecretConfirmDialog
        open={secretDialogOpen}
        busy={session.busy}
        error={session.error}
        onOpenChange={setSecretDialogOpen}
        onConfirm={session.confirmSecretAccess}
      />
    </>
  );
}
