import type { Dispatch, SetStateAction } from "react";
import type { ConsoleCredentialPoolAutomationDriver } from "../../api/contracts";
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import type { CatalogTab } from "./ProviderCatalogDialog";
import type { CatalogWorkflow } from "./providerCatalogWorkflow";
import { CatalogField } from "./ProviderCatalogFields";
import { ProviderCatalogRequestFields } from "./ProviderCatalogRequestFields";
import { ProviderCatalogMappingFields } from "./ProviderCatalogMappingFields";
import { ProviderCatalogTestFields } from "./ProviderCatalogTestFields";
import { ProviderCatalogPoolFields } from "./ProviderCatalogPoolFields";
import { ProviderCatalogHealthFields } from "./ProviderCatalogHealthFields";

export type ProviderCatalogFormProps = {
  t(zh: string, en: string): string;
  selectedTemplate: ProviderCatalogTemplate;
  draft: ProviderCatalogDraft;
  setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>;
  locked: boolean;
  tab: CatalogTab;
  setTab(tab: CatalogTab): void;
  drivers: readonly ConsoleCredentialPoolAutomationDriver[];
  validationError: string | null;
};
export function ProviderCatalogForm({ t, selectedTemplate, draft, setDraft, locked, tab, setTab, drivers, validationError }: ProviderCatalogFormProps) {
  const update = (patch: Partial<CatalogWorkflow>) => setDraft((current) => ({ ...current, workflow: { ...current.workflow!, ...patch } }));
  const fields = { t, workflow: draft.workflow!, update, locked };
  const tabs = [["request", t("调用流程", "Call flow")], ["mapping", t("模型映射", "Model mapping")],
    ["tests", t("自动测试", "Automatic tests")], ["pool", t("号池流程", "Pool workflow")], ["health", t("站点健康", "Site health")]] as const;
  const panel = {
    request: <ProviderCatalogRequestFields {...fields} template={selectedTemplate} draft={draft} setDraft={setDraft} />,
    mapping: <ProviderCatalogMappingFields key={draft.templateId} {...fields} models={draft.supportedModels} />,
    tests: <ProviderCatalogTestFields key={draft.templateId} {...fields} />,
    pool: <ProviderCatalogPoolFields key={draft.templateId} {...fields} draft={draft} setDraft={setDraft} drivers={drivers} />,
    health: <ProviderCatalogHealthFields {...fields} template={selectedTemplate} draft={draft} setDraft={setDraft} />,
  };
  return <section className="nt-provider-catalog__form" aria-label={t("服务商配置", "Provider configuration")}>
    <div className="nt-provider-catalog__basics">
      <CatalogField name="Provider ID"><input className="nt-input" value={draft.providerId} maxLength={256} disabled={locked}
        onChange={(event) => setDraft((current) => ({ ...current, providerId: event.target.value }))} /></CatalogField>
      <CatalogField name="Base URL"><input className="nt-input" type="url" value={draft.baseUrl} disabled={locked}
        readOnly={!selectedTemplate.custom && selectedTemplate.id !== "azure-api"} spellCheck={false}
        onChange={(event) => setDraft((current) => ({ ...current, baseUrl: event.target.value }))} /></CatalogField>
    </div>
    <div className="nt-provider-catalog__tabs" role="tablist" aria-label={t("服务商标准配置", "Provider configuration tabs")}>
      {tabs.map(([id, name], index) => <button type="button" key={id} role="tab" id={`catalog-tab-${id}`}
        aria-selected={tab === id} aria-controls={`catalog-panel-${id}`} tabIndex={tab === id ? 0 : -1} onClick={() => setTab(id)}
        onKeyDown={(event) => {
          const next = event.key === "ArrowRight" ? (index + 1) % tabs.length : event.key === "ArrowLeft" ? (index + tabs.length - 1) % tabs.length
            : event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : null;
          if (next !== null) { event.preventDefault(); setTab(tabs[next][0]); document.getElementById(`catalog-tab-${tabs[next][0]}`)?.focus(); }
        }}>{name}</button>)}
    </div>
    <div className="nt-provider-catalog__panels">
      {tabs.map(([id]) => <div key={id} role="tabpanel" id={`catalog-panel-${id}`} aria-labelledby={`catalog-tab-${id}`} hidden={tab !== id}>{panel[id]}</div>)}
    </div>
    <footer className="nt-provider-catalog__actions">
      <div className="nt-provider-catalog__error" role={validationError ? "alert" : undefined}>{validationError}</div>
      <button className="nt-btn nt-btn--primary" type="submit" disabled={locked}>{t("添加", "Add")}</button>
    </footer>
  </section>;
}
