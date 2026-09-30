import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import {
  PROVIDER_CATALOG_TEMPLATES,
  type ProviderCatalogDraft,
  type ProviderCatalogTemplate,
} from "./providerCatalog";
import { ProviderCatalogDirectory, type CatalogCategoryFilter } from "./ProviderCatalogDirectory";
import { ProviderCatalogForm } from "./ProviderCatalogForm";

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
  const openerRef = useRef<HTMLElement | null>(null);
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
          onOpenAutoFocus={() => {
            openerRef.current = document.activeElement instanceof HTMLElement
              ? document.activeElement : null;
          }}
          onCloseAutoFocus={(event) => {
            // Controlled dialogs have no Radix trigger; preserve a successor dialog's focus.
            event.preventDefault();
            if (document.activeElement?.closest('[role="dialog"]')?.isConnected) return;
            if (openerRef.current?.isConnected) openerRef.current.focus();
          }}
        >
          <div className="nt-pilot-dialog__header">
            <div>
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
            <ProviderCatalogDirectory
              t={t}
              query={query}
              category={category}
              filteredTemplates={filteredTemplates}
              selectedTemplateId={selectedTemplateId}
              existingProviderIdSet={existingProviderIdSet}
              setQuery={setQuery}
              setCategory={setCategory}
              selectTemplate={selectTemplate}
            />

            <ProviderCatalogForm
              t={t}
              selectedTemplate={selectedTemplate}
              providerAlreadyExists={providerAlreadyExists}
              draft={draft}
              setDraft={setDraft}
              supportedModelsText={supportedModelsText}
              setSupportedModelsText={setSupportedModelsText}
              locked={locked}
              hasSecretAccess={hasSecretAccess}
              validationError={validationError}
              onRequestSecretAccess={onRequestSecretAccess}
              suggestedCredentialId={suggestedCredentialId}
            />
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
