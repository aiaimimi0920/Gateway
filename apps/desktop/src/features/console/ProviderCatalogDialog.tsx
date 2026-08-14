import * as Dialog from "@radix-ui/react-dialog";
import { Search, X } from "lucide-react";
import { type FormEvent, useEffect, useMemo, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import {
  PROVIDER_CATALOG_TEMPLATES,
  type ProviderCatalogCategory,
  type ProviderCatalogDraft,
  type ProviderCatalogTemplate,
} from "./providerCatalog";

export type ProviderCatalogDialogProps = {
  open: boolean;
  existingProviderIds: readonly string[];
  existingCredentialIds: readonly string[];
  locked: boolean;
  hasSecretAccess: boolean;
  onOpenChange(open: boolean): void;
  onRequestSecretAccess(): void;
  onAddAccount(providerId: string): void;
  onSubmit(value: ProviderCatalogDraft): void;
};

type CatalogCategoryFilter = "all" | ProviderCatalogCategory;

function suggestedCredentialId(providerId: string): string {
  const normalized = providerId
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return `${normalized || "provider"}-account-1`;
}

function draftFromTemplate(
  template: ProviderCatalogTemplate,
  label: string,
): ProviderCatalogDraft {
  return {
    templateId: template.id,
    providerId: template.providerId,
    providerLabel: label,
    vendorKey: template.vendorKey,
    vendorName: template.vendorName,
    baseUrl: template.baseUrl,
    supportedModels: [...template.supportedModels],
    credentialId: suggestedCredentialId(template.providerId),
    accountName: `${template.vendorName} Account 1`,
    apiKey: "",
  };
}

function parseSupportedModels(value: string): string[] {
  return [
    ...new Set(
      value
        .split(/[\n,]/)
        .map((entry) => entry.trim())
        .filter((entry) => entry.length > 0),
    ),
  ];
}

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:";
  } catch {
    return false;
  }
}

