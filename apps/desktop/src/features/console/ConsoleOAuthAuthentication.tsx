import { useMemo } from "react";
import type { GatewayApiClient } from "../../api/client";
import { AccioOAuthPanel } from "./AccioOAuthPanel";
import type { AccioOAuthOperations } from "./useAccioOAuth";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";
import { DimOAuthPanel } from "./DimOAuthPanel";
import type { DimOAuthOperations } from "./useDimOAuth";
import { dimSite } from "./dimCatalog";
import { FlowmusicAccountImport } from "./FlowmusicAccountImport";
import { FreebuffLoginPanel } from "./FreebuffLoginPanel";
import type { FreebuffLoginOperations } from "./useFreebuffLogin";
import { GoogleWebAccountImport, googleWebAuthFamily } from "./GoogleWebAccountImport";
import { GrokOAuthPanel } from "./GrokOAuthPanel";
import type { GrokOAuthOperations } from "./grokOAuthOperations";
import { KiroOAuthPanel } from "./KiroOAuthPanel";
import type { KiroOAuthOperations } from "./useKiroOAuth";
import { isLobsterAiProvider } from "./lobsteraiCatalog";
import { LobsterAiOAuthPanel, type LobsterAiOAuthOperations } from "./LobsterAiOAuthPanel";
import { isLumaMcpProvider, savedLumaRegistration } from "./lumaMcp";
import { LumaOAuthPanel } from "./LumaOAuthPanel";
import type { LumaOAuthOperations } from "./useLumaOAuth";
import { isNousProvider } from "./nousCatalog";
import { NousOAuthPanel, type NousOAuthOperations } from "./NousOAuthPanel";
import { isQuickOAuthProvider } from "./quickCatalog";
import { QuickOAuthPanel, type QuickOAuthOperations } from "./QuickOAuthPanel";
import { isVertexOAuthProvider, vertexAuthorizationTarget } from "./vertexOAuth";
import { VertexOAuthPanel } from "./VertexOAuthPanel";
import type { VertexOAuthOperations } from "./useVertexOAuth";

type OAuthAuthDeps = {
  client: GatewayApiClient;
  managementToken: string | null;
  secretGrant: string | null;
  expectedRevision: string;
  disabled: boolean;
  webDisabled: boolean;
  hasSecretAccess: boolean;
  credentialProvider(id: string): Record<string, unknown> | undefined;
  confirmSecretAccess(): Promise<void>;
  onRequestSecretDialog(): void;
  onSaved(): void;
  onOpenGeminiManualAdd(family: NonNullable<ReturnType<typeof googleWebAuthFamily>>, id: string, label?: string, source?: string): void;
};

