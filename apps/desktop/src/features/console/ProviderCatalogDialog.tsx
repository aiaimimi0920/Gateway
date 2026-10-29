import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { ConsoleCredentialPoolAutomationDriver } from "../../api/contracts";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft, type ProviderCatalogTemplate } from "./providerCatalog";
import { ProviderCatalogDirectory, type CatalogCategoryFilter } from "./ProviderCatalogDirectory";
import { ProviderCatalogForm } from "./ProviderCatalogForm";
import type { DiscoverAccount } from "./accountDiscovery";
import { availableCatalogId } from "./providerCatalogIdentity";
import { catalogWorkflowError, compatibleCatalogDrivers, defaultCatalogWorkflow } from "./providerCatalogWorkflow";

export type ProviderCatalogDialogProps = {
  open: boolean;
  existingProviderIds: readonly string[];
  existingCredentialIds: readonly string[];
  locked: boolean;
  hasSecretAccess: boolean;
  drivers?: readonly ConsoleCredentialPoolAutomationDriver[];
  onOpenChange(open: boolean): void;
  onRequestSecretAccess(): void;
  onSubmit(value: ProviderCatalogDraft): void;
  onDiscover?: DiscoverAccount;
};
export type CatalogTab = "request" | "mapping" | "tests" | "pool" | "health";

function draftFromTemplate(template: ProviderCatalogTemplate, providerIds: readonly string[]): ProviderCatalogDraft {
  const providerId = availableCatalogId(template.providerId, providerIds);
  return {
    templateId: template.id, providerId, providerLabel: providerId,
    vendorKey: template.vendorKey, vendorName: template.vendorName,
    baseUrl: template.baseUrl, supportedModels: [...template.supportedModels],
    requestClientProfile: template.id === "chatgpt" ? "codex_cli" : "auto",
    workflow: defaultCatalogWorkflow(template), secrets: {},
  };
}
function validUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password && !url.search && !url.hash;
  } catch { return false; }
}

export function ProviderCatalogDialog({ open, existingProviderIds, locked, hasSecretAccess, drivers = [],
  onOpenChange, onRequestSecretAccess, onSubmit }: ProviderCatalogDialogProps) {
  const { t } = useUiLocale();
  const openerRef = useRef<HTMLElement | null>(null);
  const wasOpen = useRef(false);
  const first = PROVIDER_CATALOG_TEMPLATES[0];
  const [category, setCategory] = useState<CatalogCategoryFilter>("standard");
  const [draft, setDraft] = useState(() => draftFromTemplate(first, existingProviderIds));
  const [tab, setTab] = useState<CatalogTab>("request");
  const [validationError, setValidationError] = useState<string | null>(null);
  const selectedTemplate = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === draft.templateId) ?? first;
  useEffect(() => {
    if (!open) { wasOpen.current = false; return; }
    if (wasOpen.current) return; // Background refresh must not erase a partially edited draft.
    wasOpen.current = true;
    setCategory("standard"); setTab("request"); setDraft(draftFromTemplate(first, existingProviderIds)); setValidationError(null);
  }, [open, first, existingProviderIds]);
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (locked) return;
    const providerId = draft.providerId.trim();
    const baseUrl = draft.baseUrl.trim().replace(/\/$/, "");
    const fail = (message: string) => setValidationError(message);
    if (existingProviderIds.some((id) => id.trim() === providerId)) return fail(t(`Provider ID ${providerId} 已存在，请使用其他 ID。`, `Provider ID ${providerId} already exists.`));
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(providerId) || providerId.length > 256) return fail(t("请输入有效的 Provider ID。", "Enter a valid Provider ID."));
    if (!validUrl(baseUrl)) return fail(t("请输入有效的 HTTP(S) Base URL，不能包含凭据、查询参数或片段。", "Enter an HTTP(S) base URL without credentials, query or fragment."));
    const workflow = draft.workflow!;
    const error = catalogWorkflowError(workflow, draft.secrets);
    if (error) return fail(error);
    if (workflow.specialAdapter === "basispoints" && baseUrl !== "https://chatgpt.com/backend-api/codex") return fail(t("Basispoints 仅支持 ChatGPT 原生 Codex 地址。", "Basispoints requires the native ChatGPT Codex endpoint."));
    const available = compatibleCatalogDrivers(drivers, providerId);
    const driver = available.length === 1 ? available[0].id : available.find((entry) => entry.id === workflow.driverId)?.id ?? "";
    if ((workflow.autoRefill || workflow.autoPrune) && !driver) return fail(t("请先选择兼容的号池驱动。", "Select a compatible pool driver."));
    if (Object.values(draft.secrets ?? {}).some(Boolean) && !hasSecretAccess) {
      onRequestSecretAccess(); return fail(t("保存认证信息需要敏感信息权限。", "Secret access is required to save authentication."));
    }
    onSubmit({ ...draft, providerId, providerLabel: providerId, baseUrl,
      workflow: { ...workflow, driverId: driver } });
    onOpenChange(false);
  };
  return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Portal>
    <Dialog.Overlay className="dialog-overlay" />
    <Dialog.Content className="dialog-content nt-provider-catalog-dialog" aria-describedby={undefined}
      onOpenAutoFocus={() => { openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null; }}
      onCloseAutoFocus={(event) => {
        event.preventDefault();
        if (document.activeElement?.closest('[role="dialog"]')?.isConnected) return;
        if (openerRef.current?.isConnected) openerRef.current.focus();
      }}>
      <div className="nt-pilot-dialog__header"><Dialog.Title>{t("添加服务商", "Add provider")}</Dialog.Title>
        <Dialog.Close asChild><button className="nt-icon-close" type="button" aria-label={t("关闭", "Close")}><X size={18} aria-hidden="true" /></button></Dialog.Close>
      </div>
      <form className="nt-provider-catalog" noValidate onSubmit={submit}>
        <ProviderCatalogDirectory t={t} category={category} setCategory={setCategory}
          filteredTemplates={PROVIDER_CATALOG_TEMPLATES.filter((entry) => Boolean(entry.custom) === (category === "custom"))}
          selectedTemplateId={draft.templateId} selectTemplate={(template) => {
            setDraft(draftFromTemplate(template, existingProviderIds)); setValidationError(null);
          }} />
        <ProviderCatalogForm t={t} selectedTemplate={selectedTemplate} draft={draft} setDraft={setDraft}
          tab={tab} setTab={setTab} locked={locked} drivers={drivers} validationError={validationError} />
      </form>
    </Dialog.Content>
  </Dialog.Portal></Dialog.Root>;
}
