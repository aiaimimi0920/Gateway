import { useState } from "react";

import {
  type AccessSectionId,
  type AccessKeysWorkspaceProps,
} from "./accessKeysTypes";
import { AccessKeysSection } from "./AccessKeysSection";
import { AccessBundlesSection } from "./AccessBundlesSection";
import { AccessAffinitySection } from "./AccessAffinitySection";
import { AccessCredentialsSection } from "./AccessCredentialsSection";
import { AccessWorkspaceHeader } from "./AccessWorkspaceHeader";
import { useAccessKeysViewModel } from "./useAccessKeysViewModel";

export {
  ACCESS_DEFAULT_AFFINITY_DRAFT,
  ACCESS_DEFAULT_BUNDLE_DRAFT,
  ACCESS_DEFAULT_CREDENTIAL_DRAFT,
  ACCESS_DEFAULT_KEY_DRAFT,
  ACCESS_DEFAULT_ROTATION_DRAFT,
  ACCESS_DEFAULT_VERIFY_DRAFT,
  buildAccessBundleInput,
  buildAccessKeyInput,
  buildUserCredentialInput,
  parseScopeList,
} from "./accessKeysTypes";
export type {
  AccessAffinityDraft,
  AccessBundleDraft,
  AccessKeyDraft,
  AccessPanelState,
  AccessRevealedSecret,
  AccessSectionId,
  AccessKeysWorkspaceProps,
  ApiAccessRotationDraft,
  TranslateFn,
  UserCredentialDraft,
  UserCredentialVerifyDraft,
} from "./accessKeysTypes";

export function AccessKeysWorkspace({
  t,
  notice,
  editorLocked,
  refreshing,
  onRefresh,
  catalog,
  revealedSecret,
  onDismissSecret,
  keyDraft,
  onKeyDraftChange,
  onCreateKey,
  creatingKey,
  keyBusyId,
  onRotateKey,
  onRevokeKey,
  bundleDraft,
  onBundleDraftChange,
  onCreateBundle,
  creatingBundle,
  affinityDraft,
  onAffinityDraftChange,
  onInspectAffinity,
  onResetAffinity,
  affinity,
  affinityNotice,
  rotationDraft,
  onRotationDraftChange,
  onRotateApiAccess,
  rotatingApiAccess,
  lastRotation,
  credentialDraft,
  onCredentialDraftChange,
  onIssueCredential,
  issuingCredential,
  lastIssuedCredential,
  verifyDraft,
  onVerifyDraftChange,
  onVerifyCredential,
  onRevokeCredential,
  credentialBusy,
  verification,
  credentialNotice,
}: AccessKeysWorkspaceProps) {
  const [openSections, setOpenSections] = useState<AccessSectionId[]>(["keys"]);
  const toggleSection = (id: AccessSectionId) => {
    setOpenSections((current) =>
      current.includes(id) ? current.filter((entry) => entry !== id) : [...current, id],
    );
  };

  const {
    affinityFormIncomplete,
    activeKeyCount,
    bundleFormIncomplete,
    bundles,
    bundlesByKey,
    catalogEmpty,
    itemCountByBundle,
    keyFormIncomplete,
    sortedKeys,
  } = useAccessKeysViewModel(catalog, keyDraft, bundleDraft, affinityDraft);

  return (
    <div className="nt-settings-page nt-console-page nt-console-table-page">
      <AccessWorkspaceHeader
        notice={notice}
        onDismissSecret={onDismissSecret}
        onRefresh={onRefresh}
        refreshing={refreshing}
        revealedSecret={revealedSecret}
        t={t}
      />

      <div className="nt-settings-accordion">
        <AccessKeysSection
          activeKeyCount={activeKeyCount}
          bundles={bundles}
          bundlesByKey={bundlesByKey}
          catalog={catalog}
          catalogEmpty={catalogEmpty}
          creatingKey={creatingKey}
          editorLocked={editorLocked}
          keyBusyId={keyBusyId}
          keyDraft={keyDraft}
          keyFormIncomplete={keyFormIncomplete}
          onCreateKey={onCreateKey}
          onKeyDraftChange={onKeyDraftChange}
          onRevokeKey={onRevokeKey}
          onRotateKey={onRotateKey}
          onToggle={toggleSection}
          open={openSections.includes("keys")}
          sortedKeys={sortedKeys}
          t={t}
        />

        <AccessBundlesSection
          bundleDraft={bundleDraft}
          bundleFormIncomplete={bundleFormIncomplete}
          bundles={bundles}
          catalog={catalog}
          catalogEmpty={catalogEmpty}
          creatingBundle={creatingBundle}
          editorLocked={editorLocked}
          itemCountByBundle={itemCountByBundle}
          onBundleDraftChange={onBundleDraftChange}
          onCreateBundle={onCreateBundle}
          onToggle={toggleSection}
          open={openSections.includes("bundles")}
          t={t}
        />

        <AccessAffinitySection
          affinity={affinity}
          affinityDraft={affinityDraft}
          affinityFormIncomplete={affinityFormIncomplete}
          affinityNotice={affinityNotice}
          editorLocked={editorLocked}
          lastRotation={lastRotation}
          onAffinityDraftChange={onAffinityDraftChange}
          onInspectAffinity={onInspectAffinity}
          onResetAffinity={onResetAffinity}
          onRotateApiAccess={onRotateApiAccess}
          onRotationDraftChange={onRotationDraftChange}
          onToggle={toggleSection}
          open={openSections.includes("affinity")}
          rotationDraft={rotationDraft}
          rotatingApiAccess={rotatingApiAccess}
          t={t}
        />

        <AccessCredentialsSection
          credentialBusy={credentialBusy}
          credentialDraft={credentialDraft}
          credentialNotice={credentialNotice}
          editorLocked={editorLocked}
          issuingCredential={issuingCredential}
          lastIssuedCredential={lastIssuedCredential}
          onCredentialDraftChange={onCredentialDraftChange}
          onIssueCredential={onIssueCredential}
          onRevokeCredential={onRevokeCredential}
          onToggle={() => toggleSection("credentials")}
          onVerifyCredential={onVerifyCredential}
          onVerifyDraftChange={onVerifyDraftChange}
          open={openSections.includes("credentials")}
          t={t}
          verification={verification}
          verifyDraft={verifyDraft}
        />
      </div>
    </div>
  );
}
