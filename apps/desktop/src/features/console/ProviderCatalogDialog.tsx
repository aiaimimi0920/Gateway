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
import type { DiscoverAccount } from "./accountDiscovery";
import { availableCatalogId } from "./providerCatalogIdentity";

export type ProviderCatalogDialogProps = {
  open: boolean;
  existingProviderIds: readonly string[];
  existingCredentialIds: readonly string[];
  locked: boolean;
  hasSecretAccess: boolean;
  onOpenChange(open: boolean): void;
  onRequestSecretAccess(): void;
  onSubmit(value: ProviderCatalogDraft): void;
  onDiscover?: DiscoverAccount;
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
  providerIds: readonly string[],
  credentialIds: readonly string[],
): ProviderCatalogDraft {
  const providerId = availableCatalogId(template.providerId, providerIds);
  return {
    templateId: template.id,
    providerId,
    providerLabel: providerId === template.providerId ? label : `${label} (${providerId.slice(template.providerId.length + 1)})`,
    vendorKey: template.vendorKey,
    vendorName: template.vendorName,
    baseUrl: template.baseUrl,
    supportedModels: [...template.supportedModels],
    credentialId: availableCatalogId(suggestedCredentialId(providerId), credentialIds),
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
  onSubmit,
  onDiscover,
}: ProviderCatalogDialogProps) {
  const { t } = useUiLocale();
  const openerRef = useRef<HTMLElement | null>(null);
  const wasOpen = useRef(false);
  const firstTemplate: ProviderCatalogTemplate = PROVIDER_CATALOG_TEMPLATES[0];
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<CatalogCategoryFilter>("all");
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>(firstTemplate.id);
  const [draft, setDraft] = useState<ProviderCatalogDraft>(() =>
    draftFromTemplate(firstTemplate, t(firstTemplate.labelZh, firstTemplate.labelEn), existingProviderIds, existingCredentialIds),
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

  useEffect(() => {
    if (!open) {
      wasOpen.current = false;
      return;
    }
    // A background inventory refresh must not erase a partially completed form.
    if (wasOpen.current) return;
    wasOpen.current = true;
    const initial = PROVIDER_CATALOG_TEMPLATES[0];
    const nextDraft = draftFromTemplate(initial, t(initial.labelZh, initial.labelEn), existingProviderIds, existingCredentialIds);
    setQuery("");
    setCategory("all");
    setSelectedTemplateId(initial.id);
    setDraft(nextDraft);
    setSupportedModelsText(nextDraft.supportedModels.join("\n"));
    setValidationError(null);
  }, [open, t, existingProviderIds, existingCredentialIds]);

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
    const nextDraft = draftFromTemplate(template, t(template.labelZh, template.labelEn), existingProviderIds, existingCredentialIds);
    setSelectedTemplateId(template.id);
    setDraft(nextDraft);
    setSupportedModelsText(nextDraft.supportedModels.join("\n"));
    setValidationError(null);
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (locked) return;
    const providerId = draft.providerId.trim();
    const providerLabel = draft.providerLabel.trim();
    const vendorKey = draft.vendorKey.trim();
    const vendorName = draft.vendorName.trim();
    const baseUrl = draft.baseUrl.trim().replace(/\/$/, "");
    const credentialId = draft.credentialId.trim();
    const accountName = draft.accountName.trim();
    const apiKey = draft.apiKey.trim();
    const supportedModels = parseSupportedModels(supportedModelsText);

    if (existingProviderIdSet.has(providerId)) {
      setValidationError(t(`Provider ID ${providerId} 已存在，请使用其他 ID。`, `Provider ID ${providerId} already exists; use a different ID.`));
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
    if (supportedModels.length === 0 && !(selectedTemplate.custom && onDiscover)) {
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

    const submission: ProviderCatalogDraft = {
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
    };
    onSubmit(submission);
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
              "选择服务商",
              "Select a provider",
            )}
          </Dialog.Description>

          <form className="nt-provider-catalog" onSubmit={submit}>
            <ProviderCatalogDirectory
              t={t}
              query={query}
              category={category}
              filteredTemplates={filteredTemplates}
              selectedTemplateId={selectedTemplateId}
              setQuery={setQuery}
              setCategory={setCategory}
              selectTemplate={selectTemplate}
            />

            <ProviderCatalogForm
              t={t}
              selectedTemplate={selectedTemplate}
              draft={draft}
              setDraft={setDraft}
              supportedModelsText={supportedModelsText}
              setSupportedModelsText={setSupportedModelsText}
              locked={locked}
              autoDiscovery={!!selectedTemplate.custom && !!onDiscover}
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
