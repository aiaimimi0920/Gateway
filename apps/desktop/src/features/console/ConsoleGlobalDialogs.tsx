import { useEffect, useMemo } from "react";
import { Button, Dialog, Notice } from "@neuro/ui";
import { LumaOAuthPanel } from "./LumaOAuthPanel";
import { isLumaMcpProvider, savedLumaRegistration } from "./lumaMcp";
import type { LumaOAuthOperations } from "./useLumaOAuth";
import type { GrokOAuthOperations } from "./grokOAuthOperations";
import { isCodexProvider } from "./siteHealth";
import { supportsAccountDiscovery } from "./accountDiscovery";
import { catalogProviderKind } from "./officialModelCatalog";
import { AccountGroupDialog } from "./AccountGroupDialog";
import { CredentialDialog } from "./CredentialDialog";
import { sessionProductForPreset } from "./providerSessionProducts";
import { isBedrockProvider } from "./bedrockMaterial";
import { isQwenProvider } from "./qwenEndpoint";
import { modelIdPolicy } from "./providerModelId";
import { useCredentialKeyReveal } from "./useCredentialKeyReveal";
import { useChatgptWebBrowserCapture } from "./useChatgptWebBrowserCapture";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";
import { GrokOAuthPanel } from "./GrokOAuthPanel";
import { AccioOAuthPanel } from "./AccioOAuthPanel";
import type { AccioOAuthOperations } from "./useAccioOAuth";
import { DimOAuthPanel } from "./DimOAuthPanel";
import { DimCatalogPanel } from "./DimCatalogPanel";
import { savedDimBinding, stageDimCatalog } from "./dimModelCatalog";
import type { DimOAuthOperations } from "./useDimOAuth";
import { dimSite } from "./dimCatalog";
import { NousOAuthPanel, type NousOAuthOperations } from "./NousOAuthPanel";
import { NousCatalogPanel } from "./NousCatalogPanel";
import { savedNousBinding, stageNousCatalog } from "./nousModelCatalog";
import { isNousProvider } from "./nousCatalog";
import { isLobsterAiProvider } from "./lobsteraiCatalog";
import { LobsterAiCatalogPanel } from "./LobsterAiCatalogPanel";
import { savedLobsterAiBinding, stageLobsterAiCatalog } from "./lobsteraiModelCatalog";
import { LobsterAiOAuthPanel, type LobsterAiOAuthOperations } from "./LobsterAiOAuthPanel";
import { KiroOAuthPanel } from "./KiroOAuthPanel";
import type { KiroOAuthOperations } from "./useKiroOAuth";
import { VertexOAuthPanel } from "./VertexOAuthPanel";
import type { VertexOAuthOperations } from "./useVertexOAuth";
import { FreebuffLoginPanel } from "./FreebuffLoginPanel";
import { FreebuffCatalogPanel } from "./FreebuffCatalogPanel";
import { savedFreebuffBusiness, savedFreebuffModelKeys, stageFreebuffCatalog } from "./freebuffBusiness";
import type { FreebuffLoginOperations } from "./useFreebuffLogin";
import { isVertexOAuthProvider, vertexAuthorizationTarget } from "./vertexOAuth";
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
  const grokOperations = useMemo<GrokOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const accioOperations = useMemo<AccioOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const dimOperations = useMemo<DimOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const lumaOperations = useMemo<LumaOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const nousOperations = useMemo<NousOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const lobsterOperations = useMemo<LobsterAiOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const kiroOperations = useMemo<KiroOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const vertexOperations = useMemo<VertexOAuthOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
  const freebuffOperations = useMemo<FreebuffLoginOperations>(() => new Map(), [controller.client, controller.session.managementToken]);
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
  const lumaAuthentication = (providerId: string, onActivityChange?: (active: boolean) => void, credentialId?: string) => <LumaOAuthPanel
    client={controller.client} operations={lumaOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    registration={savedLumaRegistration(credentialProvider(providerId), credentialId)} expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;
  const nousAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => <NousOAuthPanel
    client={controller.client} operations={nousOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;
  const lobsterAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => <LobsterAiOAuthPanel
    client={controller.client} operations={lobsterOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;
  const dimAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => {
    const site = dimSite(credentialProvider(providerId));
    return site ? <DimOAuthPanel site={site} client={controller.client} operations={dimOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
      expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
      managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
      disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
        if (!session.managementToken) throw new Error("Management session unavailable");
        await session.confirmSecretAccess(session.managementToken);
      }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} /> : null;
  };
  const accioAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => <AccioOAuthPanel
    client={controller.client} operations={accioOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;
  const kiroAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => <KiroOAuthPanel
    client={controller.client} operations={kiroOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;
  const freebuffAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => <FreebuffLoginPanel
    client={controller.client} operations={freebuffOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;
  const vertexAuthentication = (providerId: string, onActivityChange: (active: boolean) => void, credentialId?: string) => <VertexOAuthPanel
    client={controller.client} operations={vertexOperations} providerId={providerId} credentialId={credentialId} onActivityChange={onActivityChange}
    target={vertexAuthorizationTarget(credentialProvider(providerId), credentialId)} expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
    managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
    disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
      if (!session.managementToken) throw new Error("Management session unavailable");
      await session.confirmSecretAccess(session.managementToken);
    }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />;

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
          modelPolicy={modelIdPolicy(credentialProvider(providerModelMappingTarget.providerId))}
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
      {credentialDialogState && isLumaMcpProvider(credentialProvider(credentialDialogState.initialValue?.providerId ?? "")) ? (
        <Dialog open title={credentialDialogState.mode === "edit" ? t("Luma MCP · 重新授权", "Luma MCP · Reauthorize") : t("Luma MCP · 新增账号", "Luma MCP · Add account")}
          onOpenChange={(open) => { if (!open) setCredentialDialogState(null); }} closeLabel={t("关闭", "Close")}>
          {lumaAuthentication(credentialDialogState.initialValue!.providerId, undefined,
            credentialDialogState.mode === "edit" ? credentialDialogState.initialValue!.credentialId : undefined)}
          <div className="n-dialog-actions"><Button onClick={() => setCredentialDialogState(null)}>{t("关闭", "Close")}</Button></div>
        </Dialog>
      ) : credentialDialogState ? (
        <CredentialDialog
          open
          mode={credentialDialogState.mode}
          providerOptions={credentialProviderOptions}
          supportsBasispoints={(id) => isCodexProvider(controller.draftDocumentState.document?.providers.find((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === id))}
          supportsWebSession={(id) => controller.draftDocumentState.document?.providers.some((p) => !!p && typeof p === "object" && (p as Record<string, unknown>).id === id && (p as Record<string, unknown>).preset === "chatgpt-web-reverse") ?? false}
          sessionProduct={(id) => sessionProductForPreset(credentialProvider(id)?.preset)}
          bedrockProvider={(id) => isBedrockProvider(credentialProvider(id)) ? credentialProvider(id) : undefined}
          qwenProvider={(id) => isQwenProvider(credentialProvider(id)) ? credentialProvider(id) : undefined}
          vertexExpressProvider={(id) => credentialProvider(id)?.preset === "vertex-express-api" ? credentialProvider(id) : undefined}
          vertexOAuthProvider={(id) => isVertexOAuthProvider(credentialProvider(id)) ? credentialProvider(id) : undefined}
          sapProvider={(id) => credentialProvider(id)?.preset === "sap-aicore-chat" ? credentialProvider(id) : undefined}
          lumaAgentsProvider={(id) => credentialProvider(id)?.preset === "lumalabs-api" ? credentialProvider(id) : undefined}
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
          modelPolicy={(id) => isNousProvider(credentialProvider(id)) ? modelIdPolicy(credentialProvider(id)) : undefined}
          onCaptureWebBrowser={captureWebBrowser}
          reauthenticationOnly={(id) => isLobsterAiProvider(credentialProvider(id)) || isNousProvider(credentialProvider(id)) || !!dimSite(credentialProvider(id)) || credentialProvider(id)?.preset === "kiro" || isVertexOAuthProvider(credentialProvider(id))}
          manualAuthentication={(id) => credentialProvider(id)?.preset === "sap-aicore-chat" ? {
            oauthLabel: "", manualLabel: "", secretLabel: "Service Key JSON", note: "",
          } : credentialProvider(id)?.preset === "freebuff" ? {
            oauthLabel: t("浏览器批准登录", "Browser approval"), manualLabel: t("现有会话材料", "Existing session material"), secretLabel: "authToken", note: "",
          } : credentialProvider(id)?.preset === "accio" ? {
            oauthLabel: t("Accio 登录", "Accio sign-in"), manualLabel: t("旧手工录入", "Legacy manual input"), secretLabel: "Access Token",
            note: t("仅保留旧手工录入兼容；此字段是 Access Token，不是原生 API Key。不会自动补齐设备、刷新或 SG 材料，亦不代表业务可用。", "Legacy manual input only: this field is an access token, not a native API key. Device, refresh and SG materials are not filled automatically; usability is not implied."),
          } : undefined}
          renderReauthentication={(providerId, credentialId, onActivityChange) => isLobsterAiProvider(credentialProvider(providerId)) ? lobsterAuthentication(providerId, onActivityChange, credentialId) : isNousProvider(credentialProvider(providerId)) ? nousAuthentication(providerId, onActivityChange, credentialId) : dimSite(credentialProvider(providerId)) ? dimAuthentication(providerId, onActivityChange, credentialId) : credentialProvider(providerId)?.preset === "freebuff" ? freebuffAuthentication(providerId, onActivityChange, credentialId) : isVertexOAuthProvider(credentialProvider(providerId)) ? vertexAuthentication(providerId, onActivityChange, credentialId) : credentialProvider(providerId)?.preset === "kiro" ? kiroAuthentication(providerId, onActivityChange, credentialId) : credentialProvider(providerId)?.preset === "accio" ? accioAuthentication(providerId, onActivityChange, credentialId) : credentialProvider(providerId)?.preset === "grok-oauth" ? (
            <GrokOAuthPanel client={controller.client} operations={grokOperations} providerId={providerId} credentialId={credentialId}
              expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
              managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
              disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
                if (!session.managementToken) throw new Error("Management session unavailable");
                await session.confirmSecretAccess(session.managementToken);
              }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />
          ) : null}
          renderStoredAccountTools={(providerId, credentialId, disabled, apply) => isLobsterAiProvider(credentialProvider(providerId)) ? (
            <LobsterAiCatalogPanel key={`${providerId}:${credentialId}`} client={controller.client}
              binding={savedLobsterAiBinding(credentialProvider(providerId), credentialId)} expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
              managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
              disabled={disabled || draftDirty || !draftMatchesActiveRevision || controller.autosavePending}
              onRequestSecretAccess={() => setSecretDialogOpen(true)} onApply={(catalog) => {
                const binding = savedLobsterAiBinding(credentialProvider(providerId), credentialId);
                return !!binding && apply((value) => stageLobsterAiCatalog(value, catalog, binding));
              }} />
          ) : isNousProvider(credentialProvider(providerId)) ? (
            <NousCatalogPanel key={`${providerId}:${credentialId}`} client={controller.client}
              binding={savedNousBinding(credentialProvider(providerId), credentialId)} expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
              managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
              disabled={disabled || draftDirty || !draftMatchesActiveRevision || controller.autosavePending}
              onRequestSecretAccess={() => setSecretDialogOpen(true)} onApply={(catalog) => {
                const binding = savedNousBinding(credentialProvider(providerId), credentialId);
                return !!binding && apply((value) => stageNousCatalog(value, catalog, binding));
              }} />
          ) : dimSite(credentialProvider(providerId)) ? (
            <DimCatalogPanel key={`${providerId}:${credentialId}`} client={controller.client}
              binding={savedDimBinding(credentialProvider(providerId), credentialId)} expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
              managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
              disabled={disabled || draftDirty || !draftMatchesActiveRevision || controller.autosavePending}
              onRequestSecretAccess={() => setSecretDialogOpen(true)} onApply={(catalog) => {
                const binding = savedDimBinding(credentialProvider(providerId), credentialId);
                return !!binding && apply((value) => stageDimCatalog(value, catalog, binding));
              }} />
          ) : credentialProvider(providerId)?.preset === "freebuff" ? (
            <FreebuffCatalogPanel key={`${providerId}:${credentialId}`} client={controller.client} providerId={providerId} credentialId={credentialId}
              expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""} managementToken={session.managementToken}
              secretGrant={session.secretGrant?.grant ?? null} disabled={disabled || draftDirty || !draftMatchesActiveRevision || controller.autosavePending}
              savedBusiness={savedFreebuffBusiness(credentialProvider(providerId), credentialId)}
              savedModelKeys={savedFreebuffModelKeys(credentialProvider(providerId), credentialId)}
              onApply={(catalog, keys) => apply((value) => stageFreebuffCatalog(value, catalog, keys))}
              onRequestSecretAccess={() => setSecretDialogOpen(true)} />
          ) : null}
          renderAuthentication={(providerId, onActivityChange) => isLumaMcpProvider(credentialProvider(providerId)) ? lumaAuthentication(providerId, onActivityChange) : isLobsterAiProvider(credentialProvider(providerId)) ? lobsterAuthentication(providerId, onActivityChange) : isNousProvider(credentialProvider(providerId)) ? nousAuthentication(providerId, onActivityChange) : dimSite(credentialProvider(providerId)) ? dimAuthentication(providerId, onActivityChange) : credentialProvider(providerId)?.preset === "freebuff" ? freebuffAuthentication(providerId, onActivityChange) : isVertexOAuthProvider(credentialProvider(providerId)) ? vertexAuthentication(providerId, onActivityChange) : credentialProvider(providerId)?.preset === "kiro" ? kiroAuthentication(providerId, onActivityChange) : credentialProvider(providerId)?.preset === "accio" ? accioAuthentication(providerId, onActivityChange) : credentialProvider(providerId)?.preset === "grok-oauth" ? (
            <GrokOAuthPanel client={controller.client} operations={grokOperations} providerId={providerId}
              expectedRevision={controller.routeConfig?.routeConfig.revision.id ?? ""}
              managementToken={session.managementToken} secretGrant={session.secretGrant?.grant ?? null}
              disabled={editorLocked || draftDirty || !draftMatchesActiveRevision} onRequestSecretAccess={async () => {
                if (!session.managementToken) throw new Error("Management session unavailable");
                await session.confirmSecretAccess(session.managementToken);
              }} onSaved={() => { setCredentialDialogState(null); void controller.refresh(); }} />
          ) : credentialProvider(providerId)?.preset === "producer" ? (
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
        activeRevision={controller.routeConfig?.routeConfig.revision.id ?? null}
        authorizationContext={session.secretGrant?.grant ?? null}
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
      {secretDialogOpen && !session.secretGrant ? <Notice tone={session.error ? "danger" : "info"}>
        {session.error ?? t("正在恢复管理会话…", "Restoring management access…")}
        {session.error && session.managementToken ? <Button compact disabled={session.busy}
          onClick={() => void session.confirmSecretAccess(session.managementToken!).catch(() => { })}>{t("重试", "Retry")}</Button> : null}
      </Notice> : null}
    </>
  );
}
