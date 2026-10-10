import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { AccountDiscovery, DiscoverAccount } from "./accountDiscovery";
import { useDiscoverySubmission } from "./useDiscoverySubmission";
import { CredentialSecretField } from "./CredentialSecretField";
import { CredentialProtocolList } from "./CredentialProtocolList";
import "./CredentialDialog.css";

export type CredentialDialogMode = "add" | "edit";
export type CredentialApiKeyOperation = "keep" | "replace" | "clear";

export type CredentialDialogValue = {
  discoveryStatus?: string;
  discovery?: AccountDiscovery;
  providerId: string;
  credentialId: string;
  accountName: string;
  enabled: boolean;
  baseUrl: string;
  supportedModelsText: string;
  apiKeyOperation: CredentialApiKeyOperation;
  apiKeyValue: string;
};

export type CredentialDialogProviderOption = {
  id: string;
  label: string;
};

export type CredentialDialogProps = {
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
  onRevealKey,
}: CredentialDialogProps) {
  const discovery = useDiscoverySubmission(open, onDiscover);
  const { t } = useUiLocale();
  const [value, setValue] = useState<CredentialDialogValue>(() =>
    initialValue ?? emptyValue(providerOptions[0]?.id ?? ""),
  );
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setValue(emptyValue());
      return;
    }
    setValue(
      initialValue ??
        emptyValue(providerOptions.length === 1 ? (providerOptions[0]?.id ?? "") : ""),
    );
    setValidationError(null);
  }, [initialValue, open, providerOptions, onRevealKey]);

  useEffect(() => {
    if (!hasSecretAccess) setValue((current) => ({ ...current, apiKeyValue: "",
      apiKeyOperation: mode === "edit" ? "keep" : "replace" }));
  }, [hasSecretAccess, mode]);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (locked || discovery.busy) return;
    if (value.apiKeyOperation !== "keep" && !hasSecretAccess) { onRequestSecretAccess(); return; }
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

    const submission = {
      ...value,
      providerId,
      credentialId,
      accountName: value.accountName.trim(),
      baseUrl: value.baseUrl.trim(),
      apiKeyValue: value.apiKeyValue.trim(),
    };
    const target = discoveryProvider?.(providerId);
    if (target && onDiscover && value.apiKeyOperation !== "clear") {
      const addressChanged = value.baseUrl.trim() !== (initialValue?.baseUrl ?? "").trim();
      if (value.apiKeyOperation !== "replace" && !value.discovery && (mode === "add" || addressChanged)) {
        setValidationError("新增账号或更改地址时，请填写 API Key 以重新识别。");
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
    void discovery.run(input, (result) => setValue((current) => ({ ...current,
      discovery: result, discoveryStatus: undefined, supportedModelsText: result.models.join("\n") })));
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

          {mode === "add" ? renderAuthentication?.(value.providerId) : null}
          <form className="nt-stack" onSubmit={submit}>
            <fieldset className="nt-credential-fields" disabled={locked || discovery.busy}>
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
                      discovery: undefined,
                    }));
                  }}
                >
                  <option value="">{t("请选择 Provider", "Select provider")}</option>
                  {providerOptions.map((provider) => (
                    <option value={provider.id} key={provider.id}>
                      {provider.label} · {provider.id}
                    </option>
                  ))}
                </select>
              </label>

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

              <label className="nt-credential-row">
                <span>{t("账号名称", "Account name")}</span>
                <input
                  className="nt-input"
                  value={value.accountName}
                  disabled={locked}
                  onChange={(event) => {
                    const accountName = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      accountName,
                    }));
                  }}
                />
              </label>


              <label className="nt-credential-row">
                <span>{t("账号覆盖地址", "Account base URL override")}</span>
                <input
                  className="nt-input"
                  type="url"
                  value={value.baseUrl}
                  disabled={locked}
                  placeholder="https://api.example.com"
                  onChange={(event) => {
                    const baseUrl = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      baseUrl,
                      discovery: undefined,
                    }));
                  }}
                />
              </label>

              {!discoveryProvider?.(value.providerId) && <label className="nt-credential-row">
                <span>{t("支持模型", "Supported models")}</span>
                <textarea
                  className="nt-textarea"
                  rows={3}
                  value={value.supportedModelsText}
                  disabled={locked}
                  placeholder={t("每行一个模型，也支持逗号分隔", "One model per line; commas are also accepted")}
                  onChange={(event) => {
                    const supportedModelsText = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      supportedModelsText,
                    }));
                  }}
                />
              </label>}

              {open && <CredentialSecretField key={`${value.providerId}:${value.credentialId}`}
                value={value.apiKeyValue} existing={mode === "edit" && value.apiKeyOperation === "keep"}
                disabled={locked || discovery.busy} hasSecretAccess={hasSecretAccess}
                onRequestSecretAccess={onRequestSecretAccess} onReveal={onRevealKey}
                onLoaded={(apiKeyValue) => setValue((current) => ({ ...current, apiKeyValue }))}
                onChange={(apiKeyValue) => setValue((current) => ({ ...current, apiKeyValue,
                  apiKeyOperation: apiKeyValue ? "replace" : mode === "edit" ? "clear" : "keep", discovery: undefined }))} />}

            </fieldset>
            {secretMutationRequested && !hasSecretAccess ? (
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>{t("需要敏感信息访问权限", "Secret access is required")}</strong>
                <button className="nt-btn nt-btn--outline" type="button" onClick={onRequestSecretAccess}>
                  {t("确认敏感信息访问权限", "Confirm secret access")}
                </button>
              </div>
            ) : null}

            {discoveryProvider?.(value.providerId) && onDiscover && <div className="nt-credential-protocols">
              {value.discoveryStatus === "pending" && <span role="status">{t("后台识别中", "Discovering in background")}</span>}
              {value.discoveryStatus === "unconfirmed" && <span role="status">{t("尚未确认可用协议，可重新刷新", "Protocols unconfirmed; refresh to retry")}</span>}
              <button className="nt-btn nt-btn--outline" type="button" disabled={locked || discovery.busy}
                onClick={refreshProtocols}>{discovery.busy ? t("刷新中…", "Refreshing…") : t("刷新协议", "Refresh protocols")}</button>
              <CredentialProtocolList discovery={value.discovery} />
            </div>}

            {validationError || discovery.error ? (
              <div className="nt-validation-list" role="alert">
                <strong>{validationError ?? discovery.error}</strong>
              </div>
            ) : null}

            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button className="nt-btn nt-btn--outline" type="button">
                  {t("取消", "Cancel")}
                </button>
              </Dialog.Close>
              <button className="nt-btn nt-btn--primary" type="submit" disabled={locked || discovery.busy}>
                {discovery.busy ? t("正在识别…", "Discovering…") : t("保存", "Save")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
