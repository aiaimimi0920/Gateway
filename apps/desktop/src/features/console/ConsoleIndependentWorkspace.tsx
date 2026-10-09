import { AppearanceSettings } from "../settings/AppearanceSettings";
import { ManagementSecuritySettings } from "../settings/ManagementSecuritySettings";
import { OperationsWorkspace } from "./OperationsWorkspace";
import { AccessKeysWorkspace } from "./AccessKeysWorkspace";
import type { useOperationsData } from "./useOperationsData";
import type { useAccessData } from "./useAccessData";

type ConsoleIndependentWorkspaceProps = {
  activeWorkspace: "operations" | "access" | "settings";
  operations: ReturnType<typeof useOperationsData>;
  access: ReturnType<typeof useAccessData>;
  editorLocked: boolean;
  t: (zh: string, en: string) => string;
};

export function ConsoleIndependentWorkspace({
  activeWorkspace,
  operations,
  access,
  editorLocked,
  t,
}: ConsoleIndependentWorkspaceProps) {
  const settingsWorkspace = (
    <div className="nt-settings-page">
      <div className="nt-settings-accordion">
        <AppearanceSettings />
        <ManagementSecuritySettings />
      </div>
    </div>
  );
  const operationsWorkspace = (
    <OperationsWorkspace
      t={t}
      editorLocked={editorLocked}
      refreshing={operations.refreshing}
      onRefresh={operations.refresh}
      pressure={operations.panels.pressure}
      readiness={operations.panels.readiness}
      operatorSummary={operations.panels.operatorSummary}
      requests={operations.panels.requests}
      requestSummary={operations.panels.requestSummary}
      usageSummary={operations.panels.usageSummary}
      promptCache={operations.panels.promptCache}
      incidents={operations.panels.incidents}
      incidentSummary={operations.panels.incidentSummary}
      alertQueue={operations.panels.alertQueue}
      policies={operations.panels.policies}
      remediationQueue={operations.panels.remediationQueue}
      remediationRuns={operations.panels.remediationRuns}
      remediationEffectiveness={operations.panels.remediationEffectiveness}
      hotspots={operations.panels.hotspots}
      exports={operations.panels.exports}
      exportInventory={operations.panels.exportInventory}
      requestFilters={operations.requestFilters}
      onRequestFiltersChange={operations.setRequestFilters}
      onApplyRequestFilters={operations.applyRequestFilters}
      incidentBusyId={operations.incidentBusyId}
      onAcknowledgeIncident={operations.acknowledgeIncident}
      onResolveIncident={operations.resolveIncident}
    />
  );
  const accessWorkspace = (
    <AccessKeysWorkspace
      t={t}
      editorLocked={editorLocked}
      refreshing={access.refreshing}
      onRefresh={access.refresh}
      catalog={access.catalog}
      revealedSecret={access.revealedSecret}
      onDismissSecret={access.dismissSecret}
      keyDraft={access.keyDraft}
      onKeyDraftChange={access.setKeyDraft}
      onCreateKey={access.createKey}
      creatingKey={access.creatingKey}
      keyBusyId={access.keyBusyId}
      onRotateKey={access.rotateKey}
      onKeyLifecycle={access.keyLifecycle}
      onUpdateKey={access.updateKey}
      onCopyKey={access.copyKey}
      bundleDraft={access.bundleDraft}
      onBundleDraftChange={access.setBundleDraft}
      onCreateBundle={access.createBundle}
      creatingBundle={access.creatingBundle}
      affinityDraft={access.affinityDraft}
      onAffinityDraftChange={access.setAffinityDraft}
      onInspectAffinity={access.inspectAffinity}
      onResetAffinity={access.resetAffinity}
      affinity={access.affinity}
      affinityNotice={access.affinityNotice}
      rotationDraft={access.rotationDraft}
      onRotationDraftChange={access.setRotationDraft}
      onRotateApiAccess={access.rotateApiAccess}
      rotatingApiAccess={access.rotatingApiAccess}
      lastRotation={access.lastRotation}
      credentialDraft={access.credentialDraft}
      onCredentialDraftChange={access.setCredentialDraft}
      onIssueCredential={access.issueCredential}
      issuingCredential={access.issuingCredential}
      lastIssuedCredential={access.lastIssuedCredential}
      verifyDraft={access.verifyDraft}
      onVerifyDraftChange={access.setVerifyDraft}
      onVerifyCredential={access.verifyCredential}
      onRevokeCredential={access.revokeCredential}
      credentialBusy={access.credentialBusy}
      verification={access.verification}
      credentialNotice={access.credentialNotice}
    />
  );

  return activeWorkspace === "operations"
    ? operationsWorkspace
    : activeWorkspace === "access"
      ? accessWorkspace
      : settingsWorkspace;
}
