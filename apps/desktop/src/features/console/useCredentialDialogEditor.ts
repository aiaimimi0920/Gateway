import { useCallback, useRef, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import type { AccountsLedgerPilotAccount } from "./AccountsLedgerWorkspace";
import type { CredentialDialogMode, CredentialDialogValue } from "./CredentialDialog";
import { addExplicitCredential, deleteExplicitCredential, updateExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import { emptyCredentialDialogValue, credentialDialogValueFromDocument, duplicateCredentialDialogValue, parseSupportedModelsText } from "./providerCredentialDraft";
import { parseRouteDocument } from "./routeDocument";

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
    setCredentialDialogState({
      mode: "add",
      initialValue: emptyCredentialDialogValue(providerId),
    });
  }, []);

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

      const supportedModels = parseSupportedModelsText(value.supportedModelsText);
      const fields: Record<string, unknown> = {
        account_name: value.accountName || undefined,
        enabled: value.enabled,
        base_url: value.baseUrl || undefined,
        supported_models: supportedModels.length > 0 ? supportedModels : undefined,
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
        updateCredentialSecretEdit(value);
        replaceEditorDocument(nextDocument, true);
        setError(null);
        pushAppToast(
          "info",
          t(
            `账号 ${value.accountName || value.credentialId} 已写入草稿，保存路由配置后生效。`,
            `Account ${value.accountName || value.credentialId} was added to the draft and will take effect after saving the route config.`,
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

  return {
    openAddCredentialDialog,
    openEditCredentialDialog,
    updateCredentialSecretEdit,
    applyCredentialDialogValue,
    togglePilotCredentialDispatch,
    openDuplicateCredentialDialog,
    removeCredential,
  };
}
