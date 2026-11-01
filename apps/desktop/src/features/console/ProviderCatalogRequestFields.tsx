import type { Dispatch, SetStateAction } from "react";
import type { ProviderCatalogDraft, ProviderCatalogTemplate } from "./providerCatalog";
import { CALL_ADAPTERS, SPECIAL_ADAPTERS, type CallAdapter, type SpecialAdapter } from "./providerCatalogWorkflow";
import { REQUEST_CLIENT_PROFILES, type RequestClientProfile } from "./requestClientProfile";
import { CatalogField, type CatalogFieldsProps } from "./ProviderCatalogFields";
import type { RecoveryWaitChoice } from "./requestWait";

export function ProviderCatalogRequestFields({ t, workflow, update, locked, template, draft, setDraft }: CatalogFieldsProps & {
  template: ProviderCatalogTemplate; draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>;
}) {
  const fixed = !template.custom;
  const managed = workflow.specialAdapter !== "none";
  const fingerprint = draft.requestClientProfile ?? "auto";
  const fixedCall = template.id === "chatgpt" ? "OpenAI Responses" : template.compatibility === "search" ? t("搜索 API", "Search API")
    : managed ? t("专用通道管理", "Managed by specialized adapter")
      : template.id === "openai" || template.compatibility === "openai" ? "OpenAI API" : t(CALL_ADAPTERS[workflow.callAdapter][0], CALL_ADAPTERS[workflow.callAdapter][1]);
  return <div className="nt-catalog-settings-grid">
    <CatalogField name={t("调用适配器", "Call adapter")}>
      {fixed || managed ? <input className="nt-input" readOnly value={fixedCall} /> :
        <select className="nt-select" value={workflow.callAdapter} disabled={locked} onChange={(event) => update({ callAdapter: event.target.value as CallAdapter })}>
          {Object.entries(CALL_ADAPTERS).map(([id, labels]) => <option key={id} value={id}>{t(labels[0], labels[1])}</option>)}
        </select>}
    </CatalogField>
    {template.compatibility !== "search" && <CatalogField name={t("恢复等待", "Recovery waiting")}>
      <select className="nt-select" disabled={locked} value={draft.recoveryWait ?? "auto"}
        onChange={(event) => setDraft((current) => ({ ...current, recoveryWait: event.target.value as RecoveryWaitChoice }))}>
        <option value="auto">{t("自动", "Automatic")}</option>
        <option value="300">{t("5 分钟", "5 minutes")}</option>
        <option value="600">{t("10 分钟", "10 minutes")}</option>
        <option value="900">{t("15 分钟", "15 minutes")}</option>
        <option value="client_connected">{t("连接期间持续等待", "Wait while connected")}</option>
        <option value="disabled">{t("快速失败", "Fail fast")}</option>
      </select>
    </CatalogField>}
    <CatalogField name={t("指纹适配器", "Fingerprint adapter")}>
      {fixed || managed ? <input className="nt-input" readOnly value={workflow.specialAdapter === "antigravity_oauth" ? "Antigravity" : managed ? t("站点会话指纹", "Site session profile") : fingerprint === "codex_cli" ? "Codex" : t("自动", "Automatic")} /> :
        <select className="nt-select" value={fingerprint} disabled={locked} onChange={(event) => setDraft((current) => ({ ...current, requestClientProfile: event.target.value as RequestClientProfile }))}>
          {REQUEST_CLIENT_PROFILES.map((id) => <option key={id} value={id}>{id === "auto" ? t("自动", "Automatic") : id}</option>)}
        </select>}
    </CatalogField>
    <CatalogField name={t("专用适配器", "Specialized adapter")} wide>
      {fixed ? <input className="nt-input" readOnly value={t(SPECIAL_ADAPTERS[workflow.specialAdapter][0], SPECIAL_ADAPTERS[workflow.specialAdapter][1])} /> :
        <select className="nt-select" value={workflow.specialAdapter} disabled={locked} onChange={(event) => update({ specialAdapter: event.target.value as SpecialAdapter })}>
          {Object.entries(SPECIAL_ADAPTERS).map(([id, labels]) => <option key={id} value={id}
            disabled={id === "basispoints" && draft.baseUrl.replace(/\/$/, "") !== "https://chatgpt.com/backend-api/codex"}>{t(labels[0], labels[1])}</option>)}
        </select>}
    </CatalogField>
    {template.id === "chatgpt" && <CatalogField name={t("Basispoints 通道", "Basispoints channel")}>
      <select className="nt-select" disabled={locked} value={workflow.specialAdapter} onChange={(event) => update({ specialAdapter: event.target.value as SpecialAdapter })}>
        <option value="none">{t("关闭", "Off")}</option><option value="basispoints">{t("开启", "On")}</option>
      </select>
    </CatalogField>}
  </div>;
}