export function useConsoleOAuthAuthentication(deps: OAuthAuthDeps) {
  const grokOperations = useMemo<GrokOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const accioOperations = useMemo<AccioOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const dimOperations = useMemo<DimOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const lumaOperations = useMemo<LumaOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const nousOperations = useMemo<NousOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const lobsterOperations = useMemo<LobsterAiOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const quickOperations = useMemo<QuickOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const kiroOperations = useMemo<KiroOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const vertexOperations = useMemo<VertexOAuthOperations>(() => new Map(), [deps.client, deps.managementToken]);
  const freebuffOperations = useMemo<FreebuffLoginOperations>(() => new Map(), [deps.client, deps.managementToken]);

  const common = (providerId: string, credentialId?: string) => ({
    client: deps.client,
    providerId,
    credentialId,
    expectedRevision: deps.expectedRevision,
    managementToken: deps.managementToken,
    secretGrant: deps.secretGrant,
    disabled: deps.disabled,
    onRequestSecretAccess: deps.confirmSecretAccess,
    onSaved: deps.onSaved,
  });

  const renderLuma = (providerId: string, onActivityChange?: (active: boolean) => void, credentialId?: string) => (
    <LumaOAuthPanel
      {...common(providerId, credentialId)}
      operations={lumaOperations}
      onActivityChange={onActivityChange}
      registration={savedLumaRegistration(deps.credentialProvider(providerId), credentialId)}
    />
  );

  const renderManagedOAuth = (
    providerId: string,
    onActivityChange: (active: boolean) => void,
    credentialId?: string,
  ) => {
    const provider = deps.credentialProvider(providerId);
    if (isLobsterAiProvider(provider)) {
      return <LobsterAiOAuthPanel {...common(providerId, credentialId)} operations={lobsterOperations} onActivityChange={onActivityChange} />;
    }
    if (isQuickOAuthProvider(provider)) {
      return <QuickOAuthPanel {...common(providerId, credentialId)} operations={quickOperations} onActivityChange={onActivityChange} />;
    }
    if (isNousProvider(provider)) {
      return <NousOAuthPanel {...common(providerId, credentialId)} operations={nousOperations} onActivityChange={onActivityChange} />;
    }
    const site = dimSite(provider);
    if (site) {
      return <DimOAuthPanel site={site} {...common(providerId, credentialId)} operations={dimOperations} onActivityChange={onActivityChange} />;
    }
    if (provider?.preset === "freebuff") {
      return <FreebuffLoginPanel {...common(providerId, credentialId)} operations={freebuffOperations} onActivityChange={onActivityChange} />;
    }
    if (isVertexOAuthProvider(provider)) {
      return (
        <VertexOAuthPanel
          {...common(providerId, credentialId)}
          operations={vertexOperations}
          onActivityChange={onActivityChange}
          target={vertexAuthorizationTarget(provider, credentialId)}
        />
      );
    }
    if (provider?.preset === "kiro") {
      return <KiroOAuthPanel {...common(providerId, credentialId)} operations={kiroOperations} onActivityChange={onActivityChange} />;
    }
    if (provider?.preset === "accio") {
      return <AccioOAuthPanel {...common(providerId, credentialId)} operations={accioOperations} onActivityChange={onActivityChange} />;
    }
    if (provider?.preset === "grok-oauth") {
      return <GrokOAuthPanel {...common(providerId, credentialId)} operations={grokOperations} />;
    }
    return null;
  };

  const renderAuthentication = (providerId: string, onActivityChange: (active: boolean) => void) => {
    const provider = deps.credentialProvider(providerId);
    if (isLumaMcpProvider(provider)) return renderLuma(providerId, onActivityChange);
    const managed = renderManagedOAuth(providerId, onActivityChange);
    if (managed) return managed;
    if (provider?.preset === "producer") {
      return (
        <FlowmusicAccountImport
          providerId={providerId}
          disabled={deps.disabled}
          hasSecretAccess={deps.hasSecretAccess}
          onRequestSecretAccess={deps.onRequestSecretDialog}
          onStart={(family, id, label) => deps.onOpenGeminiManualAdd(family, id, label)}
          onImportJson={(family, id, label, source) => deps.onOpenGeminiManualAdd(family, id, label, source)}
        />
      );
    }
    if (googleWebAuthFamily(provider?.preset)) {
      return (
        <GoogleWebAccountImport
          preset={provider?.preset}
          providerId={providerId}
          disabled={deps.disabled}
          hasSecretAccess={deps.hasSecretAccess}
          onRequestSecretAccess={deps.onRequestSecretDialog}
          onImportJson={(family, id, label, source) => deps.onOpenGeminiManualAdd(family, id, label, source)}
          onStart={(family, id, label) => deps.onOpenGeminiManualAdd(family, id, label)}
        />
      );
    }
    if (["chatgpt-codex-oauth-official-api", "antigravity-oauth"].includes(String(provider?.preset))) {
      return (
        <ChatgptOAuthPanel
          client={deps.client}
          providerId={providerId}
          kind={provider?.preset === "antigravity-oauth" ? "antigravity" : "chatgpt"}
          managementToken={deps.managementToken}
          secretGrant={deps.secretGrant}
          disabled={deps.webDisabled}
          onRequestSecretAccess={deps.confirmSecretAccess}
          onSaved={deps.onSaved}
        />
      );
    }
    return null;
  };

  const reauthenticationOnly = (providerId: string) => {
    const provider = deps.credentialProvider(providerId);
    return (
      isLobsterAiProvider(provider) ||
      isQuickOAuthProvider(provider) ||
      isNousProvider(provider) ||
      Boolean(dimSite(provider)) ||
      provider?.preset === "kiro" ||
      isVertexOAuthProvider(provider)
    );
  };

  return {
    renderLuma,
    renderAuthentication,
    renderReauthentication: (providerId: string, credentialId: string, onActivityChange: (active: boolean) => void) =>
      renderManagedOAuth(providerId, onActivityChange, credentialId),
    reauthenticationOnly,
  };
}
