import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { AccountDiscovery, DiscoverAccount } from "./accountDiscovery";
import { useDiscoverySubmission } from "./useDiscoverySubmission";
import { visibleChatgptModels, type DiscoverChatgptModels } from "./chatgptModelCatalog";
import { type DiscoverProviderModels, type OfficialModelCatalog, type CatalogProviderKind, officialCatalogBase } from "./officialModelCatalog";
import { CredentialSecretField } from "./CredentialSecretField";
import { ChatgptWebSessionImport } from "./ChatgptWebSessionImport";
import type { ChatgptWebMaterial } from "./chatgptWebMaterial";
import type { WebBrowserCapture } from "./ChatgptWebBrowserImport";
import { CredentialProtocolList } from "./CredentialProtocolList";
import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import { CredentialModelEditors } from "./CredentialModelEditors";
import { dialogModelSelection, editModelSources, selectedModels, withModelEditors } from "./credentialModelSelection";
import "./CredentialDialog.css";

export type CredentialDialogMode = "add" | "edit";
export type CredentialApiKeyOperation = "keep" | "replace" | "clear";

export type CredentialDialogValue = {
  webSessionMaterial?: ChatgptWebMaterial;
  chatgptAccountId?: string;
  chatgptGroup?: string;
  codexBasispointsEnabled?: boolean;
  discoveryStatus?: string;
  discovery?: AccountDiscovery;
  modelCatalog?: OfficialModelCatalog;
  providerId: string;
  credentialId: string;
  accountName: string;
  enabled: boolean;
  baseUrl: string;
  supportedModelsText: string;
  fetchedModelsText?: string;
  manualModelsText?: string;
  apiKeyOperation: CredentialApiKeyOperation;
  apiKeyValue: string;
};

export type CredentialDialogProviderOption = {
  id: string;
  label: string;
};

export type CredentialDialogProps = {
  onCaptureWebBrowser?: WebBrowserCapture;
  supportsWebSession?: (providerId: string) => boolean;
  supportsBasispoints?: (providerId: string) => boolean;
  open: boolean;
  mode: CredentialDialogMode;
  providerOptions: CredentialDialogProviderOption[];
  existingCredentialIds: string[];
  initialValue?: CredentialDialogValue | null;
  locked: boolean;
  hasSecretAccess: boolean;
  onOpenChange(open: boolean): void;
  onRequestSecretAccess(): void;
  onSubmit(value: CredentialDialogValue): void;
  renderAuthentication?: (providerId: string) => ReactNode;
  discoveryProvider?: (providerId: string) => { baseUrl: string } | null;
  onDiscover?: DiscoverAccount;
  onDiscoverChatgptModels?: DiscoverChatgptModels;
  onDiscoverProviderModels?: DiscoverProviderModels;
  catalogProviderKind?: (providerId: string) => CatalogProviderKind | undefined;
  onRevealKey?: (signal: AbortSignal) => Promise<string>;
};

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

