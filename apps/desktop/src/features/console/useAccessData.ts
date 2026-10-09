import { useCallback, useEffect, useRef, useState } from "react";

import type { ConsoleApi } from "../../api/console";
import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
  ConsoleAccessStickyAffinity,
  ConsoleApiAccessRotation,
  ConsoleIssuedUserCredential,
  ConsoleUserCredentialCacheEntry,
} from "../../api/contracts";
import {
  ACCESS_DEFAULT_AFFINITY_DRAFT,
  ACCESS_DEFAULT_BUNDLE_DRAFT,
  ACCESS_DEFAULT_CREDENTIAL_DRAFT,
  ACCESS_DEFAULT_ROTATION_DRAFT,
  ACCESS_DEFAULT_VERIFY_DRAFT,
  buildAccessBundleInput,
  buildUserCredentialInput,
  parseScopeList,
  type AccessAffinityDraft,
  type AccessBundleDraft,
  type AccessKeyDraft,
  type AccessPanelState,
  type AccessRevealedSecret,
  type ApiAccessRotationDraft,
  type UserCredentialDraft,
  type UserCredentialVerifyDraft,
} from "./accessKeysTypes";
import { useAccessKeyActions } from "./useAccessKeyActions";

type TranslateFn = (zh: string, en: string) => string;

type VerificationView = {
  valid: boolean;
  credential: ConsoleUserCredentialCacheEntry | null;
  reason: string;
};

function idlePanel<T>(): AccessPanelState<T> {
  return { data: null, error: null, loading: false };
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

export type UseAccessDataOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  /** Fetching only starts once the workspace is on screen. */
  active: boolean;
  t: TranslateFn;
};

export type UseAccessDataResult = {
  catalog: AccessPanelState<ConsoleAccessCatalog>;
  refreshing: boolean;
  refresh: () => void;

  revealedSecret: AccessRevealedSecret | null;
  dismissSecret: () => void;

  keyDraft: AccessKeyDraft;
  setKeyDraft: (next: AccessKeyDraft) => void;
  createKey: () => Promise<boolean>;
  creatingKey: boolean;
  keyBusyId: string | null;
  rotateKey: (accessKeyId: string) => Promise<boolean>;
  keyLifecycle: (
    accessKeyId: string,
    action: "enable" | "disable" | "delete",
  ) => Promise<boolean>;
  copyKey: (accessKeyId: string) => Promise<boolean>;
  updateKey: (key: ConsoleAccessKey, draft: AccessKeyDraft) => Promise<boolean>;

  bundleDraft: AccessBundleDraft;
  setBundleDraft: (next: AccessBundleDraft) => void;
  createBundle: () => void;
  creatingBundle: boolean;

  affinityDraft: AccessAffinityDraft;
  setAffinityDraft: (next: AccessAffinityDraft) => void;
  inspectAffinity: () => void;
  resetAffinity: () => void;
  affinity: AccessPanelState<ConsoleAccessStickyAffinity | null>;
  affinityNotice: string | null;

  rotationDraft: ApiAccessRotationDraft;
  setRotationDraft: (next: ApiAccessRotationDraft) => void;
  rotateApiAccess: () => void;
  rotatingApiAccess: boolean;
  lastRotation: ConsoleApiAccessRotation | null;

  credentialDraft: UserCredentialDraft;
  setCredentialDraft: (next: UserCredentialDraft) => void;
  issueCredential: () => void;
  issuingCredential: boolean;
  lastIssuedCredential: ConsoleIssuedUserCredential | null;

  verifyDraft: UserCredentialVerifyDraft;
  setVerifyDraft: (next: UserCredentialVerifyDraft) => void;
  verifyCredential: () => void;
  revokeCredential: () => void;
  credentialBusy: boolean;
  verification: AccessPanelState<VerificationView>;
  credentialNotice: string | null;
};

