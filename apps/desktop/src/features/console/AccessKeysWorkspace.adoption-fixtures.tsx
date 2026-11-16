import { useEffect } from "react";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleAccessCatalog, ConsoleAccessStickyAffinity, ConsoleIssuedUserCredential } from "../../api/contracts";
import { AccessKeysWorkspace } from "./AccessKeysWorkspace";
import { useAccessData, type UseAccessDataResult } from "./useAccessData";
import type { TranslateFn } from "./accessKeysTypes";

export const advancedCatalog: ConsoleAccessCatalog = {
  storageMode: "server", accountGroups: [], accessKeys: [], balances: [],
  bundles: [{ id: "bundle-a", projectId: null, slug: "bundle-a", displayName: "Bundle A", billingMode: "metered", status: "active", description: null, metadata: null, createdAt: "2026-10-01T00:00:00Z", updatedAt: "2026-10-01T00:00:00Z" }],
  bundleItems: ["access-a", "access-b"].map(platformAccessId => ({ bundleId: "bundle-a", platformAccessId, createdAt: "2026-10-01T00:00:00Z" })),
  providerCapabilities: [], platformAccessRows: [], keyBundleBindings: [], aggregateMemberships: [],
};
export const advancedAffinity: ConsoleAccessStickyAffinity = {
  scope: "scope-a", model: "model-a", requestingAccessKeyId: "key-a", sourceAccessKeyId: "source-a",
  platformAccessId: "platform-a", providerAccountId: "provider-a", realCredentialRef: null, expiresAt: null,
};
export const advancedCredential: ConsoleIssuedUserCredential = {
  id: "credential-a", credentialKey: "synthetic-credential-secret", expiresAt: "2026-12-01T00:00:00Z",
  scope: ["chat.completions", "responses"], userId: "user-a", projectId: "project-a", tenantId: "tenant-a", credentialType: "session",
};

/** Uses the application's real controller; only the receiving API and session inputs are fixtures. */
export function AccessWorkspaceHarness({ api, t, locked = false, active = true, token = "synthetic-admin", onAccess }: {
  api: ConsoleApi; t: TranslateFn; locked?: boolean; active?: boolean; token?: string | null;
  onAccess?: (access: UseAccessDataResult) => void;
}) {
  const access = useAccessData({ api, active, managementToken: token, t });
  useEffect(() => { onAccess?.(access); }, [access, onAccess]);
  return <AccessKeysWorkspace
    t={t} editorLocked={locked} refreshing={access.refreshing} onRefresh={access.refresh} catalog={access.catalog}
    revealedSecret={access.revealedSecret} onDismissSecret={access.dismissSecret}
    keyDraft={access.keyDraft} onKeyDraftChange={access.setKeyDraft} onCreateKey={access.createKey}
    creatingKey={access.creatingKey} keyBusyId={access.keyBusyId} onRotateKey={access.rotateKey}
    onKeyLifecycle={access.keyLifecycle} onUpdateKey={access.updateKey} onCopyKey={access.copyKey}
    bundleDraft={access.bundleDraft} onBundleDraftChange={access.setBundleDraft} onCreateBundle={access.createBundle} creatingBundle={access.creatingBundle}
    affinityDraft={access.affinityDraft} onAffinityDraftChange={access.setAffinityDraft}
    onInspectAffinity={access.inspectAffinity} onResetAffinity={access.resetAffinity} affinity={access.affinity} affinityNotice={access.affinityNotice}
    rotationDraft={access.rotationDraft} onRotationDraftChange={access.setRotationDraft} onRotateApiAccess={access.rotateApiAccess}
    rotatingApiAccess={access.rotatingApiAccess} lastRotation={access.lastRotation}
    credentialDraft={access.credentialDraft} onCredentialDraftChange={access.setCredentialDraft} onIssueCredential={access.issueCredential}
    issuingCredential={access.issuingCredential} lastIssuedCredential={access.lastIssuedCredential}
    verifyDraft={access.verifyDraft} onVerifyDraftChange={access.setVerifyDraft}
    onVerifyCredential={access.verifyCredential} onRevokeCredential={access.revokeCredential}
    credentialBusy={access.credentialBusy} verification={access.verification} credentialNotice={access.credentialNotice}
  />;
}
