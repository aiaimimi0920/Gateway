import type { ReactNode } from "react";
import { keyQuotaInput } from "./accessKeyQuota";

import type {
  ConsoleAccessBundleInput,
  ConsoleAccessCatalog,
  ConsoleAccessKeyInput,
  ConsoleAccessKey,
  ConsoleAccessStickyAffinity,
  ConsoleApiAccessRotation,
  ConsoleIssuedUserCredential,
  ConsoleUserCredentialCacheEntry,
  ConsoleUserCredentialIssueInput,
} from "../../api/contracts";

export type TranslateFn = (zh: string, en: string) => string;

/** Sections open one at a time by default, mirroring the operations page. */
export type AccessSectionId = "keys" | "bundles" | "affinity" | "credentials";

export type AccessPanelState<T> = {
  data: T | null;
  error: string | null;
  loading: boolean;
};

export type AccessKeyDraft = {
  ownerType: string;
  ownerId: string;
  resolvedProjectId: string;
  resolvedTenantId: string;
  keyKind: string;
  publicKeyPrefix: string;
  displayName: string;
  expiresAt: string;
  bundleIds: string[];
  accountGroupIds: string[];
  quotaMode:
    | "unlimited"
    | "message_prepaid"
    | "token_prepaid"
    | "cash_prepaid"
    | "unchanged";
  quotaLimit: string;
};

export type AccessBundleDraft = {
  projectId: string;
  slug: string;
  displayName: string;
  billingMode: string;
  status: string;
  description: string;
};

export type AccessAffinityDraft = {
  accessKeyId: string;
  model: string;
  explicitSessionKey: string;
};

export type ApiAccessRotationDraft = {
  projectId: string;
  name: string;
  actorUserId: string;
};

export type UserCredentialDraft = {
  userId: string;
  projectId: string;
  credentialType: string;
  durationDays: string;
  scope: string;
};

export type UserCredentialVerifyDraft = {
  credentialKey: string;
  scope: string;
};

/** A one-shot secret held only until the operator dismisses it. */
export type AccessRevealedSecret = {
  kind: "access-key" | "api-access" | "user-credential";
  label: string;
  secret: string;
  hint?: string;
};

export const ACCESS_DEFAULT_KEY_DRAFT: AccessKeyDraft = {
  ownerType: "user",
  ownerId: "",
  resolvedProjectId: "",
  resolvedTenantId: "",
  keyKind: "user",
  publicKeyPrefix: "sk-gw",
  displayName: "",
  expiresAt: "",
  bundleIds: [],
  accountGroupIds: [],
  quotaMode: "unlimited",
  quotaLimit: "",
};

export const ACCESS_DEFAULT_BUNDLE_DRAFT: AccessBundleDraft = {
  projectId: "",
  slug: "",
  displayName: "",
  billingMode: "metered",
  status: "active",
  description: "",
};

export const ACCESS_DEFAULT_AFFINITY_DRAFT: AccessAffinityDraft = {
  accessKeyId: "",
  model: "",
  explicitSessionKey: "",
};

export const ACCESS_DEFAULT_ROTATION_DRAFT: ApiAccessRotationDraft = {
  projectId: "",
  name: "",
  actorUserId: "",
};

export const ACCESS_DEFAULT_CREDENTIAL_DRAFT: UserCredentialDraft = {
  userId: "",
  projectId: "",
  credentialType: "session",
  durationDays: "30",
  scope: "chat.completions",
};

export const ACCESS_DEFAULT_VERIFY_DRAFT: UserCredentialVerifyDraft = {
  credentialKey: "",
  scope: "",
};

