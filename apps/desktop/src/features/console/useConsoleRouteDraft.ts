import { useCallback, useEffect, useMemo, useState } from "react";
import type { ConsoleRouteConfigResponse, ConsoleRouteDocument, ConsoleRouteConfigCommitRequest, ConsoleSecretPatch } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import { type SecretPatchDraft, buildSecretPatches, createSecretPatchDrafts } from "./secretPatchDraft";
import { type CredentialSecretEdit, buildCredentialSecretPatches } from "./credentialDocument";
import { type ModelRouteDraftRow, modelRouteDraftRowsFromDocument } from "./modelRouteDraft";
import { type ProviderDraftRow, providerDraftRowsFromDocument } from "./providerCredentialDraft";
import { type AccountGroupDraftRow, accountGroupDraftNeedsId, parseAccountGroupBillingMultiplier, documentHasInvalidAccountGroupBillingMultiplier, documentHasIncompleteAccountGroup } from "./accountGroupDraft";
import { parseRouteDocument, formatRouteDocument, isRecord } from "./routeDocument";

export const ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH = "分组 ID 必须填写。";
export const ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN = "Group ID is required.";
export const ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH =
  "分组计费倍率必须是大于等于 0 的数字。";
export const ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN =
  "Group billing multiplier must be a number greater than or equal to 0.";

type ConsoleRouteDraftOptions = {
  routeConfig: ConsoleRouteConfigResponse | null;
  hasSecretAccess: boolean;
  t: (zh: string, en: string) => string;
  invalidateCredentialProbes: () => void;
  accountGroupDraftRowsFromDocument: (document: ConsoleRouteDocument) => AccountGroupDraftRow[];
  onRouteConfigHydrated: () => void;
  setError: (value: string | null) => void;
  setSecretDialogOpen: (value: boolean) => void;
};

