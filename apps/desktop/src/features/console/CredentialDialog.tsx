import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type CredentialDialogMode = "add" | "edit";
export type CredentialApiKeyOperation = "keep" | "replace" | "clear";

export type CredentialDialogValue = {
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
}: CredentialDialogProps) {
  const { t } = useUiLocale();
  const [value, setValue] = useState<CredentialDialogValue>(() =>
    initialValue ?? emptyValue(providerOptions[0]?.id ?? ""),
  );
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setValue(
      initialValue ??
        emptyValue(providerOptions.length === 1 ? (providerOptions[0]?.id ?? "") : ""),
    );
    setValidationError(null);
  }, [initialValue, open, providerOptions]);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
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

    onSubmit({
      ...value,
      providerId,
      credentialId,
      accountName: value.accountName.trim(),
      baseUrl: value.baseUrl.trim(),
      apiKeyValue: value.apiKeyValue.trim(),
    });
    onOpenChange(false);
  };

  const secretMutationRequested = value.apiKeyOperation !== "keep";

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-credential-dialog"
          aria-describedby="credential-dialog-description"
        >
          <Dialog.Title>
            {mode === "add" ? t("新增账号", "Add account") : t("编辑账号", "Edit account")}
          </Dialog.Title>
          <Dialog.Description id="credential-dialog-description">
            {t(
              "账号会作为 route-config 中的显式 credential 写入草稿，最终通过修订事务统一保存。",
              "The account is written to the route-config draft as an explicit credential and saved through the revision transaction.",
            )}
          </Dialog.Description>

          {mode === "add" ? renderAuthentication?.(value.providerId) : null}
          <form className="nt-stack" onSubmit={submit}>
            <div className="nt-grid nt-grid--2">
              <label className="nt-field">
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

              <label className="nt-field">
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

              <label className="nt-field">
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

              <label className="nt-chip nt-field--toggle">
                <input
                  checked={value.enabled}
                  disabled={locked}
                  aria-label={t("账号启用状态", "Account enabled state")}
                  type="checkbox"
                  onChange={(event) => {
                    const enabled = event.currentTarget.checked;
                    setValue((current) => ({
                      ...current,
                      enabled,
                    }));
                  }}
                />
                <span>{value.enabled ? t("账号已启用", "Account enabled") : t("账号已停用", "Account disabled")}</span>
              </label>

              <label className="nt-field">
                <span>{t("账号覆盖地址", "Account base URL override")}</span>
                <input
                  className="nt-input"
                  type="url"
                  value={value.baseUrl}
                  disabled={locked}
                  placeholder="https://api.example.com/v1"
                  onChange={(event) => {
                    const baseUrl = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      baseUrl,
                    }));
                  }}
                />
              </label>

              <label className="nt-field nt-field--wide">
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
              </label>

              <label className="nt-field">
                <span>{t("API Key 操作", "API key operation")}</span>
                <select
                  className="nt-select"
                  value={value.apiKeyOperation}
                  disabled={locked}
                  onChange={(event) => {
                    const apiKeyOperation = event.currentTarget.value as CredentialApiKeyOperation;
                    setValue((current) => ({
                      ...current,
                      apiKeyOperation,
                      apiKeyValue: apiKeyOperation === "replace" ? current.apiKeyValue : "",
                    }));
                  }}
                >
                  <option value="keep">
                    {mode === "add"
                      ? t("继承 Provider 密钥 / 不单独设置", "Inherit provider key / leave unset")
                      : t("保留当前 API Key", "Keep current API key")}
                  </option>
                  <option value="replace">{t("替换 API Key", "Replace API key")}</option>
                  {mode === "edit" ? (
                    <option value="clear">{t("清空 API Key", "Clear API key")}</option>
                  ) : null}
                </select>
              </label>

              <label className="nt-field">
                <span>API Key</span>
                <input
                  className="nt-input"
                  type="password"
                  autoComplete="new-password"
                  value={value.apiKeyValue}
                  disabled={locked || value.apiKeyOperation !== "replace"}
                  onChange={(event) => {
                    const apiKeyValue = event.currentTarget.value;
                    setValue((current) => ({
                      ...current,
                      apiKeyValue,
                    }));
                  }}
                />
              </label>
            </div>

            {secretMutationRequested && !hasSecretAccess ? (
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>{t("需要敏感信息访问权限", "Secret access is required")}</strong>
                <ul>
                  <li>
                    {t(
                      "替换或清空 API Key 前，请先确认管理员敏感信息访问权限。",
                      "Confirm administrator secret access before replacing or clearing an API key.",
                    )}
                  </li>
                </ul>
                <button className="nt-btn nt-btn--outline" type="button" onClick={onRequestSecretAccess}>
                  {t("确认敏感信息访问权限", "Confirm secret access")}
                </button>
              </div>
            ) : null}

            {validationError ? (
              <div className="nt-validation-list" role="alert">
                <strong>{validationError}</strong>
              </div>
            ) : null}

            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button className="nt-btn nt-btn--outline" type="button" disabled={locked}>
                  {t("取消", "Cancel")}
                </button>
              </Dialog.Close>
              <button className="nt-btn nt-btn--primary" type="submit" disabled={locked}>
                {t("保存到草稿", "Save to draft")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
