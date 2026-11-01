import { useEffect } from "react";
import { isCodexProvider } from "./siteHealth";
import { supportsAccountDiscovery } from "./accountDiscovery";
import { catalogProviderKind } from "./officialModelCatalog";
import { AccountGroupDialog } from "./AccountGroupDialog";
import { CredentialDialog } from "./CredentialDialog";
import { useCredentialKeyReveal } from "./useCredentialKeyReveal";
import { useChatgptWebBrowserCapture } from "./useChatgptWebBrowserCapture";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";
import { GoogleWebAccountImport, googleWebAuthFamily } from "./GoogleWebAccountImport";
import { FlowmusicAccountImport } from "./FlowmusicAccountImport";
import { GeminiManualAddDialog } from "./GeminiManualAddDialog";
import { PilotActionDialog } from "./PilotActionDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { ProviderModelMappingDialog } from "./ProviderModelMappingDialog";
import { CredentialPruneDialog } from "./CredentialPruneDialog";
import type { useConsoleController } from "./useConsoleController";

type ConsoleGlobalDialogsProps = {
  controller: Pick<
    ReturnType<typeof useConsoleController>,
    | "activePilotManagedAccount"
    | "credentialPruneReview"
    | "closeCredentialPruneReview"
    | "applyCredentialPruneReview"
    | "discoverAccount"
    | "discoverChatgptModels"
    | "discoverProviderModels"
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
    | "openGeminiManualAddDialog"
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
    | "providerCatalogDrivers"
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
  const captureWebBrowser = useChatgptWebBrowserCapture(controller.client, session.managementToken,
    session.secretGrant?.grant ?? null, controller.routeConfig?.routeConfig.revision.id, editorLocked || draftDirty);
  const credentialProvider = (id: string) => controller.draftDocumentState.document?.providers.find((p) =>
    !!p && typeof p === "object" && (p as Record<string, unknown>).id === id) as Record<string, unknown> | undefined;

  useEffect(() => { if (secretDialogOpen && session.secretGrant) setSecretDialogOpen(false); }, [secretDialogOpen, session.secretGrant, setSecretDialogOpen]);

  return (
    <>
      <CredentialPruneDialog review={controller.credentialPruneReview} locked={editorLocked || draftDirty}
        onClose={controller.closeCredentialPruneReview} onApply={controller.applyCredentialPruneReview} />
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
        drivers={controller.providerCatalogDrivers}
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
          supportsBasispoints={(id) => isCodexProvider(controller.draftDocumentState.document?.providers.find((p) => !!p && typeof p === "object" && (p as Record<string,unknown>).id === id))}
          supportsWebSession={(id) => controller.draftDocumentState.document?.providers.some((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === id && (p as Record<string, unknown>).preset === "chatgpt-web-reverse") ?? false}
          managedWebProvider={(id) => googleWebAuthFamily(credentialProvider(id)?.preset) !== null || credentialProvider(id)?.preset === "producer"}
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
          onDiscoverChatgptModels={controller.discoverChatgptModels}
          onDiscoverProviderModels={controller.discoverProviderModels}
          catalogProviderKind={(id) => {
            const provider = controller.draftDocumentState.document?.providers.find((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === id);
            return provider && typeof provider === "object" ? catalogProviderKind(provider as Record<string, unknown>) : undefined;
          }}
          discoveryProvider={(id) => {
            const provider = controller.draftDocumentState.document?.providers.find((p) =>
              !!p && typeof p === "object" && (p as Record<string, unknown>).id === id) as Record<string, unknown> | undefined;
            return provider && !catalogProviderKind(provider) && supportsAccountDiscovery(provider) ? { baseUrl: String(provider.base_url ?? "") } : null;
          }}
          onRevealKey={revealCredentialKey}
          onCaptureWebBrowser={captureWebBrowser}
          renderAuthentication={(providerId) => credentialProvider(providerId)?.preset === "producer" ? (
            <FlowmusicAccountImport providerId={providerId} disabled={editorLocked || draftDirty || !draftMatchesActiveRevision}
              hasSecretAccess={hasSecretAccess} onRequestSecretAccess={() => setSecretDialogOpen(true)}
              onStart={(family, id, label) => { setCredentialDialogState(null); void controller.openGeminiManualAddDialog(family, id, label); }}
              onImportJson={(family, id, label, source) => { setCredentialDialogState(null); void controller.openGeminiManualAddDialog(family, id, label, source); }} />
          ) : googleWebAuthFamily(credentialProvider(providerId)?.preset) ? (
            <GoogleWebAccountImport preset={credentialProvider(providerId)?.preset} providerId={providerId}
              disabled={editorLocked || draftDirty || !draftMatchesActiveRevision}
              hasSecretAccess={hasSecretAccess} onRequestSecretAccess={() => setSecretDialogOpen(true)}
              onImportJson={(family, id, label, source) => {
                setCredentialDialogState(null);
                void controller.openGeminiManualAddDialog(family, id, label, source);
              }}
              onStart={(family, id, label) => {
                setCredentialDialogState(null);
                void controller.openGeminiManualAddDialog(family, id, label);
              }} />
          ) : ["chatgpt-codex-oauth-official-api", "antigravity-oauth"].includes(String(credentialProvider(providerId)?.preset)) ? (
              <ChatgptOAuthPanel client={controller.client} providerId={providerId}
                kind={credentialProvider(providerId)?.preset === "antigravity-oauth" ? "antigravity" : "chatgpt"}
                managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
                disabled={editorLocked || draftDirty} onRequestSecretAccess={async () => {
                  if (!session.managementToken) throw new Error(t("管理会话不可用，请重新连接。", "Management session unavailable; reconnect."));
                  await session.confirmSecretAccess(session.managementToken);
                }}
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
