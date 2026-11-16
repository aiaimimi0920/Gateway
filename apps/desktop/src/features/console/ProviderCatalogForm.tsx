import { Button, Input, Notice, Panel, Tab, TabList, TabPanel, TabsRoot } from "@neuro/ui";
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
import { QwenEndpointFields } from "./QwenEndpointFields";
import { buildQwenEndpoint, emptyQwenEndpoint } from "./qwenEndpoint";
import { VertexTargetFields } from "./VertexTargetFields";
import { vertexTargetBase } from "./vertexOAuth";
import { kiroProfileTarget } from "./kiroOAuthCatalog";
import { cloudflareBase } from "./cloudflareWorkersAi";
import { CatalogFirstAccountFields } from "./CatalogFirstAccountFields";
import { SapAiCoreFields } from "./SapAiCoreFields";

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
  const allTabs = [["basics", t("基础与账号", "Basics & account")], ["request", t("调用配置", "Call settings")], ["mapping", t("模型映射", "Model mapping")],
  ["tests", t("自动测试", "Automatic tests")], ["pool", t("账号池", "Account pool")], ["health", t("站点健康", "Site health")]] as const;
  const tabs = allTabs.filter(([id]) => selectedTemplate.id !== "lumalabs-mcp" || id === "basics");
  const basics = (
    <div className="nt-provider-catalog__basics">
      <CatalogField name={t("名称", "Name")}><Input value={draft.providerLabel} disabled={locked} onChange={event => setDraft(current => ({ ...current, providerLabel: event.target.value }))} /></CatalogField>
      <CatalogField name="Provider ID"><Input value={draft.providerId} maxLength={256} disabled={locked}
        onChange={(event) => setDraft((current) => ({ ...current, providerId: event.target.value }))} /></CatalogField>
      <CatalogField name="Base URL"><Input type="url" value={draft.baseUrl} disabled={locked}
        readOnly={!selectedTemplate.custom && !["azure-api", "sap-aicore-chat"].includes(selectedTemplate.id)} spellCheck={false}
        onChange={(event) => setDraft((current) => ({ ...current, baseUrl: event.target.value }))} /></CatalogField>
      {selectedTemplate.id === "cloudflare-workers-ai" && <CatalogField name="Cloudflare Account ID"><Input
        value={draft.cloudflareAccountId ?? ""} disabled={locked} maxLength={32} spellCheck={false}
        onChange={(event) => { const cloudflareAccountId = event.target.value;
          setDraft((current) => ({ ...current, cloudflareAccountId, baseUrl: cloudflareBase(cloudflareAccountId) ?? "" })); }} /></CatalogField>}
      {selectedTemplate.id === "sap-aicore-chat" && <SapAiCoreFields draft={draft} setDraft={setDraft} disabled={locked} />}
      {selectedTemplate.preset === "qwen-official-api" && <div className="nt-provider-catalog__qwen"><QwenEndpointFields t={t} disabled={locked}
        value={draft.qwenEndpoint ?? emptyQwenEndpoint()} onChange={(qwenEndpoint) => setDraft((current) => ({
          ...current, qwenEndpoint, baseUrl: buildQwenEndpoint(qwenEndpoint) ?? "",
        }))} /></div>}
      {selectedTemplate.id === "vertex-oauth" && <VertexTargetFields value={draft.vertexTarget ?? { project: "", location: "" }} quota={false}
        disabled={locked} onChange={(vertexTarget) => setDraft((current) => ({ ...current, vertexTarget, baseUrl: vertexTargetBase(vertexTarget) ?? "" }))} />}
      {selectedTemplate.id === "kiro-oauth" && <CatalogField name="Profile ARN"><Input value={draft.kiroProfileArn ?? ""} disabled={locked}
        maxLength={2048} spellCheck={false} onChange={(event) => { const kiroProfileArn = event.target.value;
          setDraft((current) => ({ ...current, kiroProfileArn, baseUrl: kiroProfileTarget(kiroProfileArn) ?? "" })); }} /></CatalogField>}
      <CatalogFirstAccountFields template={selectedTemplate} draft={draft} setDraft={setDraft} locked={locked} t={t} />
    </div>
  );
  const panel = {
    basics,
    request: <ProviderCatalogRequestFields {...fields} template={selectedTemplate} draft={draft} setDraft={setDraft} />,
    mapping: <ProviderCatalogMappingFields key={draft.templateId} {...fields} models={draft.supportedModels} />,
    tests: <ProviderCatalogTestFields key={draft.templateId} {...fields} />,
    pool: <ProviderCatalogPoolFields key={draft.templateId} {...fields} draft={draft} setDraft={setDraft} drivers={drivers} />,
    health: <ProviderCatalogHealthFields {...fields} template={selectedTemplate} draft={draft} setDraft={setDraft} />,
  };
  return <Panel className="nt-provider-catalog__form" aria-label={t("服务商配置", "Provider configuration")}>
    <TabsRoot className="nt-provider-catalog__tab-root" value={tab} onValueChange={next => setTab(next as CatalogTab)}>
      <TabList className="nt-provider-catalog__tabs" label={t("服务商标准配置", "Provider configuration tabs")}>
        {tabs.map(([id, name]) => <Tab key={id} value={id}>{name}</Tab>)}
      </TabList>
      <div className="nt-provider-catalog__panels">
        {tabs.map(([id]) => <TabPanel key={id} value={id} forceMount hidden={tab !== id}>{panel[id]}</TabPanel>)}
      </div>
    </TabsRoot>
    <footer className="nt-provider-catalog__actions">
      <div className="nt-provider-catalog__error">{validationError && <Notice tone="danger">{validationError}</Notice>}</div>
      <Button variant="primary" type="submit" disabled={locked}>{t("添加", "Add")}</Button>
    </footer>
  </Panel>;
}