export function CredentialDialog({
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
  discoveryProvider,
  onDiscover,
  onDiscoverChatgptModels,
  onDiscoverProviderModels,
  catalogProviderKind,
  onRevealKey,
  supportsBasispoints,
  supportsWebSession,
  onCaptureWebBrowser,
}: CredentialDialogProps) {
  const { t } = useUiLocale();
  const [value, setValue] = useState<CredentialDialogValue>(() =>
    withModelEditors(initialValue ?? emptyValue(providerOptions[0]?.id ?? "")),
  );
  const [validationError, setValidationError] = useState<string | null>(null);
  const [authenticationMode, setAuthenticationMode] = useState("oauth");
  const [catalogRead, setCatalogRead] = useState(false);
  const initialized = useRef<{ mode: CredentialDialogMode; initialValue: CredentialDialogProps["initialValue"] } | null>(null);
  const requestFence = `${mode}:${value.providerId}:${value.credentialId}:${value.baseUrl}:${value.apiKeyOperation}:${value.apiKeyValue}`;
  const discovery = useDiscoverySubmission(open && hasSecretAccess, onDiscover, requestFence);
  const catalog = useDiscoverySubmission(open && hasSecretAccess, onDiscoverChatgptModels, `${requestFence}:${value.chatgptAccountId}:${authenticationMode}`);
  const official = useDiscoverySubmission(open && hasSecretAccess, onDiscoverProviderModels, requestFence);
  const busy = discovery.busy || catalog.busy || official.busy;
  const webSession = Boolean(supportsWebSession?.(value.providerId));
  const oauth = mode === "add" && !webSession ? renderAuthentication?.(value.providerId) : null;
  const manualCodex = Boolean(oauth && supportsBasispoints?.(value.providerId) && authenticationMode === "token");
  // OAuth owns credential creation; never submit its result through the API-key form.
  const authentication = manualCodex ? null : oauth;
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
    if (!hasSecretAccess) setValue((current) => ({ ...current, apiKeyValue: "", webSessionMaterial: undefined,
      apiKeyOperation: mode === "edit" ? "keep" : "replace" }));
  }, [hasSecretAccess, mode]);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (authentication) return;
    if (locked || busy) return;
    if ((manualCodex || value.apiKeyOperation !== "keep") && !hasSecretAccess) { onRequestSecretAccess(); return; }
    if (webSession && mode === "add" && !value.webSessionMaterial) {
      setValidationError("请完成网页登录或导入完整的网页会话 JSON。"); return;
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
    if (value.apiKeyOperation === "replace" && value.apiKeyValue.trim().length === 0) {
      setValidationError(
        t("替换 API Key 时必须填写新密钥。", "Enter a new API key when replacing it."),
      );
      return;
    }

    let supportedModelsText: string;
    try { supportedModelsText = selectedModels(dialogModelSelection(value)!).join("\n"); }
    catch { setValidationError(t("模型 ID 无效，单栏最多 2048 个模型，每个不超过 256 字节，不能包含控制字符或通配符。", "Invalid model IDs: up to 2048 per editor, 256 bytes each; no control characters or wildcards.")); return; }
    const submission = {
      ...value,
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
    const catalogKind = catalogProviderKind?.(providerId);
    if (catalogKind && value.apiKeyOperation !== "clear") {
      if (!value.modelCatalog && !submission.supportedModelsText && mode === "add") {
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
    if (!hasSecretAccess) { onRequestSecretAccess(); return; }
    const target = discoveryProvider?.(value.providerId);
    if (!target || locked || discovery.busy) return;
    const addressChanged = value.baseUrl.trim() !== (initialValue?.baseUrl ?? "").trim();
    const input = value.apiKeyValue.trim()
      ? { baseUrl: value.baseUrl.trim() || target.baseUrl, apiKey: value.apiKeyValue.trim() }
      : mode === "edit" && value.apiKeyOperation === "keep" && !addressChanged
        ? { credentialId: value.credentialId } : null;
    if (!input) { setValidationError(t("请填写 API Key。", "Enter an API key.")); return; }
    setValidationError(null);
    void discovery.run(input, (result) => setValue((current) => editModelSources({ ...current,
      discovery: result, discoveryStatus: undefined }, result.models.join("\n"), current.manualModelsText ?? "")));
  };

  const catalogKind = catalogProviderKind?.(value.providerId);
  const chatgpt = webSession || Boolean(supportsBasispoints?.(value.providerId));
  const canFetch = Boolean((catalogKind && onDiscoverProviderModels) || (chatgpt && onDiscoverChatgptModels
    && ((mode === "edit" && value.apiKeyOperation === "keep") || manualCodex)));
  const fetchModels = () => {
    if (locked || busy || value.apiKeyOperation === "clear") return;
    if (!hasSecretAccess) { onRequestSecretAccess(); return; }
    setValidationError(null);
    const stored = mode === "edit" && value.apiKeyOperation === "keep";
    if (!stored && !value.apiKeyValue.trim()) { setValidationError(t("请填写 API Key。", "Enter an API key.")); return; }
    if (chatgpt && onDiscoverChatgptModels) {
      if (!stored && !value.chatgptAccountId?.trim()) { setValidationError("请填写 ChatGPT Account ID。"); return; }
      void catalog.run(stored ? { credentialId: value.credentialId }
        : { accessToken: value.apiKeyValue.trim(), accountId: value.chatgptAccountId!.trim() },
      (result) => { setCatalogRead(true); setValue((current) => editModelSources(current, visibleChatgptModels(result).join("\n"), current.manualModelsText ?? "")); });
    } else if (catalogKind && onDiscoverProviderModels) {
      if (value.baseUrl && !officialCatalogBase(catalogKind, value.baseUrl)) { setValidationError("模型目录仅支持官方 API 地址。"); return; }
      void official.run(stored ? { credentialId: value.credentialId } : { providerId: value.providerId, apiKey: value.apiKeyValue.trim() },
        (modelCatalog) => setValue((current) => editModelSources({ ...current, modelCatalog }, modelCatalog.models.join("\n"), current.manualModelsText ?? "")));
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-credential-dialog"
          aria-describedby={undefined}
        >
          <Dialog.Title>
            {mode === "add" ? t("新增账号", "Add account") : t("编辑账号", "Edit account")}
          </Dialog.Title>

          {oauth && supportsBasispoints?.(value.providerId) && <label className="nt-credential-row">
            <span>{t("录入方式", "Import method")}</span>
            <select className="nt-select" value={authenticationMode} disabled={locked} onChange={(event) => {
              setAuthenticationMode(event.target.value); setValidationError(null);
              setValue((current) => ({ ...current, apiKeyValue: "", chatgptAccountId: "", apiKeyOperation: "replace" }));
            }}>
              <option value="oauth">{t("ChatGPT 登录", "ChatGPT sign-in")}</option>
              <option value="token">{t("现有 Access Token", "Existing access token")}</option>
            </select>
          </label>}
          {authentication}
          <form className="nt-stack" onSubmit={submit}>
            <fieldset className="nt-credential-fields" disabled={locked || busy}>
              <label className="nt-credential-row">
                <span>Provider</span>
                <select
                  className="nt-select"
                  value={value.providerId}
                  disabled={locked || mode === "edit"}
                  onChange={(event) => {
                    const providerId = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      providerId,
                      accountName: "", baseUrl: "",
                      apiKeyValue: "",
                      webSessionMaterial: undefined,
                      chatgptAccountId: "",
                      chatgptGroup: undefined,
                      apiKeyOperation: "replace",
                      discovery: undefined,
                      modelCatalog: undefined,
                      supportedModelsText: "", fetchedModelsText: "", manualModelsText: "",
                    }));
                  }}
                >
                  <option value="">{t("请选择 Provider", "Select provider")}</option>
                  {providerOptions.map((provider) => (
                    <option value={provider.id} key={provider.id}>
                      {provider.label}
                    </option>
                  ))}
                </select>
              </label>

              {!authentication && <>
              {supportsBasispoints?.(value.providerId) && <label className="nt-credential-row">
                <span>{t("订阅分组", "Subscription group")}</span>
                <select className="nt-select" value={value.chatgptGroup ?? "free"}
                  onChange={(event) => setValue((current) => ({ ...current, chatgptGroup: event.target.value }))}>
                  {CHATGPT_POOL_CATEGORIES.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}
                </select>
              </label>}
              {manualCodex && <label className="nt-credential-row"><span>ChatGPT Account ID</span>
                <input className="nt-input" autoComplete="off" value={value.chatgptAccountId ?? ""}
                  onChange={(event) => setValue((current) => ({ ...current, chatgptAccountId: event.target.value }))} />
              </label>}
              {supportsBasispoints?.(value.providerId) && <label className="nt-credential-row">
                <span>{t("Codex 调用通道", "Codex channel")}</span>
                <select className="nt-select" value={value.codexBasispointsEnabled === undefined ? "inherit" : value.codexBasispointsEnabled ? "basispoints" : "native"}
                  onChange={(e) => setValue((current) => ({ ...current, codexBasispointsEnabled: e.target.value === "inherit" ? undefined : e.target.value === "basispoints" }))}>
                  <option value="inherit">{t("继承服务商设置", "Inherit provider setting")}</option>
                  <option value="native">{t("原生 Codex", "Native Codex")}</option>
                  <option value="basispoints">Basispoints</option>
                </select>
              </label>}

              <label className="nt-credential-row">
                <span>{t("账号 ID", "Account ID")}</span>
                <input
                  className="nt-input"
                  value={value.credentialId}
                  disabled={locked || mode === "edit"}
                  onChange={(event) => {
                    const credentialId = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      credentialId,
                    }));
                  }}
                />
              </label>

              <CredentialModelEditors value={value} disabled={locked || busy} canFetch={canFetch && value.apiKeyOperation !== "clear"}
                busy={catalog.busy || official.busy} onFetch={fetchModels} onChange={setValue} />

              {open && webSession && <ChatgptWebSessionImport key={`web:${value.providerId}:${initialValue?.credentialId ?? ""}`}
                capture={onCaptureWebBrowser} providerId={value.providerId} disabled={locked || busy || !hasSecretAccess}
                loaded={Boolean(value.webSessionMaterial)}
                onClear={() => setValue((current) => ({ ...current, webSessionMaterial: undefined, apiKeyValue: "",
                  apiKeyOperation: mode === "edit" ? "keep" : "replace" }))}
                onMaterial={(webSessionMaterial, accountName) => setValue((current) => {
                  const imported = { ...current, webSessionMaterial, accountName: current.accountName || accountName || "",
                    apiKeyOperation: "replace" as const, apiKeyValue: webSessionMaterial.accessToken };
                  return webSessionMaterial.modelSlugs?.length
                    ? editModelSources(imported, webSessionMaterial.modelSlugs.join("\n"), current.manualModelsText ?? "") : imported;
                })} />}
              {open && !webSession && <CredentialSecretField key={`${value.providerId}:${value.credentialId}`}
                value={value.apiKeyValue} existing={mode === "edit" && value.apiKeyOperation === "keep"}
                disabled={locked || busy} hasSecretAccess={hasSecretAccess}
                onRequestSecretAccess={onRequestSecretAccess} onReveal={onRevealKey}
                onLoaded={(apiKeyValue) => setValue((current) => ({ ...current, apiKeyValue }))}
                onChange={(apiKeyValue) => setValue((current) => ({ ...current, apiKeyValue,
                  apiKeyOperation: apiKeyValue ? "replace" : mode === "edit" ? "clear" : "keep", discovery: undefined, modelCatalog: undefined }))} />}

              </>}
            </fieldset>
            {!authentication && secretMutationRequested && !hasSecretAccess ? (
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>{t("需要敏感信息访问权限", "Secret access is required")}</strong>
                <button className="nt-btn nt-btn--outline" type="button" onClick={onRequestSecretAccess}>
                  {t("确认敏感信息访问权限", "Confirm secret access")}
                </button>
              </div>
            ) : null}

            {!authentication && discoveryProvider?.(value.providerId) && onDiscover && <div className="nt-credential-protocols">
              {value.discoveryStatus === "pending" && <span role="status">{t("后台识别中", "Discovering in background")}</span>}
              {value.discoveryStatus === "unconfirmed" && <span role="status">{t("尚未确认可用协议，可重新刷新", "Protocols unconfirmed; refresh to retry")}</span>}
              <button className="nt-btn nt-btn--outline" type="button" disabled={locked || discovery.busy}
                onClick={refreshProtocols}>{discovery.busy ? t("刷新中…", "Refreshing…") : t("刷新协议", "Refresh protocols")}</button>
              <CredentialProtocolList discovery={value.discovery} />
            </div>}

            {validationError || discovery.error || catalog.error || official.error ? (
              <div className="nt-validation-list" role="alert">
                <strong>{validationError ?? discovery.error ?? catalog.error ?? official.error}</strong>
              </div>
            ) : null}

            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button className="nt-btn nt-btn--outline" type="button">
                  {t("取消", "Cancel")}
                </button>
              </Dialog.Close>
              {!authentication && <button className="nt-btn nt-btn--primary" type="submit" disabled={locked || busy}>
                {busy ? t("正在识别…", "Discovering…") : t("保存", "Save")}
              </button>}
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