export function useAccessData({
  api,
  managementToken,
  active,
  t,
}: UseAccessDataOptions): UseAccessDataResult {
  const [catalog, setCatalog] =
    useState<AccessPanelState<ConsoleAccessCatalog>>(idlePanel);
  const [refreshing, setRefreshing] = useState(false);

  const [revealedSecret, setRevealedSecret] =
    useState<AccessRevealedSecret | null>(null);

  const [bundleDraft, setBundleDraft] = useState<AccessBundleDraft>({
    ...ACCESS_DEFAULT_BUNDLE_DRAFT,
  });
  const [creatingBundle, setCreatingBundle] = useState(false);

  const [affinityDraft, setAffinityDraft] = useState<AccessAffinityDraft>({
    ...ACCESS_DEFAULT_AFFINITY_DRAFT,
  });
  const [affinity, setAffinity] =
    useState<AccessPanelState<ConsoleAccessStickyAffinity | null>>(idlePanel);
  const [affinityNotice, setAffinityNotice] = useState<string | null>(null);

  const [rotationDraft, setRotationDraft] = useState<ApiAccessRotationDraft>({
    ...ACCESS_DEFAULT_ROTATION_DRAFT,
  });
  const [rotatingApiAccess, setRotatingApiAccess] = useState(false);
  const [lastRotation, setLastRotation] =
    useState<ConsoleApiAccessRotation | null>(null);

  const [credentialDraft, setCredentialDraft] = useState<UserCredentialDraft>({
    ...ACCESS_DEFAULT_CREDENTIAL_DRAFT,
  });
  const [issuingCredential, setIssuingCredential] = useState(false);
  const [lastIssuedCredential, setLastIssuedCredential] =
    useState<ConsoleIssuedUserCredential | null>(null);

  const [verifyDraft, setVerifyDraft] = useState<UserCredentialVerifyDraft>({
    ...ACCESS_DEFAULT_VERIFY_DRAFT,
  });
  const [credentialBusy, setCredentialBusy] = useState(false);
  const [verification, setVerification] =
    useState<AccessPanelState<VerificationView>>(idlePanel);
  const [credentialNotice, setCredentialNotice] = useState<string | null>(null);

  // A later load always wins, so a slow first response cannot overwrite it.
  const generationRef = useRef(0);

  const loadCatalog = useCallback(async () => {
    if (!managementToken || typeof api.getAccessCatalog !== "function") {
      return;
    }
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    setRefreshing(true);
    setCatalog((current) => ({ ...current, loading: true }));
    try {
      const data = await api.getAccessCatalog(managementToken);
      if (generation !== generationRef.current) {
        return;
      }
      setCatalog({ data, error: null, loading: false });
    } catch (cause) {
      if (generation !== generationRef.current) {
        return;
      }
      setCatalog({ data: null, error: errorMessage(cause), loading: false });
    } finally {
      if (generation === generationRef.current) {
        setRefreshing(false);
      }
    }
  }, [api, managementToken]);

  useEffect(() => {
    if (!active || !managementToken) {
      return;
    }
    void loadCatalog();
  }, [active, loadCatalog, managementToken]);

  const refresh = useCallback(() => {
    void loadCatalog();
  }, [loadCatalog]);

  const dismissSecret = useCallback(() => setRevealedSecret(null), []);

  const keyActions = useAccessKeyActions({
    api,
    managementToken,
    active,
    t,
    catalog,
    setCatalog,
    loadCatalog,
    setRevealedSecret,
  });

  const createBundle = useCallback(() => {
    if (!managementToken || typeof api.createAccessBundle !== "function") {
      return;
    }
    const run = api.createAccessBundle;
    setCreatingBundle(true);
    void (async () => {
      try {
        await run(managementToken, buildAccessBundleInput(bundleDraft));
        setBundleDraft({ ...ACCESS_DEFAULT_BUNDLE_DRAFT });
        await loadCatalog();
      } catch (cause) {
        setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
      } finally {
        setCreatingBundle(false);
      }
    })();
  }, [api, bundleDraft, loadCatalog, managementToken]);

  /** Both affinity calls are Redis-only, so they work without Postgres. */
  const affinityParams = useCallback(() => {
    const explicitSessionKey = affinityDraft.explicitSessionKey.trim();
    return {
      accessKeyId: affinityDraft.accessKeyId.trim(),
      model: affinityDraft.model.trim(),
      explicitSessionKey:
        explicitSessionKey.length > 0 ? explicitSessionKey : undefined,
    };
  }, [affinityDraft]);

  const inspectAffinity = useCallback(() => {
    if (!managementToken || typeof api.inspectAccessAffinity !== "function") {
      return;
    }
    const run = api.inspectAccessAffinity;
    setAffinityNotice(null);
    setAffinity((current) => ({ ...current, loading: true }));
    void (async () => {
      try {
        const response = await run(managementToken, affinityParams());
        setAffinity({ data: response.affinity, error: null, loading: false });
      } catch (cause) {
        setAffinity({ data: null, error: errorMessage(cause), loading: false });
      }
    })();
  }, [affinityParams, api, managementToken]);

  const resetAffinity = useCallback(() => {
    if (!managementToken || typeof api.resetAccessAffinity !== "function") {
      return;
    }
    const run = api.resetAccessAffinity;
    setAffinity((current) => ({ ...current, loading: true }));
    void (async () => {
      try {
        const response = await run(managementToken, affinityParams());
        // Reset clears the binding, so the panel drops its stale row too.
        setAffinity({ data: null, error: null, loading: false });
        setAffinityNotice(
          response.message ??
            t("粘性绑定已清除。", "Sticky affinity has been cleared."),
        );
      } catch (cause) {
        setAffinity((current) => ({
          ...current,
          error: errorMessage(cause),
          loading: false,
        }));
      }
    })();
  }, [affinityParams, api, managementToken, t]);

  const rotateApiAccess = useCallback(() => {
    if (!managementToken || typeof api.rotateApiAccess !== "function") {
      return;
    }
    const run = api.rotateApiAccess;
    const name = rotationDraft.name.trim();
    const actorUserId = rotationDraft.actorUserId.trim();
    setRotatingApiAccess(true);
    void (async () => {
      try {
        const rotation = await run(managementToken, {
          projectId: rotationDraft.projectId.trim(),
          name: name.length > 0 ? name : undefined,
          actorUserId: actorUserId.length > 0 ? actorUserId : undefined,
        });
        setLastRotation(rotation);
        setRevealedSecret({
          kind: "api-access",
          label: t(
            `项目 API 明文 · ${rotation.projectId}`,
            `Project API plaintext · ${rotation.projectId}`,
          ),
          secret: rotation.token,
        });
      } catch (cause) {
        setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
      } finally {
        setRotatingApiAccess(false);
      }
    })();
  }, [api, managementToken, rotationDraft, t]);

  const issueCredential = useCallback(() => {
    if (!managementToken || typeof api.issueUserCredential !== "function") {
      return;
    }
    const run = api.issueUserCredential;
    setIssuingCredential(true);
    setCredentialNotice(null);
    void (async () => {
      try {
        const response = await run(
          managementToken,
          buildUserCredentialInput(credentialDraft),
        );
        setLastIssuedCredential(response.credential);
        setRevealedSecret({
          kind: "user-credential",
          label: t(
            `用户凭据明文 · ${response.credential.userId}`,
            `User credential plaintext · ${response.credential.userId}`,
          ),
          secret: response.credential.credentialKey,
        });
      } catch (cause) {
        setVerification((current) => ({
          ...current,
          error: errorMessage(cause),
        }));
      } finally {
        setIssuingCredential(false);
      }
    })();
  }, [api, credentialDraft, managementToken, t]);

  const verifyCredential = useCallback(() => {
    if (!managementToken || typeof api.verifyUserCredential !== "function") {
      return;
    }
    const run = api.verifyUserCredential;
    // `scope` is a single required scope on the wire, so only the first wins.
    const [scope] = parseScopeList(verifyDraft.scope);
    setCredentialBusy(true);
    setCredentialNotice(null);
    setVerification((current) => ({ ...current, loading: true }));
    void (async () => {
      try {
        const result = await run(
          managementToken,
          verifyDraft.credentialKey.trim(),
          scope,
        );
        setVerification({ data: result, error: null, loading: false });
      } catch (cause) {
        setVerification({
          data: null,
          error: errorMessage(cause),
          loading: false,
        });
      } finally {
        setCredentialBusy(false);
      }
    })();
  }, [api, managementToken, verifyDraft]);

  const revokeCredential = useCallback(() => {
    if (!managementToken || typeof api.revokeUserCredential !== "function") {
      return;
    }
    const run = api.revokeUserCredential;
    setCredentialBusy(true);
    void (async () => {
      try {
        const response = await run(
          managementToken,
          verifyDraft.credentialKey.trim(),
          "revoked from console",
        );
        // The cache entry is gone, so the stale verification result goes with it.
        setVerification(idlePanel());
        setCredentialNotice(
          response.message ?? t("凭据已吊销。", "Credential has been revoked."),
        );
      } catch (cause) {
        setVerification((current) => ({
          ...current,
          error: errorMessage(cause),
        }));
      } finally {
        setCredentialBusy(false);
      }
    })();
  }, [api, managementToken, t, verifyDraft]);

  return {
    catalog,
    refreshing,
    refresh,
    revealedSecret,
    dismissSecret,
    ...keyActions,
    bundleDraft,
    setBundleDraft,
    createBundle,
    creatingBundle,
    affinityDraft,
    setAffinityDraft,
    inspectAffinity,
    resetAffinity,
    affinity,
    affinityNotice,
    rotationDraft,
    setRotationDraft,
    rotateApiAccess,
    rotatingApiAccess,
    lastRotation,
    credentialDraft,
    setCredentialDraft,
    issueCredential,
    issuingCredential,
    lastIssuedCredential,
    verifyDraft,
    setVerifyDraft,
    verifyCredential,
    revokeCredential,
    credentialBusy,
    verification,
    credentialNotice,
  };
}