export function ProviderCatalogDialog({
  open,
  existingProviderIds,
  existingCredentialIds,
  locked,
  hasSecretAccess,
  onOpenChange,
  onRequestSecretAccess,
  onAddAccount,
  onSubmit,
}: ProviderCatalogDialogProps) {
  const { t } = useUiLocale();
  const firstTemplate: ProviderCatalogTemplate = PROVIDER_CATALOG_TEMPLATES[0];
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<CatalogCategoryFilter>("all");
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>(firstTemplate.id);
  const [draft, setDraft] = useState<ProviderCatalogDraft>(() =>
    draftFromTemplate(firstTemplate, t(firstTemplate.labelZh, firstTemplate.labelEn)),
  );
  const [supportedModelsText, setSupportedModelsText] = useState(
    firstTemplate.supportedModels.join("\n"),
  );
  const [validationError, setValidationError] = useState<string | null>(null);

  const selectedTemplate: ProviderCatalogTemplate =
    PROVIDER_CATALOG_TEMPLATES.find((template) => template.id === selectedTemplateId) ??
    firstTemplate;
  const existingProviderIdSet = useMemo(
    () => new Set(existingProviderIds.map((id) => id.trim())),
    [existingProviderIds],
  );
  const existingCredentialIdSet = useMemo(
    () => new Set(existingCredentialIds.map((id) => id.trim())),
    [existingCredentialIds],
  );
  const providerAlreadyExists = existingProviderIdSet.has(draft.providerId.trim());

  useEffect(() => {
    if (!open) {
      return;
    }
    const initial = PROVIDER_CATALOG_TEMPLATES[0];
    const nextDraft = draftFromTemplate(initial, t(initial.labelZh, initial.labelEn));
    setQuery("");
    setCategory("all");
    setSelectedTemplateId(initial.id);
    setDraft(nextDraft);
    setSupportedModelsText(nextDraft.supportedModels.join("\n"));
    setValidationError(null);
  }, [open, t]);

  const filteredTemplates = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return PROVIDER_CATALOG_TEMPLATES.filter((template) => {
      if (category !== "all" && template.category !== category) {
        return false;
      }
      if (!normalizedQuery) {
        return true;
      }
      return [
        template.labelZh,
        template.labelEn,
        template.vendorName,
        template.providerId,
        template.preset ?? "",
      ].some((value) => value.toLowerCase().includes(normalizedQuery));
    });
  }, [category, query]);

  const selectTemplate = (template: ProviderCatalogTemplate) => {
    const nextDraft = draftFromTemplate(template, t(template.labelZh, template.labelEn));
    setSelectedTemplateId(template.id);
    setDraft(nextDraft);
    setSupportedModelsText(nextDraft.supportedModels.join("\n"));
    setValidationError(null);
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const providerId = draft.providerId.trim();
    const providerLabel = draft.providerLabel.trim();
    const vendorKey = draft.vendorKey.trim();
    const vendorName = draft.vendorName.trim();
    const baseUrl = draft.baseUrl.trim().replace(/\/$/, "");
    const credentialId = draft.credentialId.trim();
    const accountName = draft.accountName.trim();
    const apiKey = draft.apiKey.trim();
    const supportedModels = parseSupportedModels(supportedModelsText);

    if (providerAlreadyExists) {
      onAddAccount(providerId);
      onOpenChange(false);
      return;
    }
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(providerId)) {
      setValidationError(
        t(
          "Provider ID 必须以字母或数字开头，且只能包含字母、数字、点、下划线、冒号或连字符。",
          "Provider ID must start with a letter or number and contain only letters, numbers, dots, underscores, colons, or hyphens.",
        ),
      );
      return;
    }
    if (!providerLabel || !vendorKey || !vendorName) {
      setValidationError(
        t(
          "Provider 名称、服务商标识和服务商名称不能为空。",
          "Provider label, vendor key, and vendor name are required.",
        ),
      );
      return;
    }
    if (!isHttpUrl(baseUrl)) {
      setValidationError(
        t("请输入有效的 HTTP(S) Base URL。", "Enter a valid HTTP(S) base URL."),
      );
      return;
    }
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(credentialId)) {
      setValidationError(
        t(
          "账号 ID 必须以字母或数字开头，且只能包含字母、数字、点、下划线、冒号或连字符。",
          "Account ID must start with a letter or number and contain only letters, numbers, dots, underscores, colons, or hyphens.",
        ),
      );
      return;
    }
    if (existingCredentialIdSet.has(credentialId)) {
      setValidationError(
        t(`账号 ID ${credentialId} 已存在。`, `Account ID ${credentialId} already exists.`),
      );
      return;
    }
    if (supportedModels.length === 0) {
      setValidationError(
        t(
          "至少填写一个模型，Gateway 才能创建对应模型路由并参与聚合。",
          "Enter at least one model so Gateway can create model routes and include the provider in aggregation.",
        ),
      );
      return;
    }
    if (!apiKey) {
      setValidationError(t("首个账号必须填写 API Key。", "The first account requires an API key."));
      return;
    }

    onSubmit({
      ...draft,
      providerId,
      providerLabel,
      vendorKey,
      vendorName,
      baseUrl,
      supportedModels,
      credentialId,
      accountName,
      apiKey,
    });
    onOpenChange(false);
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-provider-catalog-dialog"
          aria-describedby="provider-catalog-description"
        >
          <div className="nt-pilot-dialog__header">
            <div>
              <p className="nt-kicker">// Provider catalog</p>
              <Dialog.Title>{t("添加服务商与账号", "Add provider and account")}</Dialog.Title>
            </div>
            <Dialog.Close asChild>
              <button className="nt-icon-close" type="button" aria-label={t("关闭", "Close")}>
                <X size={18} aria-hidden="true" />
              </button>
            </Dialog.Close>
          </div>
          <Dialog.Description id="provider-catalog-description">
            {t(
              "选择 Gateway 已内建的主流 API 模板，或手动添加第三方 OpenAI-compatible 服务。创建后可继续在同一 Provider 下添加多个账号。",
              "Choose a mainstream API template built into Gateway, or add a third-party OpenAI-compatible service. You can add more accounts under the same provider afterward.",
            )}
          </Dialog.Description>

          <form className="nt-provider-catalog" onSubmit={submit}>
            <aside className="nt-provider-catalog__directory" aria-label={t("服务商目录", "Provider directory")}>
              <label className="nt-field nt-provider-catalog__search">
                <span>{t("搜索服务商", "Search providers")}</span>
                <span className="nt-provider-catalog__search-control">
                  <Search size={15} aria-hidden="true" />
                  <input
                    className="nt-input"
                    value={query}
                    placeholder={t("名称、Provider ID 或 preset", "Name, provider ID, or preset")}
                    onChange={(event) => setQuery(event.currentTarget.value)}
                  />
                </span>
              </label>
              <label className="nt-field">
                <span>{t("目录分类", "Catalog category")}</span>
                <select
                  className="nt-select"
                  value={category}
                  onChange={(event) => setCategory(event.currentTarget.value as CatalogCategoryFilter)}
                >
                  <option value="all">{t("全部内建模板", "All built-in templates")}</option>
                  <option value="mainstream">{t("主流官方服务", "Mainstream official services")}</option>
                  <option value="aggregator">{t("模型聚合服务", "Model aggregators")}</option>
                  <option value="search">{t("搜索与抓取", "Search and fetch")}</option>
                  <option value="third-party-compatible">
                    {t("第三方兼容服务", "Third-party compatible services")}
                  </option>
                </select>
              </label>
              <div className="nt-provider-catalog__list">
                {filteredTemplates.map((template) => {
                  const selected = template.id === selectedTemplateId;
                  const alreadyAdded = existingProviderIdSet.has(template.providerId);
                  return (
                    <button
                      className={
                        selected
                          ? "nt-provider-catalog__item nt-provider-catalog__item--selected"
                          : "nt-provider-catalog__item"
                      }
                      type="button"
                      aria-pressed={selected}
                      key={template.id}
                      onClick={() => selectTemplate(template)}
                    >
                      <span className="nt-provider-catalog__item-copy">
                        <strong>{t(template.labelZh, template.labelEn)}</strong>
                        <small>{template.providerId}</small>
                      </span>
                      {alreadyAdded ? (
                        <span className="nt-badge nt-badge--success">
                          {t("已添加", "Added")}
                        </span>
                      ) : template.category === "third-party-compatible" ? (
                        <span className="nt-badge nt-badge--info">
                          {t("第三方兼容", "Third-party")}
                        </span>
                      ) : null}
                    </button>
                  );
                })}
                {filteredTemplates.length === 0 ? (
                  <p className="nt-empty">{t("没有匹配的服务商模板。", "No provider templates match.")}</p>
                ) : null}
              </div>
            </aside>

            <section className="nt-provider-catalog__form">
              <div className="nt-provider-catalog__summary">
                <div>
                  <strong>{t(selectedTemplate.labelZh, selectedTemplate.labelEn)}</strong>
                  <p>{t(selectedTemplate.descriptionZh, selectedTemplate.descriptionEn)}</p>
                </div>
                <span className="nt-badge nt-badge--info">
                  {selectedTemplate.compatibility === "openai"
                    ? "OpenAI-compatible"
                    : selectedTemplate.compatibility === "anthropic"
                      ? "Anthropic Messages"
                      : selectedTemplate.compatibility === "search"
                        ? t("搜索协议", "Search protocol")
                        : t("原生协议", "Native protocol")}
                </span>
              </div>

              {providerAlreadyExists ? (
                <div className="nt-validation-list nt-validation-list--info" role="status">
                  <strong>{t("该 Provider 已存在", "This provider already exists")}</strong>
                  <ul>
                    <li>
                      {t(
                        "不会重复创建 Provider；继续后将打开账号对话框，为现有服务商添加另一个账号。",
                        "Gateway will not create a duplicate provider. Continue to open the account dialog and add another account.",
                      )}
                    </li>
                  </ul>
                </div>
              ) : (
                <>
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
                        placeholder="https://api.example.com/v1"
                        onChange={(event) => {
                          const baseUrl = event.currentTarget.value;
                          setDraft((current) => ({ ...current, baseUrl }));
                        }}
                      />
                    </label>
                    <label className="nt-field nt-field--wide">
                      <span>{t("支持模型与聚合路由", "Supported models and aggregation routes")}</span>
                      <textarea
                        className="nt-textarea nt-provider-catalog__models"
                        rows={4}
                        value={supportedModelsText}
                        disabled={locked}
                        placeholder={t("每行一个模型", "One model per line")}
                        onChange={(event) => setSupportedModelsText(event.currentTarget.value)}
                      />
                    </label>
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
                </>
              )}

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
                  {providerAlreadyExists
                    ? t("为现有服务商添加账号", "Add account to existing provider")
                    : t("创建服务商与首个账号", "Create provider and first account")}
                </button>
              </div>
            </section>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
