import { useCallback, useRef, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import type { AccountsLedgerPilotAccount } from "./AccountsLedgerWorkspace";
import type { CredentialDialogMode, CredentialDialogValue } from "./CredentialDialog";
import { addExplicitCredential, deleteExplicitCredential, updateExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import { emptyCredentialDialogValue, credentialDialogValueFromDocument, duplicateCredentialDialogValue, parseSupportedModelsText } from "./providerCredentialDraft";
import { parseRouteDocument } from "./routeDocument";
import { removeProviderDocument } from "./providerRemovalDocument";
import { applyAccountDiscovery, supportsAccountDiscovery } from "./accountDiscovery";
import { registerChatgptCredentialModels } from "./chatgptCredentialModels";
import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import { applyOfficialCatalog, catalogProviderKind } from "./officialModelCatalog";
import { applyModelSelection, dialogModelSelection, selectedModels } from "./credentialModelSelection";
import { applyChatgptWebMaterial } from "./chatgptWebMaterial";
import { mergeCredentialSecretEditEntries } from "./secretPatchDraft";

type CredentialDialogEditorOptions = {
  editorText: string;
  credentialDialogState: { mode: CredentialDialogMode; initialValue: CredentialDialogValue | null } | null;
  setCredentialDialogState: Dispatch<SetStateAction<{ mode: CredentialDialogMode; initialValue: CredentialDialogValue | null } | null>>;
  setCredentialSecretEdits: Dispatch<SetStateAction<CredentialSecretEdit[]>>;
  setError: Dispatch<SetStateAction<string | null>>;
  replaceEditorDocument: (document: ConsoleRouteDocument, syncStructuredEditors?: boolean) => void;
  t: (zh: string, en: string) => string;
};

export function useCredentialDialogEditor({
  editorText,
  credentialDialogState,
  setCredentialDialogState,
  setCredentialSecretEdits,
  setError,
  replaceEditorDocument,
  t,
}: CredentialDialogEditorOptions) {
  // Pool buttons update several credentials before React renders the new draft.
  const dispatchDraftRef = useRef(editorText);
  dispatchDraftRef.current = editorText;
  const openAddCredentialDialog = useCallback((providerId = "") => {
    const initialValue = emptyCredentialDialogValue(providerId);
    try {
      const provider = parseRouteDocument(editorText).providers.find((p) => p && typeof p === "object"
        && (p as Record<string, unknown>).id === providerId) as Record<string, unknown> | undefined;
      if (provider && !catalogProviderKind(provider) && provider.preset !== "chatgpt-codex-oauth-official-api") {
        initialValue.manualModelsText = Array.isArray(provider.supported_models) ? provider.supported_models.filter((id) => typeof id === "string").join("\n") : "";
        initialValue.fetchedModelsText = "";
        initialValue.supportedModelsText = initialValue.manualModelsText;
      }
    } catch { /* Invalid drafts are rejected by the normal save path. */ }
    setCredentialDialogState({
      mode: "add",
      initialValue,
    });
  }, [editorText]);

  const openEditCredentialDialog = useCallback(
    (providerId: string, credentialId: string) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      const initialValue = credentialDialogValueFromDocument(
        document,
        providerId,
        credentialId,
      );
      if (!initialValue) {
        setError(
          t(
            `找不到账号 ${credentialId}。`,
            `Account ${credentialId} could not be found.`,
          ),
        );
        return;
      }
      setCredentialDialogState({ mode: "edit", initialValue });
    },
    [editorText, t],
  );

  const updateCredentialSecretEdit = useCallback((value: CredentialDialogValue) => {
    setCredentialSecretEdits((current) => {
      const next = current.filter(
        (entry) =>
          !(
            entry.providerId === value.providerId &&
            entry.credentialId === value.credentialId &&
            entry.field === "api_key"
          ),
      );
      if (value.apiKeyOperation === "keep") {
        return next;
      }
      if (value.apiKeyOperation === "replace") {
        return [
          ...next,
          {
            providerId: value.providerId,
            credentialId: value.credentialId,
            field: "api_key",
            operation: "replace",
            value: value.apiKeyValue,
          },
        ];
      }
      return [
        ...next,
        {
          providerId: value.providerId,
          credentialId: value.credentialId,
          field: "api_key",
          operation: "clear",
        },
      ];
    });
  }, []);

  const applyCredentialDialogValue = useCallback(
    (value: CredentialDialogValue) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      let selection;
      try { selection = dialogModelSelection(value); }
      catch { setError(t("模型列表无效。", "Invalid model list.")); return; }
      const supportedModels = selection ? selectedModels(selection) : parseSupportedModelsText(value.supportedModelsText);
      const provider = document.providers.find((p) => p && typeof p === "object" &&
        (p as Record<string, unknown>).id === value.providerId) as Record<string, unknown> | undefined;
      const discoverLater = !selection && !value.discovery && value.apiKeyOperation === "replace" &&
        provider && !catalogProviderKind(provider) && supportsAccountDiscovery(provider);
      const fields: Record<string, unknown> = {
        ...(provider?.preset === "chatgpt-codex-oauth-official-api" && CHATGPT_POOL_CATEGORIES.some((group) => group.id === value.chatgptGroup)
          ? { credential_identity_category_id: value.chatgptGroup } : {}),
        // Token import is additive only; editing must preserve existing OAuth headers/refresh state.
        ...(credentialDialogState?.mode === "add" && provider?.preset === "chatgpt-codex-oauth-official-api" && value.chatgptAccountId
          ? { headers: { "Chatgpt-Account-Id": value.chatgptAccountId } } : {}),
        codex_basispoints_enabled: value.codexBasispointsEnabled,
        account_name: value.accountName || undefined,
        enabled: value.enabled,
        base_url: value.baseUrl || undefined,
        ...(!selection ? { supported_models: supportedModels.length > 0 ? supportedModels : undefined } : {}),
        ...(value.discovery ? { discovery: value.discovery } : value.apiKeyOperation !== "keep" ? { discovery: undefined } : {}),
        ...(!value.modelCatalog ? { model_catalog: undefined } : {}),
        ...(discoverLater ? { discovery_job: { id: crypto.randomUUID(), status: "pending" } }
          : selection || value.modelCatalog || value.discovery || value.apiKeyOperation === "clear" ? { discovery_job: undefined } : {}),
      };
      try {
        const nextDocument =
          credentialDialogState?.mode === "edit"
            ? updateExplicitCredential(
                document,
                {
                  providerId: value.providerId,
                  credentialId: value.credentialId,
                },
                fields,
              )
            : addExplicitCredential(document, {
                providerId: value.providerId,
                credential: {
                  id: value.credentialId,
                  ...fields,
                },
              });
        const completeDocument = provider?.preset === "chatgpt-codex-oauth-official-api"
          ? registerChatgptCredentialModels(nextDocument, value.providerId, supportedModels) : nextDocument;
        const discoveredDocument = value.modelCatalog
          ? applyOfficialCatalog(completeDocument, value.providerId, value.credentialId, value.modelCatalog)
          : value.discovery ? applyAccountDiscovery(nextDocument, value.providerId, value.credentialId, value.discovery)
          : completeDocument;
        const selectedDocument = selection ? applyModelSelection(discoveredDocument, value.providerId, value.credentialId, selection) : discoveredDocument;
        const webImport = value.webSessionMaterial
          ? applyChatgptWebMaterial(selectedDocument, value.providerId, value.credentialId, value.webSessionMaterial) : null;
        updateCredentialSecretEdit(value);
        if (webImport) setCredentialSecretEdits((current) => mergeCredentialSecretEditEntries(current, webImport.secretEdits));
        replaceEditorDocument(webImport?.document ?? selectedDocument, true);
        setError(null);
        pushAppToast(
          "info",
          t(
            `账号 ${value.accountName || value.credentialId} 正在保存。`,
            `Saving account ${value.accountName || value.credentialId}.`,
          ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [
      credentialDialogState?.mode,
      editorText,
      replaceEditorDocument,
      t,
      updateCredentialSecretEdit,
    ],
  );

  const togglePilotCredentialDispatch = useCallback(
    (providerId: string, credentialId: string, nextEnabled: boolean) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(dispatchDraftRef.current);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      if (credentialDialogValueFromDocument(document, providerId, credentialId) === null) {
        setError(
          t(
            `账号 ${credentialId} 不在当前草稿的服务商 ${providerId} 下，无法切换调度状态。`,
            `Account ${credentialId} is not under provider ${providerId} in the current draft, so its dispatch state cannot be toggled.`,
          ),
        );
        return;
      }

      try {
        const nextDocument = updateExplicitCredential(
          document,
          {
            providerId,
            credentialId,
          },
          {
            enabled: nextEnabled,
          },
        );
        dispatchDraftRef.current = JSON.stringify(nextDocument, null, 2);
        replaceEditorDocument(nextDocument, true);
        setError(null);
        pushAppToast(
          "info",
          nextEnabled
            ? t(
                `账号 ${credentialId} 已恢复调度，正在自动保存。`,
                `Account ${credentialId} was resumed in the draft and is being saved automatically.`,
              )
            : t(
                `账号 ${credentialId} 已暂停调度，正在自动保存。`,
                `Account ${credentialId} was paused in the draft and is being saved automatically.`,
              ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [editorText, replaceEditorDocument, t],
  );

  const openDuplicateCredentialDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      setCredentialDialogState({
        mode: "add",
        initialValue: duplicateCredentialDialogValue(document, providerId, account),
      });
    },
    [editorText, t],
  );

  const removeCredential = useCallback(
    (providerId: string, credentialId: string, displayName: string) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      try {
        const nextDocument = deleteExplicitCredential(document, {
          providerId,
          credentialId,
        });
        setCredentialSecretEdits((current) =>
          current.filter(
            (entry) =>
              !(entry.providerId === providerId && entry.credentialId === credentialId),
          ),
        );
        replaceEditorDocument(nextDocument, true);
        setError(null);
        pushAppToast(
          "info",
          t(
            `账号 ${displayName} 已从草稿删除，保存路由配置后生效。`,
            `Account ${displayName} was removed from the draft and will take effect after saving the route config.`,
          ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [editorText, replaceEditorDocument, t],
  );

  const removeProvider = useCallback((providerId: string) => {
    try {
      const next = removeProviderDocument(parseRouteDocument(editorText), providerId);
      setCredentialSecretEdits((edits) => edits.filter((edit) => edit.providerId !== providerId));
      replaceEditorDocument(next, true);
      setError(null);
      pushAppToast("info", t("凭据池已从草稿移除，正在自动保存。", "The pool was removed from the draft and is being saved automatically."));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [editorText, replaceEditorDocument, setCredentialSecretEdits, setError, t]);

  return {
    removeProvider,
    openAddCredentialDialog,
    openEditCredentialDialog,
    updateCredentialSecretEdit,
    applyCredentialDialogValue,
    togglePilotCredentialDispatch,
    openDuplicateCredentialDialog,
    removeCredential,
  };
}
