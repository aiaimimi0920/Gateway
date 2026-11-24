import { Dialog } from "@neuro/ui";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { ConsoleCredentialPoolAutomationDriver } from "../../api/contracts";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft, type ProviderCatalogDraft, type ProviderCatalogTemplate } from "./providerCatalog";
import { ProviderCatalogDirectory, type CatalogCategoryFilter } from "./ProviderCatalogDirectory";
import { ProviderCatalogForm } from "./ProviderCatalogForm";
import type { DiscoverAccount } from "./accountDiscovery";
import type { DiscoverSapDeployments } from "../../api/sapAiCoreDeployments";
import { availableCatalogId } from "./providerCatalogIdentity";
import { catalogWorkflowError, compatibleCatalogDrivers, defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { qwenCatalogBase } from "./qwenEndpoint";
import { vertexOAuthCatalogBase } from "./vertexOAuthCatalog";
import { kiroOAuthCatalogBase } from "./kiroOAuthCatalog";
import { supportsCatalogApiKey } from "./catalogFirstAccount";
import { providerAccessKind } from "./providerAccessKind";

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
  onDiscoverSapDeployments?: DiscoverSapDeployments;
};
export type CatalogTab = "basics" | "request" | "mapping" | "tests" | "pool" | "health";

function draftFromTemplate(template: ProviderCatalogTemplate, providerIds: readonly string[]): ProviderCatalogDraft {
  const providerId = availableCatalogId(template.providerId, providerIds);
  return {
    templateId: template.id, providerId, providerLabel: providerId,
    vendorKey: template.vendorKey, vendorName: template.vendorName,
    baseUrl: template.baseUrl, supportedModels: [...template.supportedModels],
    requestClientProfile: template.id === "chatgpt" ? "codex_cli" : "auto",
    workflow: defaultCatalogWorkflow(template), secrets: {},
    credentialId: supportsCatalogApiKey(template) ? crypto.randomUUID() : undefined,
  };
}
function validUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password && !url.search && !url.hash;
  } catch { return false; }
}

