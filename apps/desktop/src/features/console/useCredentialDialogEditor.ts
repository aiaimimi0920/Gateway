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
import { normalizeCloudflareCredential } from "./cloudflareWorkersAi";
import { applyModelSelection, dialogModelSelection, selectedModels } from "./credentialModelSelection";
import { applyChatgptWebMaterial } from "./chatgptWebMaterial";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";
import { sessionProductForPreset } from "./providerSessionProducts";
import { mergeCredentialSecretEditEntries } from "./secretPatchDraft";
import { normalizeSapCredential } from "./sapAiCore";
import { normalizeLumaInferenceCredential, lumaInferenceTarget } from "./lumaInferenceCredential";
import { applyBedrockMaterial } from "./bedrockMaterialApply";
import { isBedrockProvider } from "./bedrockMaterial";
import { modelIdPolicy } from "./providerModelId";
import { isQwenProvider } from "./qwenEndpoint";
import { normalizeQwenCredentialDraft } from "./qwenCredentialDraft";
import { qwenMaterialTarget, type QwenMaterialBinding } from "./qwenMaterialBinding";
import { kiroKeyMaterialTarget, type KiroKeyMaterialBinding } from "./kiroKeyMaterialBinding";
import { isVertexExpressProvider, normalizeVertexExpressDraft, vertexExpressMaterialTarget, type VertexExpressMaterialBinding } from "./vertexExpress";
import { withModelEditors } from "./credentialModelSelection";
import { normalizeVertexOAuthDraft } from "./vertexOAuth";
import { isLumaAgentsProvider, normalizeLumaAgentsDraft, lumaAgentsMaterialTarget, type LumaAgentsMaterialBinding } from "./lumaAgents";
import { freebuffBusinessFields } from "./freebuffBusiness";
import { dimCatalogFields } from "./dimModelCatalog";
import { lobsteraiCatalogFields } from "./lobsteraiModelCatalog";
import { commandcodeZdrFields } from "./commandcodeZdr";
import { filterVercelAccountModels, registerVercelModels, VERCEL_CREDIT_AUTH_MODE, VERCEL_PRESET } from "./vercelFreeCatalog";
import { parseAccountConcurrencyLimit } from "./accountConcurrencyLimit";

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
      if (provider && !isQwenProvider(provider) && !catalogProviderKind(provider) && provider.preset !== "chatgpt-codex-oauth-official-api") {
        const models = Array.isArray(provider.supported_models) ? provider.supported_models.filter((id) => typeof id === "string") : [];
        initialValue.manualModelsText = filterVercelAccountModels(provider.preset, undefined, models).join("\n");
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

  const updateCredentialSecretEdit = useCallback((value: CredentialDialogValue, qwenBinding?: QwenMaterialBinding, kiroKeyBinding?: KiroKeyMaterialBinding, vertexExpressBinding?: VertexExpressMaterialBinding, lumaAgentsBinding?: LumaAgentsMaterialBinding) => {
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
        // Keep refers to the current draft, including an unsaved replacement.
        return current;
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
            ...(qwenBinding ? { qwenBinding } : {}),
            ...(kiroKeyBinding ? { kiroKeyBinding } : {}),
            ...(vertexExpressBinding ? { vertexExpressBinding } : {}),
            ...(lumaAgentsBinding ? { lumaAgentsBinding } : {}),
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
    (input: CredentialDialogValue) => {
      let value = input;
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find((p) => p && typeof p === "object" &&
        (p as Record<string, unknown>).id === value.providerId) as Record<string, unknown> | undefined;
      try {
        const previous = credentialDialogState?.mode === "edit"
          ? credentialDialogValueFromDocument(document, value.providerId, value.credentialId) : null;
        value = provider?.preset === "cloudflare-workers-ai" ? normalizeCloudflareCredential(value, previous)
          : normalizeQwenCredentialDraft(provider, value, previous);
      }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      try { value = normalizeVertexExpressDraft(provider, isVertexExpressProvider(provider) ? withModelEditors(value) : value, credentialDialogState?.mode === "edit"); }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      try { value = normalizeVertexOAuthDraft(provider, value, credentialDialogState?.mode === "edit"); }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      try { value = normalizeLumaAgentsDraft(provider, value, credentialDialogState?.mode === "edit"); }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      try { value = normalizeLumaInferenceCredential(provider, value); }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      let selection;
      try { value = normalizeSapCredential(provider, value); }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      try { selection = dialogModelSelection(value, modelIdPolicy(provider, { discovery: value.discovery })); }
      catch { setError(t("模型列表无效。", "Invalid model list.")); return; }
      const vercelAuthMode = value.vercelMonthlyCredit ? VERCEL_CREDIT_AUTH_MODE : "vercel_free_key";
      const filterModels = (models: string[]) => filterVercelAccountModels(provider?.preset, vercelAuthMode, models);
      if (selection) selection = { manual: filterModels(selection.manual), fetched: filterModels(selection.fetched) };
      const supportedModels = selection ? selectedModels(selection) : filterModels(parseSupportedModelsText(value.supportedModelsText));
      let businessFields;
      try { businessFields = {
        ...freebuffBusinessFields(provider, value), ...dimCatalogFields(provider, value), ...lobsteraiCatalogFields(provider, value),
        max_concurrent_requests: parseAccountConcurrencyLimit(value.maxConcurrentRequestsText),
      }; }
      catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return; }
      const discoverLater = !selection && !value.discovery && value.apiKeyOperation === "replace" &&
        provider && !isQwenProvider(provider) && !isBedrockProvider(provider) && !catalogProviderKind(provider) && supportsAccountDiscovery(provider);
      const fields: Record<string, unknown> = {
        ...businessFields,
        ...commandcodeZdrFields(provider, value),
        ...(provider?.preset === VERCEL_PRESET ? { auth_mode: vercelAuthMode } : {}),
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
        ...(isQwenProvider(provider) ? { discovery: undefined, discovery_job: undefined }
          : value.discovery ? { discovery: value.discovery } : value.apiKeyOperation !== "keep" ? { discovery: undefined } : {}),
        ...(!value.modelCatalog ? { model_catalog: undefined } : {}),
        ...(discoverLater ? { discovery_job: { id: crypto.randomUUID(), status: "pending" } }
          : selection || value.modelCatalog || value.discovery || value.apiKeyOperation === "clear" ? { discovery_job: undefined } : {}),
      };
      try {
        if (sessionProductForPreset(provider?.preset) && !value.sessionMaterial
          && (credentialDialogState?.mode !== "edit" || value.apiKeyOperation !== "keep")) {
          throw new Error("此产品需要完整会话材料，不能通过 API Key 表单添加或替换。");
        }
        if (isBedrockProvider(provider) && value.apiKeyOperation !== "keep" && (value.apiKeyOperation !== "replace" || !value.bedrockMaterial)) {
          throw new Error("Bedrock 账号需要完整、显式的认证材料，不能使用单 API Key 字段或保存半套材料。");
        }
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
          ? registerChatgptCredentialModels(nextDocument, value.providerId, supportedModels)
          : registerVercelModels(nextDocument, value.providerId, supportedModels);
        const discoveredDocument = value.modelCatalog
          ? applyOfficialCatalog(completeDocument, value.providerId, value.credentialId, value.modelCatalog)
          : value.discovery ? applyAccountDiscovery(nextDocument, value.providerId, value.credentialId, value.discovery)
          : completeDocument;
        const selectedDocument = selection ? applyModelSelection(discoveredDocument, value.providerId, value.credentialId, selection) : discoveredDocument;
        const webImport = isBedrockProvider(provider) && value.apiKeyOperation === "replace" && value.bedrockMaterial
          ? applyBedrockMaterial(selectedDocument, value.providerId, value.credentialId, value.bedrockMaterial)
          : value.sessionMaterial
          ? applyProviderSessionMaterial(selectedDocument, value.providerId, value.credentialId, value.sessionMaterial)
          : value.webSessionMaterial ? applyChatgptWebMaterial(selectedDocument, value.providerId, value.credentialId, value.webSessionMaterial) : null;
        if (!value.sessionMaterial && !sessionProductForPreset(provider?.preset) && !isBedrockProvider(provider)
          // Metadata-only Keep must retain a previously staged, audience-bound Key until autosave.
          && (!isQwenProvider(provider) && provider?.preset !== "kiro-api-key" && provider?.preset !== "agnes-api" && provider?.preset !== "sensenova-token-plan" && provider?.preset !== "commandcode-api" && provider?.preset !== "sap-aicore-chat" && provider?.preset !== "minimax-m-plan" && provider?.preset !== "minimax-m-plan-cn" && !isVertexExpressProvider(provider) && !isLumaAgentsProvider(provider) || value.apiKeyOperation !== "keep")) updateCredentialSecretEdit(value,
            isQwenProvider(provider) && value.apiKeyOperation === "replace" ? qwenMaterialTarget(selectedDocument, value.providerId, value.credentialId) : undefined,
            provider?.preset === "kiro-api-key" && value.apiKeyOperation === "replace" ? kiroKeyMaterialTarget(selectedDocument, value.providerId, value.credentialId) : undefined,
            isVertexExpressProvider(provider) && value.apiKeyOperation === "replace" ? vertexExpressMaterialTarget(selectedDocument, value.providerId, value.credentialId) : undefined,
            isLumaAgentsProvider(provider) && value.apiKeyOperation === "replace" ? lumaAgentsMaterialTarget(selectedDocument, value.providerId, value.credentialId) : undefined);
        if (webImport) setCredentialSecretEdits((current) => mergeCredentialSecretEditEntries(current, webImport.secretEdits));
        if (provider?.preset === "lumalabs-inference-api" && value.apiKeyOperation === "replace") {
          const binding = lumaInferenceTarget(selectedDocument, value.providerId, value.credentialId);
          setCredentialSecretEdits(current => current.map(edit => edit.providerId === value.providerId
            && edit.credentialId === value.credentialId && edit.field === "api_key" ? { ...edit, lumaInferenceBinding: binding } : edit));
        }
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