/** Turns a comma/whitespace separated scope field into API scope entries. */
export function parseScopeList(raw: string): string[] {
  return raw
    .split(/[\s,]+/u)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

export function buildAccessKeyInput(
  draft: AccessKeyDraft,
): ConsoleAccessKeyInput {
  return {
    ownerType: draft.ownerType.trim(),
    ownerId: draft.ownerId.trim(),
    resolvedProjectId: draft.resolvedProjectId.trim(),
    resolvedTenantId: draft.resolvedTenantId.trim(),
    keyKind: draft.keyKind.trim(),
    publicKeyPrefix: draft.publicKeyPrefix.trim(),
    displayName: draft.displayName.trim(),
    expiresAt:
      draft.expiresAt.trim().length > 0 ? draft.expiresAt.trim() : null,
    bundleIds: draft.bundleIds,
    metadata: { accountGroupIds: draft.accountGroupIds },
    quota: keyQuotaInput(draft),
  };
}

export function buildAccessBundleInput(
  draft: AccessBundleDraft,
): ConsoleAccessBundleInput {
  const projectId = draft.projectId.trim();
  const billingMode = draft.billingMode.trim();
  const description = draft.description.trim();
  return {
    projectId: projectId.length > 0 ? projectId : null,
    slug: draft.slug.trim(),
    displayName: draft.displayName.trim(),
    billingMode: billingMode.length > 0 ? billingMode : null,
    status: draft.status.trim(),
    description: description.length > 0 ? description : null,
  };
}

export function buildUserCredentialInput(
  draft: UserCredentialDraft,
): ConsoleUserCredentialIssueInput {
  const projectId = draft.projectId.trim();
  const parsedDays = Number.parseInt(draft.durationDays.trim(), 10);
  return {
    userId: draft.userId.trim(),
    projectId: projectId.length > 0 ? projectId : null,
    credentialType: draft.credentialType.trim(),
    durationDays:
      Number.isFinite(parsedDays) && parsedDays > 0 ? parsedDays : 30,
    scope: parseScopeList(draft.scope),
  };
}

export type AccessKeysWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  refreshing: boolean;
  onRefresh: () => void;
  catalog: AccessPanelState<ConsoleAccessCatalog>;
  revealedSecret: AccessRevealedSecret | null;
  onDismissSecret: () => void;
  keyDraft: AccessKeyDraft;
  onKeyDraftChange: (next: AccessKeyDraft) => void;
  onCreateKey: () => Promise<boolean>;
  creatingKey: boolean;
  keyBusyId: string | null;
  onRotateKey: (accessKeyId: string) => Promise<boolean>;
  onKeyLifecycle: (
    accessKeyId: string,
    action: "enable" | "disable" | "delete",
  ) => Promise<boolean>;
  onCopyKey: (accessKeyId: string) => Promise<boolean>;
  onUpdateKey: (
    key: ConsoleAccessKey,
    draft: AccessKeyDraft,
  ) => Promise<boolean>;
  bundleDraft: AccessBundleDraft;
  onBundleDraftChange: (next: AccessBundleDraft) => void;
  onCreateBundle: () => void;
  creatingBundle: boolean;
  affinityDraft: AccessAffinityDraft;
  onAffinityDraftChange: (next: AccessAffinityDraft) => void;
  onInspectAffinity: () => void;
  onResetAffinity: () => void;
  affinity: AccessPanelState<ConsoleAccessStickyAffinity | null>;
  affinityNotice: string | null;
  rotationDraft: ApiAccessRotationDraft;
  onRotationDraftChange: (next: ApiAccessRotationDraft) => void;
  onRotateApiAccess: () => void;
  rotatingApiAccess: boolean;
  lastRotation: ConsoleApiAccessRotation | null;
  credentialDraft: UserCredentialDraft;
  onCredentialDraftChange: (next: UserCredentialDraft) => void;
  onIssueCredential: () => void;
  issuingCredential: boolean;
  lastIssuedCredential: ConsoleIssuedUserCredential | null;
  verifyDraft: UserCredentialVerifyDraft;
  onVerifyDraftChange: (next: UserCredentialVerifyDraft) => void;
  onVerifyCredential: () => void;
  onRevokeCredential: () => void;
  credentialBusy: boolean;
  verification: AccessPanelState<{
    valid: boolean;
    credential: ConsoleUserCredentialCacheEntry | null;
    reason: string;
  }>;
  credentialNotice: string | null;
};
