import { type FormEvent, useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useDiscoverySubmission } from "./useDiscoverySubmission";
import { visibleChatgptModels } from "./chatgptModelCatalog";
import { officialCatalogBase } from "./officialModelCatalog";
import { bedrockMaterialDraft, validateBedrockMaterial, withoutBedrockSecrets } from "./bedrockMaterial";
import { dialogModelSelection, editModelSources, selectedModels, withModelEditors } from "./credentialModelSelection";
import { modelIdPolicy, modelIdLimit } from "./providerModelId";
import { normalizeQwenCredentialDraft, clearQwenEvidence, qwenCredentialBase } from "./qwenCredentialDraft";
import { acceptQwenCatalog, qwenCatalogInput } from "./qwenModelCatalog";
import type { CredentialDialogMode, CredentialDialogValue, CredentialDialogProps } from "./credentialDialogTypes";
import { normalizeVertexExpressDraft } from "./vertexExpress";
import { normalizeVertexOAuthDraft } from "./vertexOAuth";
import { normalizeLumaAgentsDraft } from "./lumaAgents";
import { normalizeLumaInferenceCredential } from "./lumaInferenceCredential";
import { normalizeSapCredential } from "./sapAiCore";

function emptyValue(providerId = ""): CredentialDialogValue {
  return {
    providerId,
    credentialId: "",
    accountName: "",
    enabled: true,
    baseUrl: "",
    supportedModelsText: "",
    fetchedModelsText: "",
    manualModelsText: "",
    apiKeyOperation: "replace",
    apiKeyValue: "",
  };
}