export function useConsoleRouteDraft({
  routeConfig,
  hasSecretAccess,
  t,
  invalidateCredentialProbes,
  accountGroupDraftRowsFromDocument,
  onRouteConfigHydrated,
  setError,
  setSecretDialogOpen,
}: ConsoleRouteDraftOptions) {
  const [editorText, setEditorText] = useState("");

  const [commitMessage, setCommitMessage] = useState("");

  const [secretDrafts, setSecretDrafts] = useState<Record<string, SecretPatchDraft>>({});

  const [credentialSecretEdits, setCredentialSecretEdits] = useState<CredentialSecretEdit[]>([]);


  const [modelRouteDraftRows, setModelRouteDraftRows] = useState<ModelRouteDraftRow[]>([]);

  const [providerDraftRows, setProviderDraftRows] = useState<ProviderDraftRow[]>([]);

  const [accountGroupDraftRows, setAccountGroupDraftRows] = useState<AccountGroupDraftRow[]>([]);

  const activeSecretPatches = useMemo(
    () => buildSecretPatches(routeConfig, secretDrafts),
    [routeConfig, secretDrafts],
  );

  const secretPatchDraftDocument = useMemo(() => {
    if (!routeConfig) {
      return null;
    }
    try {
      return editorText.trim().length > 0
        ? parseRouteDocument(editorText)
        : routeConfig.routeConfig.document;
    } catch {
      return routeConfig.routeConfig.document;
    }
  }, [editorText, routeConfig]);

  const secretPatches = useMemo(() => {
    if (!routeConfig || !secretPatchDraftDocument) {
      return activeSecretPatches;
    }
    return buildCredentialSecretPatches({
      activeDocument: routeConfig.routeConfig.document,
      draftDocument: secretPatchDraftDocument,
      activeSecrets: routeConfig.routeConfig.secrets,
      activeSecretPatches,
      credentialSecretEdits,
    });
  }, [activeSecretPatches, credentialSecretEdits, routeConfig, secretPatchDraftDocument]);

  const draftDocumentState = useMemo(() => {
    if (editorText.trim().length === 0) {
      return {
        document: routeConfig?.routeConfig.document ?? null,
        invalid: false,
      };
    }
    try {
      return {
        document: parseRouteDocument(editorText),
        invalid: false,
      };
    } catch {
      return {
        document: routeConfig?.routeConfig.document ?? null,
        invalid: true,
      };
    }
  }, [editorText, routeConfig]);

  const draftMatchesActiveRevision = useMemo(() => {
    if (!routeConfig || draftDocumentState.invalid) {
      return false;
    }
    if (editorText.trim().length === 0) {
      return true;
    }
    return editorText === formatRouteDocument(routeConfig.routeConfig.document);
  }, [draftDocumentState.invalid, editorText, routeConfig]);

  const incompleteAccountGroupDraftCount = useMemo(
    () => accountGroupDraftRows.filter(accountGroupDraftNeedsId).length,
    [accountGroupDraftRows],
  );

  const draftDirty = useMemo(() => {
    if (!routeConfig) {
      return false;
    }
    const activeDocumentText = formatRouteDocument(routeConfig.routeConfig.document);
    const documentChanged = editorText.trim().length > 0 && editorText !== activeDocumentText;
    const messageChanged =
      commitMessage.trim() !== (routeConfig.routeConfig.revision.message ?? "").trim();
    const secretChanged = secretPatches.some((patch) => patch.operation !== "keep");
    return (
      documentChanged ||
      messageChanged ||
      secretChanged ||
      incompleteAccountGroupDraftCount > 0
    );
  }, [commitMessage, editorText, incompleteAccountGroupDraftCount, routeConfig, secretPatches]);

  const buildCommitRequest = useCallback(
    (
      document: ConsoleRouteDocument,
      messageOverride?: string,
      secretPatchOverride?: ConsoleSecretPatch[],
    ): ConsoleRouteConfigCommitRequest => {
      if (!routeConfig) {
        throw new Error("Route configuration is not loaded yet.");
      }
      const message = messageOverride?.trim() || commitMessage.trim();
      const commitSecretPatches = secretPatchOverride ?? secretPatches;
      for (const patch of commitSecretPatches) {
        if (patch.operation !== "keep" && !hasSecretAccess) {
          throw new Error(
            t(
              "执行敏感字段替换或清空前，需要先确认敏感信息访问权限。",
              "Secret replacement or clearing requires confirmed secret access.",
            ),
          );
        }
        if (patch.operation === "replace" && (!patch.value || patch.value.trim().length === 0)) {
          throw new Error(
            t(
              `${patch.path} 的替换值不能为空。`,
              `Replacement secret for ${patch.path} cannot be empty.`,
            ),
          );
        }
      }
      return {
        expectedRevision: routeConfig.routeConfig.revision.id,
        document,
        secretPatches: commitSecretPatches,
        ...(message ? { message } : {}),
      };
    },
    [commitMessage, hasSecretAccess, routeConfig, secretPatches, t],
  );

  const parseDraft = useCallback((): ConsoleRouteConfigCommitRequest => {
    const document = parseRouteDocument(editorText);
    if (
      accountGroupDraftRows.some(accountGroupDraftNeedsId)
    ) {
      throw new Error(
        t(
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH,
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN,
        ),
      );
    }
    if (
      accountGroupDraftRows.some(
        (row) => parseAccountGroupBillingMultiplier(row.billingMultiplier) === null,
      ) ||
      documentHasInvalidAccountGroupBillingMultiplier(document)
    ) {
      throw new Error(
        t(
          ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH,
          ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN,
        ),
      );
    }
    if (documentHasIncompleteAccountGroup(document)) {
      throw new Error(
        t(
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH,
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN,
        ),
      );
    }
    return buildCommitRequest(document);
  }, [accountGroupDraftRows, buildCommitRequest, editorText, t]);

  const replaceEditorDocument = useCallback(
    (document: ConsoleRouteDocument, syncStructuredEditors = false) => {
      invalidateCredentialProbes();
      setEditorText(formatRouteDocument(document));
      if (syncStructuredEditors) {
        setModelRouteDraftRows(modelRouteDraftRowsFromDocument(document));
        setProviderDraftRows(providerDraftRowsFromDocument(document));
        setAccountGroupDraftRows(accountGroupDraftRowsFromDocument(document));
      }
    },
    [invalidateCredentialProbes, accountGroupDraftRowsFromDocument],
  );

  const updateProviderStoragePassword = useCallback(
    (providerId: string, password: string): boolean => {
      if (!hasSecretAccess) {
        setSecretDialogOpen(true);
        pushAppToast(
          "info",
          t(
            "编辑存储密码前需要先确认敏感信息访问权限。",
            "Confirm secret access before editing the storage password.",
          ),
        );
        return false;
      }
      const providerIndex = routeConfig?.routeConfig.document.providers.findIndex(
        (provider) => isRecord(provider) && provider.id === providerId,
      );
      if (providerIndex === undefined || providerIndex < 0) {
        setError(t(`找不到服务商 ${providerId}。`, `Provider ${providerId} could not be found.`));
        return false;
      }
      const path = `/providers/${providerIndex}/credential_storage_password`;
      if (!routeConfig?.routeConfig.secrets.some((secret) => secret.path === path)) {
        setError(t("当前服务商未提供存储密码字段。", "The provider has no storage password field."));
        return false;
      }
      setSecretDrafts((current) => ({
        ...current,
        [path]: { operation: "replace", value: password },
      }));
      setError(null);
      pushAppToast(
        "success",
        t("存储密码已写入当前路由草稿，请保存配置后生效。", "Storage password added to the route draft; save the configuration to apply it."),
      );
      return true;
    },
    [hasSecretAccess, routeConfig, setError, setSecretDialogOpen, t],
  );

  useEffect(() => {
    if (!routeConfig) {
      return;
    }
    setEditorText(JSON.stringify(routeConfig.routeConfig.document, null, 2));
    setCommitMessage(routeConfig.routeConfig.revision.message ?? "");
    setSecretDrafts(createSecretPatchDrafts(routeConfig));
    setCredentialSecretEdits([]);
    // Preserve dependent UI/probe resets between draft-value and row hydration.
    onRouteConfigHydrated();
    setModelRouteDraftRows(modelRouteDraftRowsFromDocument(routeConfig.routeConfig.document));
    setProviderDraftRows(providerDraftRowsFromDocument(routeConfig.routeConfig.document));
    setAccountGroupDraftRows(accountGroupDraftRowsFromDocument(routeConfig.routeConfig.document));
  }, [accountGroupDraftRowsFromDocument, onRouteConfigHydrated, routeConfig]);

  return {
    editorText,
    setEditorText,
    commitMessage,
    setCommitMessage,
    secretDrafts,
    setSecretDrafts,
    credentialSecretEdits,
    setCredentialSecretEdits,
    modelRouteDraftRows,
    setModelRouteDraftRows,
    providerDraftRows,
    setProviderDraftRows,
    accountGroupDraftRows,
    setAccountGroupDraftRows,
    activeSecretPatches,
    secretPatches,
    draftDocumentState,
    draftMatchesActiveRevision,
    incompleteAccountGroupDraftCount,
    draftDirty,
    buildCommitRequest,
    parseDraft,
    replaceEditorDocument,
    updateProviderStoragePassword,
  };
}