export function ProviderCatalogDialog({ open, existingProviderIds, existingCredentialIds, locked, hasSecretAccess, drivers = [],
  onOpenChange, onRequestSecretAccess, onSubmit, onDiscoverSapDeployments }: ProviderCatalogDialogProps) {
  const { t } = useUiLocale();
  const wasOpen = useRef(false);
  const first = PROVIDER_CATALOG_TEMPLATES[0];
  const [category, setCategory] = useState<CatalogCategoryFilter>("all");
  const [draft, setDraft] = useState(() => draftFromTemplate(first, existingProviderIds));
  const [tab, setTab] = useState<CatalogTab>("basics");
  const [validationError, setValidationError] = useState<string | null>(null);
  const selectedTemplate = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === draft.templateId) ?? first;
  useEffect(() => {
    if (!open) { wasOpen.current = false; return; }
    if (wasOpen.current) return; // Background refresh must not erase a partially edited draft.
    wasOpen.current = true;
    setCategory("all"); setTab("basics"); setDraft(draftFromTemplate(first, existingProviderIds)); setValidationError(null);
  }, [open, first, existingProviderIds]);
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (locked) return;
    const providerId = draft.providerId.trim();
    const baseUrl = draft.baseUrl.trim().replace(/\/$/, "");
    const fail = (message: string) => setValidationError(message);
    if (existingProviderIds.some((id) => id.trim() === providerId)) return fail(t(`Provider ID ${providerId} 已存在，请使用其他 ID。`, `Provider ID ${providerId} already exists.`));
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(providerId) || providerId.length > 256) return fail(t("请输入有效的 Provider ID。", "Enter a valid Provider ID."));
    if (selectedTemplate.preset === "qwen-official-api") { try { qwenCatalogBase(draft); } catch (error) { return fail(String(error instanceof Error ? error.message : error)); } }
    if (selectedTemplate.id === "vertex-oauth") { try { vertexOAuthCatalogBase(draft); } catch (error) { return fail(String(error instanceof Error ? error.message : error)); } }
    if (selectedTemplate.id === "kiro-oauth") { try { kiroOAuthCatalogBase(draft); } catch (error) { return fail(String(error instanceof Error ? error.message : error)); } }
    if (selectedTemplate.id !== "cloudflare-workers-ai" && !validUrl(baseUrl)) return fail(t("请输入有效的 HTTP(S) Base URL，不能包含凭据、查询参数或片段。", "Enter an HTTP(S) base URL without credentials, query or fragment."));
    if (selectedTemplate.id === "accio-oauth" && baseUrl !== "https://phoenix-gw.alibaba.com") return fail(t("Accio OAuth 仅支持固定的服务地址。", "Accio OAuth requires its fixed gateway endpoint."));
    if (selectedTemplate.id === "kiro-api-key" && baseUrl !== selectedTemplate.baseUrl) return fail(t("Kiro CLI API Key 仅支持固定的 US East runtime 地址。", "Kiro CLI API Key requires its fixed US East runtime endpoint."));
    if (selectedTemplate.id === "vertex-express-api" && baseUrl !== selectedTemplate.baseUrl) return fail(t("Vertex Express API Key 仅支持固定的 global publisher-model 地址。", "Vertex Express API Key requires its fixed global publisher-model endpoint."));
    if (selectedTemplate.id === "lumalabs-api" && baseUrl !== selectedTemplate.baseUrl) return fail(t("Luma Agents 仅支持固定官方地址。", "Luma Agents requires its fixed official endpoint."));
    if (selectedTemplate.id === "quick-oauth" && baseUrl !== selectedTemplate.baseUrl) return fail(t("Amazon Quick 仅支持固定的 https://quick.aws.com 地址。", "Amazon Quick requires its fixed https://quick.aws.com endpoint."));
    const workflow = draft.workflow!;
    const error = catalogWorkflowError(workflow, draft.secrets);
    if (error) return fail(error);
    if (["lumalabs-api", "lumalabs-mcp", "sap-aicore-chat"].includes(selectedTemplate.id)) { try { providerDefinitionFromCatalogDraft(selectedTemplate, { ...draft, baseUrl }); }
      catch (cause) { return fail(cause instanceof Error ? cause.message : String(cause)); } }
    if (workflow.specialAdapter === "basispoints" && baseUrl !== "https://chatgpt.com/backend-api/codex") return fail(t("Basispoints 仅支持 ChatGPT 原生 Codex 地址。", "Basispoints requires the native ChatGPT Codex endpoint."));
    const available = compatibleCatalogDrivers(drivers, providerId);
    const driver = selectedTemplate.id === "lumalabs-mcp" ? "" : available.length === 1 ? available[0].id : available.find((entry) => entry.id === workflow.driverId)?.id ?? "";
    if ((workflow.autoRefill || workflow.autoPrune) && !driver) return fail(t("请先选择兼容的号池驱动。", "Select a compatible pool driver."));
    if (draft.credentialId && (!supportsCatalogApiKey(selectedTemplate) || !draft.apiKey?.trim())) return fail(t("请填写首个账号的 API Key，或取消同时添加账号。", "Enter an API Key or disable first account creation."));
    if (draft.credentialId && existingCredentialIds.includes(draft.credentialId)) return fail(t("账号 ID 已存在，请重新选择模板。", "Account ID already exists; reselect the template."));
    if ((draft.apiKey?.trim() || Object.values(draft.secrets ?? {}).some(Boolean)) && !hasSecretAccess) {
      onRequestSecretAccess(); return fail(t("保存认证信息需要敏感信息权限。", "Secret access is required to save authentication."));
    }
    onSubmit({
      ...draft, providerId, providerLabel: draft.providerLabel.trim() || providerId, baseUrl,
      workflow: { ...workflow, driverId: driver }
    });
    onOpenChange(false);
  };
  return <Dialog open={open} onOpenChange={onOpenChange} title={t("添加服务商", "Add provider")}
    closeLabel={t("关闭", "Close")} className="nt-provider-catalog-dialog">
    <form className="nt-provider-catalog" noValidate onSubmit={submit}>
      <ProviderCatalogDirectory t={t} category={category} setCategory={setCategory}
        filteredTemplates={PROVIDER_CATALOG_TEMPLATES.filter(entry => category === "all" || (entry.custom ? "custom" : entry.id.endsWith("-oauth") || ["lumalabs-mcp", "lobsterai", "nous-portal"].includes(entry.id) ? "oauth" : providerAccessKind({ providerPreset: entry.preset, adapter: entry.adapter, protocolProfile: entry.protocolProfile })) === category)}
        selectedTemplateId={draft.templateId} selectTemplate={(template) => {
          setDraft(draftFromTemplate(template, existingProviderIds)); setTab("basics"); setValidationError(null);
        }} />
      <ProviderCatalogForm t={t} selectedTemplate={selectedTemplate} draft={draft} setDraft={setDraft}
        tab={tab} setTab={setTab} locked={locked} drivers={drivers} validationError={validationError}
        discoverSapDeployments={open ? onDiscoverSapDeployments : undefined} hasSecretAccess={hasSecretAccess} onRequestSecretAccess={onRequestSecretAccess} />
    </form>
  </Dialog>;
}