export function useCredentialDialog({
  open,
  mode,
  providerOptions,
  existingCredentialIds,
  initialValue,
  locked,
  hasSecretAccess,
  onOpenChange,
  onRequestSecretAccess,
  onSubmit,
  renderAuthentication,
  renderReauthentication,
  reauthenticationOnly,
  manualAuthentication,
  discoveryProvider,
  onDiscover,
  onDiscoverAccountModels,
  onDiscoverChatgptModels,
  onDiscoverProviderModels,
  catalogProviderKind,
  onRevealKey,
  supportsBasispoints,
  supportsWebSession,
  sessionProduct,
  bedrockProvider,
  qwenProvider,
  vertexExpressProvider,
  vertexOAuthProvider,
  lumaAgentsProvider,
  lumaInferenceProvider,
  sapProvider,
  managedWebProvider,
  modelPolicy: resolveModelPolicy,
  onCaptureWebBrowser,
}: CredentialDialogProps) {
  const { t } = useUiLocale();
  const [value, setValue] = useState<CredentialDialogValue>(() =>
    withModelEditors(initialValue ?? emptyValue(providerOptions[0]?.id ?? "")),
  );
  const [validationError, setValidationError] = useState<string | null>(null);
  const [authenticationMode, setAuthenticationMode] = useState("oauth");
  const [reauthorizing, setReauthorizing] = useState(false);
  const [authenticationActive, setAuthenticationActive] = useState(false);
  const [catalogRead, setCatalogRead] = useState(false);
  const initialized = useRef<{ mode: CredentialDialogMode; initialValue: CredentialDialogProps["initialValue"] } | null>(null);
  const qwen = qwenProvider?.(value.providerId);
  const requestFence = `${mode}:${value.providerId}:${value.credentialId}:${value.baseUrl}:${value.apiKeyOperation}:${value.apiKeyValue}:${qwen?.base_url}:${qwen?.auth_mode}:${qwen?.preset}`;
  const discovery = useDiscoverySubmission(open && hasSecretAccess, onDiscover, requestFence);
  const catalog = useDiscoverySubmission(open && hasSecretAccess, onDiscoverChatgptModels, `${requestFence}:${value.chatgptAccountId}:${authenticationMode}`);
  const official = useDiscoverySubmission(open && hasSecretAccess, onDiscoverProviderModels, requestFence);
  const customCatalog = useDiscoverySubmission(open && hasSecretAccess, onDiscoverAccountModels, requestFence);
  const busy = discovery.busy || catalog.busy || official.busy || customCatalog.busy;
  const webSession = Boolean(supportsWebSession?.(value.providerId));
  const productSession = sessionProduct?.(value.providerId);
  const bedrock = bedrockProvider?.(value.providerId);
  const express = vertexExpressProvider?.(value.providerId);
  const vertex = vertexOAuthProvider?.(value.providerId);
  const sap = sapProvider?.(value.providerId);
  const lumaAgents = lumaAgentsProvider?.(value.providerId);
  const bedrockMaterial = value.bedrockMaterial ?? (bedrock ? bedrockMaterialDraft(bedrock, {}, mode === "add") : undefined);
  const managedWeb = Boolean(managedWebProvider?.(value.providerId));
  const manual = manualAuthentication?.(value.providerId);
  const oauth = mode === "add" && !webSession && (!productSession || manual) && !bedrock ? renderAuthentication?.(value.providerId, setAuthenticationActive) : null;
  const reauthentication = mode === "edit" ? renderReauthentication?.(value.providerId, value.credentialId, setAuthenticationActive) : null;
  const localEdits = Boolean(initialValue && JSON.stringify(value) !== JSON.stringify(withModelEditors(initialValue)));
  const manualCodex = Boolean(oauth && supportsBasispoints?.(value.providerId) && authenticationMode === "token");
  // OAuth owns credential creation; never submit its result through the API-key form.
  const authentication = reauthorizing ? reauthentication : manualCodex || manual && authenticationMode === "token" ? null : oauth;
  useEffect(() => { setReauthorizing(false); }, [open, mode, value.providerId, value.credentialId]);
  useEffect(() => { setAuthenticationMode("oauth"); }, [open, value.providerId]);
  useEffect(() => { setCatalogRead(false); }, [open, value.providerId, value.apiKeyOperation, value.apiKeyValue]);

  useEffect(() => {
    if (!open) {
      initialized.current = null;
      setValue(emptyValue());
      return;
    }
    // Background option/callback refreshes are not a new editing session or permission revocation.
    if (initialized.current?.mode === mode && initialized.current.initialValue === initialValue) return;
    initialized.current = { mode, initialValue };
    setValue(withModelEditors(
      initialValue ??
      emptyValue(providerOptions.length === 1 ? (providerOptions[0]?.id ?? "") : ""),
    ));
    setValidationError(null);
  }, [initialValue, open, mode, providerOptions]);

  useEffect(() => {
    if (!hasSecretAccess) setValue((current) => {
      const next = {
        ...current, apiKeyValue: "", webSessionMaterial: undefined, sessionMaterial: undefined,
        bedrockMaterial: current.bedrockMaterial ? withoutBedrockSecrets(current.bedrockMaterial) : undefined,
        apiKeyOperation: mode === "edit" ? "keep" as const : "replace" as const
      };
      return qwen && current.apiKeyOperation !== "keep" ? clearQwenEvidence(next) : next;
    });
  }, [hasSecretAccess, mode]);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (authentication || (managedWeb && mode === "add")) return;
    if (reauthenticationOnly?.(value.providerId) && (mode === "add" || value.apiKeyOperation !== "keep")) return;
    if (locked || busy) return;
    let normalized = value;
    try { normalized = normalizeQwenCredentialDraft(qwen, value, mode === "edit" ? initialValue : null); }
    catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    if ((manualCodex || value.apiKeyOperation !== "keep") && !hasSecretAccess) { onRequestSecretAccess(); return; }
    try { normalized = normalizeVertexExpressDraft(express, normalized, mode === "edit"); }
    catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    try { normalized = normalizeVertexOAuthDraft(vertex, normalized, mode === "edit"); }
    catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    try { normalized = normalizeLumaAgentsDraft(lumaAgents, normalized, mode === "edit"); }
    catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    try { normalized = normalizeLumaInferenceCredential(lumaInferenceProvider?.(value.providerId), normalized); }
    catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    try { normalized = normalizeSapCredential(sap, normalized); }
    catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    if (bedrock && value.apiKeyOperation !== "keep") {
      try { if (value.apiKeyOperation !== "replace" || !bedrockMaterial) throw new Error("请重新填写完整 Bedrock 材料，或删除此账号；不能保存半套材料。"); validateBedrockMaterial(bedrockMaterial); }
      catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); return; }
    }
    if (webSession && mode === "add" && !value.webSessionMaterial) {
      setValidationError("请完成网页登录或导入完整的网页会话 JSON。"); return;
    }
    if (productSession && ((mode === "add" || value.apiKeyOperation !== "keep") && !value.sessionMaterial
      || value.sessionMaterial && value.sessionMaterial.product !== productSession)) {
      setValidationError("请导入与此产品匹配的完整会话材料。"); return;
    }
    if (manualCodex && (!value.chatgptAccountId?.trim() || /[\r\n]/.test(value.chatgptAccountId))) {
      setValidationError(t("请填写有效的 ChatGPT Account ID。", "Enter a valid ChatGPT Account ID."));
      return;
    }
    if (manualCodex && (value.apiKeyOperation !== "replace" || !value.apiKeyValue.trim())) {
      setValidationError(t("请填写 Access Token。", "Enter an access token."));
      return;
    }
    const providerId = value.providerId.trim();
    const credentialId = value.credentialId.trim();
    if (!providerId) {
      setValidationError(t("请选择 Provider。", "Select a provider."));
      return;
    }
    if (!credentialId) {
      setValidationError(t("账号 ID 不能为空。", "Account ID is required."));
      return;
    }
    if (
      mode === "add" &&
      existingCredentialIds.some((existing) => existing.trim() === credentialId)
    ) {
      setValidationError(
        t(
          `账号 ID ${credentialId} 已存在。`,
          `Account ID ${credentialId} already exists.`,
        ),
      );
      return;
    }
    if (value.apiKeyOperation === "replace" && !value.sessionMaterial && !bedrock && value.apiKeyValue.trim().length === 0) {
      setValidationError(
        t("替换 API Key 时必须填写新密钥。", "Enter a new API key when replacing it."),
      );
      return;
    }

    let supportedModelsText: string;
    const modelPolicy = resolveModelPolicy?.(value.providerId) ?? modelIdPolicy(express ?? vertex ?? bedrock, { discovery: value.discovery });
    try { supportedModelsText = selectedModels(dialogModelSelection(normalized, modelPolicy)!).join("\n"); }
    catch { setValidationError(express ? t("Vertex Express 模型 ID 无效：须为 1–2048 字节的单个资源段，不接受完整路径或 URL 编码。", "Invalid Vertex Express model ID: use a single 1–2048 byte resource segment, without full paths or URL encoding.")
      : t(`模型 ID 无效，单栏最多 2048 个模型，每个不超过 ${modelIdLimit(modelPolicy)} 字节；长 Bedrock ID 须符合原生语法。`, `Invalid model IDs: up to 2048 per editor, ${modelIdLimit(modelPolicy)} bytes each; extended Bedrock IDs require native syntax.`)); return; }
    const submission = {
      ...normalized,
      ...(bedrock ? { bedrockMaterial: value.apiKeyOperation === "replace" ? bedrockMaterial : undefined } : {}),
      supportedModelsText,
      providerId,
      credentialId,
      apiKeyValue: value.apiKeyValue.trim(),
      chatgptAccountId: manualCodex ? value.chatgptAccountId?.trim() : undefined,
      chatgptGroup: supportsBasispoints?.(providerId) ? value.chatgptGroup ?? "free" : undefined,
    };
    setValidationError(null);
    if (manualCodex && !submission.supportedModelsText && !catalogRead) {
      if (!onDiscoverChatgptModels) { setValidationError("模型目录服务不可用，请重新连接后重试。"); return; }
      void catalog.run({ accessToken: submission.apiKeyValue, accountId: submission.chatgptAccountId! }, (result) => {
        onSubmit(editModelSources(submission, visibleChatgptModels(result).join("\n"), submission.manualModelsText ?? ""));
        onOpenChange(false);
      });
      return;
    }
    const catalogKind = sap || bedrock || qwen || vertex || lumaAgents ? undefined : catalogProviderKind?.(providerId);
    if (catalogKind && value.apiKeyOperation !== "clear") {
      if (catalogKind !== "mistral" && !value.modelCatalog && !submission.supportedModelsText && mode === "add") {
        if (submission.baseUrl && !officialCatalogBase(catalogKind, submission.baseUrl)) { setValidationError("模型目录仅支持官方 API 地址。"); return; }
        if (!onDiscoverProviderModels) { setValidationError("模型目录服务不可用，请重新连接后重试。"); return; }
        void official.run({ providerId, apiKey: submission.apiKeyValue }, (modelCatalog) => {
          onSubmit(editModelSources({ ...submission, modelCatalog }, modelCatalog.models.join("\n"), submission.manualModelsText ?? "")); onOpenChange(false);
        });
        return;
      }
    }
    onSubmit(submission);
    onOpenChange(false);
  };

  const secretMutationRequested = value.apiKeyOperation !== "keep";
  const refreshProtocols = () => {
    if (sap || bedrock || qwen || vertex || lumaAgents) return;
    if (!hasSecretAccess) { onRequestSecretAccess(); return; }
    const target = discoveryProvider?.(value.providerId);
    if (!target || locked || busy) return;
    const addressChanged = value.baseUrl.trim() !== (initialValue?.baseUrl ?? "").trim();
    const input = value.apiKeyValue.trim()
      ? { baseUrl: value.baseUrl.trim() || target.baseUrl, apiKey: value.apiKeyValue.trim() }
      : mode === "edit" && value.apiKeyOperation === "keep" && !addressChanged
        ? { credentialId: value.credentialId } : null;
    if (!input) { setValidationError(t("请填写 API Key。", "Enter an API key.")); return; }
    setValidationError(null);
    let models: string[];
    try { models = selectedModels(dialogModelSelection(value, "standard")!); }
    catch { setValidationError(t("模型列表无效。", "Invalid model list.")); return; }
    void discovery.run({ ...input, ...(models.length ? { models } : {}) }, (result) => setValue((current) => editModelSources({
      ...current,
      discovery: result, discoveryStatus: undefined
    }, result.models.join("\n"), current.manualModelsText ?? "")));
  };

  const requestedCatalogKind = catalogProviderKind?.(value.providerId);
  const catalogKind = sap || bedrock || vertex || lumaAgents || qwen && requestedCatalogKind !== "qwen" ? undefined : requestedCatalogKind;
  const chatgpt = webSession || Boolean(supportsBasispoints?.(value.providerId));
  const customTarget = discoveryProvider?.(value.providerId);
  const canFetch = !sap && !bedrock && Boolean((customTarget && onDiscoverAccountModels) || (catalogKind && onDiscoverProviderModels) || (chatgpt && onDiscoverChatgptModels
    && ((mode === "edit" && value.apiKeyOperation === "keep") || manualCodex)));
  const fetchModels = () => {
    if (sap || lumaAgents || locked || busy || value.apiKeyOperation === "clear") return;
    if (!hasSecretAccess) { onRequestSecretAccess(); return; }
    setValidationError(null);
    const stored = mode === "edit" && value.apiKeyOperation === "keep";
    if (!stored && !value.apiKeyValue.trim()) { setValidationError(t("请填写 API Key。", "Enter an API key.")); return; }
    if (customTarget && onDiscoverAccountModels) {
      const addressChanged = value.baseUrl.trim() !== (initialValue?.baseUrl ?? "").trim();
      if (stored && addressChanged) { setValidationError(t("地址已修改，请先保存或重新填写 API Key。", "Save the new address or enter the API key again.")); return; }
      void customCatalog.run(stored ? { credentialId: value.credentialId }
        : { baseUrl: value.baseUrl.trim() || customTarget.baseUrl, apiKey: value.apiKeyValue.trim() },
        (result) => setValue((current) => editModelSources(current, result.models.join("\n"), current.manualModelsText ?? "")));
    } else if (chatgpt && onDiscoverChatgptModels) {
      if (!stored && !value.chatgptAccountId?.trim()) { setValidationError("请填写 ChatGPT Account ID。"); return; }
      void catalog.run(stored ? { credentialId: value.credentialId }
        : { accessToken: value.apiKeyValue.trim(), accountId: value.chatgptAccountId!.trim() },
        (result) => { setCatalogRead(true); setValue((current) => editModelSources(current, visibleChatgptModels(result).join("\n"), current.manualModelsText ?? "")); });
    } else if (catalogKind && onDiscoverProviderModels) {
      if (qwen) {
        try {
          const base = qwenCredentialBase(qwen, value);
          const previousBase = mode === "edit" && initialValue ? qwenCredentialBase(qwen, initialValue) : undefined;
          const input = qwenCatalogInput(value, base, previousBase);
          void official.run(input, (result) => {
            const modelCatalog = acceptQwenCatalog(qwen, value, base, result);
            setValue((current) => editModelSources({ ...current, modelCatalog }, modelCatalog.models.join("\n"), current.manualModelsText ?? ""));
          });
        } catch (error) { setValidationError(error instanceof Error ? error.message : String(error)); }
        return;
      }
      if (value.baseUrl && !officialCatalogBase(catalogKind, value.baseUrl)) { setValidationError("模型目录仅支持官方 API 地址。"); return; }
      void official.run(stored ? { credentialId: value.credentialId } : { providerId: value.providerId, apiKey: value.apiKeyValue.trim() },
        (modelCatalog) => setValue((current) => editModelSources({ ...current, modelCatalog }, modelCatalog.models.join("\n"), current.manualModelsText ?? "")));
    }
  };

  return {
    value, setValue, validationError, setValidationError, authenticationMode, setAuthenticationMode,
    reauthorizing, setReauthorizing, authenticationActive, qwen, discovery, catalog, official, customCatalog, busy, webSession,
    productSession, bedrock, bedrockMaterial, express, vertex, lumaAgents, sap, managedWeb, oauth, reauthentication, localEdits, manualCodex,
    manual, authentication, submit, secretMutationRequested, refreshProtocols, canFetch, fetchModels
  };
}
export type CredentialDialogState = ReturnType<typeof useCredentialDialog>;
