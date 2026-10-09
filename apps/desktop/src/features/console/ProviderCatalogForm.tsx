import * as Dialog from "@radix-ui/react-dialog";
import type { Dispatch, SetStateAction } from "react";
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";

type TranslateFn = (zh: string, en: string) => string;

export type ProviderCatalogFormProps = {
  t: TranslateFn;
  selectedTemplate: ProviderCatalogTemplate;
  draft: ProviderCatalogDraft;
  setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>;
  supportedModelsText: string;
  setSupportedModelsText: Dispatch<SetStateAction<string>>;
  locked: boolean;
  autoDiscovery?: boolean;
  hasSecretAccess: boolean;
  validationError: string | null;
  onRequestSecretAccess(): void;
  suggestedCredentialId(providerId: string): string;
};

export function ProviderCatalogForm({
  t,
  selectedTemplate,
  draft,
  setDraft,
  supportedModelsText,
  setSupportedModelsText,
  locked,
  autoDiscovery,
  hasSecretAccess,
  validationError,
  onRequestSecretAccess,
  suggestedCredentialId,
}: ProviderCatalogFormProps) {
  return (
            <section className="nt-provider-catalog__form">
              <div className="nt-provider-catalog__summary">
                <div>
                  <strong>{t(selectedTemplate.labelZh, selectedTemplate.labelEn)}</strong>
                  <p>{t(selectedTemplate.descriptionZh, selectedTemplate.descriptionEn)}</p>
                </div>
                <span className="nt-badge nt-badge--info">
                  {autoDiscovery ? t("自动识别", "Automatic") : selectedTemplate.compatibility === "openai"
                    ? "OpenAI-compatible"
                    : selectedTemplate.compatibility === "anthropic"
                      ? "Anthropic Messages"
                      : selectedTemplate.compatibility === "search"
                        ? t("搜索协议", "Search protocol")
                        : t("原生协议", "Native protocol")}
                </span>
              </div>

                  <div className="nt-grid nt-grid--2">
                    <label className="nt-field">
                      <span>Provider ID</span>
                      <input
                        className="nt-input"
                        value={draft.providerId}
                        disabled={locked}
                        onChange={(event) => {
                          const providerId = event.currentTarget.value;
                          setDraft((current) => ({
                            ...current,
                            providerId,
                            credentialId:
                              current.credentialId === suggestedCredentialId(current.providerId)
                                ? suggestedCredentialId(providerId)
                                : current.credentialId,
                          }));
                        }}
                      />
                    </label>
                    <label className="nt-field">
                      <span>{t("显示名称", "Display label")}</span>
                      <input
                        className="nt-input"
                        value={draft.providerLabel}
                        disabled={locked}
                        onChange={(event) => {
                          const providerLabel = event.currentTarget.value;
                          setDraft((current) => ({ ...current, providerLabel }));
                        }}
                      />
                    </label>
                    <label className="nt-field">
                      <span>{t("服务商标识", "Vendor key")}</span>
                      <input
                        className="nt-input"
                        value={draft.vendorKey}
                        disabled={locked}
                        onChange={(event) => {
                          const vendorKey = event.currentTarget.value;
                          setDraft((current) => ({ ...current, vendorKey }));
                        }}
                      />
                    </label>
                    <label className="nt-field">
                      <span>{t("服务商名称", "Vendor name")}</span>
                      <input
                        className="nt-input"
                        value={draft.vendorName}
                        disabled={locked}
                        onChange={(event) => {
                          const vendorName = event.currentTarget.value;
                          setDraft((current) => ({ ...current, vendorName }));
                        }}
                      />
                    </label>
                    <label className="nt-field nt-field--wide">
                      <span>Base URL</span>
                      <input
                        className="nt-input"
                        type="url"
                        value={draft.baseUrl}
                        disabled={locked}
                        placeholder="https://api.example.com"
                        onChange={(event) => {
                          const baseUrl = event.currentTarget.value;
                          setDraft((current) => ({ ...current, baseUrl }));
                        }}
                      />
                    </label>
                    {!autoDiscovery && <label className="nt-field nt-field--wide">
                      <span>{t("支持模型与聚合路由", "Supported models and aggregation routes")}</span>
                      <textarea
                        className="nt-textarea nt-provider-catalog__models"
                        rows={4}
                        value={supportedModelsText}
                        disabled={locked}
                        placeholder={t("每行一个模型", "One model per line")}
                        onChange={(event) => setSupportedModelsText(event.currentTarget.value)}
                      />
                    </label>}
                    <label className="nt-field">
                      <span>{t("首个账号 ID", "First account ID")}</span>
                      <input
                        className="nt-input"
                        value={draft.credentialId}
                        disabled={locked}
                        onChange={(event) => {
                          const credentialId = event.currentTarget.value;
                          setDraft((current) => ({ ...current, credentialId }));
                        }}
                      />
                    </label>
                    <label className="nt-field">
                      <span>{t("账号名称", "Account name")}</span>
                      <input
                        className="nt-input"
                        value={draft.accountName}
                        disabled={locked}
                        onChange={(event) => {
                          const accountName = event.currentTarget.value;
                          setDraft((current) => ({ ...current, accountName }));
                        }}
                      />
                    </label>
                    <label className="nt-field nt-field--wide">
                      <span>API Key</span>
                      <input
                        className="nt-input"
                        type="password"
                        autoComplete="new-password"
                        value={draft.apiKey}
                        disabled={locked}
                        onChange={(event) => {
                          const apiKey = event.currentTarget.value;
                          setDraft((current) => ({ ...current, apiKey }));
                        }}
                      />
                    </label>
                  </div>

                  {!hasSecretAccess ? (
                    <div className="nt-validation-list nt-validation-list--warning">
                      <strong>{t("保存密钥需要敏感信息权限", "Secret access is required to save the key")}</strong>
                      <ul>
                        <li>
                          {t(
                            "服务商和账号可以先写入草稿；正式保存路由配置前需要确认管理员敏感信息访问权限。",
                            "The provider and account can be added to the draft first; confirm administrator secret access before saving the route config.",
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
                  <button className="nt-btn nt-btn--outline" type="button">
                    {t("取消", "Cancel")}
                  </button>
                </Dialog.Close>
                <button className="nt-btn nt-btn--primary" type="submit" disabled={locked}>
                  {t("创建服务商与首个账号", "Create provider and first account")}
                </button>
              </div>
            </section>
  );
}
