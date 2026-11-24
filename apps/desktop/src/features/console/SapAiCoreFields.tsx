import { Checkbox, Input } from "@neuro/ui";
import { type Dispatch, type SetStateAction, useRef } from "react";
import type { DiscoverSapDeployments } from "../../api/sapAiCoreDeployments";
import { SapAiCoreDeploymentPicker } from "./SapAiCoreDeploymentPicker";
import { CatalogField } from "./ProviderCatalogFields";
import type { ProviderCatalogDraft } from "./providerCatalog";
import { SAP_IMAGE_API_VERSION, SAP_O3_MINI_API_VERSION, sapEndpoint, setCatalogDeploymentUrl, validSapTarget } from "./sapAiCore";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
/** Only deployment routing fields belong to the pool; secrets are entered on the account. */
export function SapAiCoreFields({ draft, setDraft, disabled, discover, hasSecretAccess, onRequestSecretAccess }: {
  draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>; disabled: boolean;
  discover?: DiscoverSapDeployments; hasSecretAccess: boolean; onRequestSecretAccess(): void;
}) {
  const picked = useRef<string | null>(null);
  const target = draft.sapTarget ?? { resourceGroup: "", apiVersion: "" };
  const { t } = useUiLocale();
  const orchestration = target.orchestrationModel !== undefined;
  const bedrock = target.bedrockConverse === true;
  const lookupProtocol = orchestration ? "orchestration" : bedrock ? "bedrock" : "azure";
  function selectMode(mode: "azure" | "orchestration" | "bedrock") {
    const previous = picked.current;
    picked.current = null;
    setDraft((current) => ({ ...current,
      baseUrl: previous === current.baseUrl ? "" : current.baseUrl,
      sapTarget: { resourceGroup: target.resourceGroup, apiVersion: target.apiVersion,
        orchestrationModel: mode === "orchestration" ? "" : undefined,
        bedrockConverse: mode === "bedrock" ? true : undefined } }));
  }
  return <>
    <CatalogField name="AI-Resource-Group"><Input value={target.resourceGroup} disabled={disabled} maxLength={253}
      onChange={(event) => {
        const group = event.target.value;
        const previous = picked.current; picked.current = null;
        setDraft((current) => ({ ...current, baseUrl: previous === current.baseUrl ? "" : current.baseUrl,
          sapTarget: { ...target, resourceGroup: group, imageInputDeployment: undefined, o3MiniDeployment: undefined, nativeResponses: undefined, nativeResponsesImageInput: undefined, storedResponses: undefined, bedrockImageInput: undefined, orchestrationImageInput: undefined, orchestrationReasoning: undefined,
            orchestrationModel: orchestration ? "" : undefined } }));
      }} /></CatalogField>
    <Checkbox label={t("使用 Orchestration v2", "Use Orchestration v2")} checked={orchestration} disabled={disabled}
      onChange={(event) => selectMode(event.target.checked ? "orchestration" : "azure")} />
    <Checkbox label={t("使用 Bedrock Converse", "Use Bedrock Converse")} checked={bedrock} disabled={disabled}
      onChange={(event) => selectMode(event.target.checked ? "bedrock" : "azure")} />
    {bedrock ? <Checkbox label={t("启用 Bedrock 图片输入", "Enable Bedrock image input")}
      checked={target.bedrockImageInput === true}
      disabled={disabled || !validSapTarget(target) || !sapEndpoint(draft.baseUrl)}
      onChange={(event) => {
        const checked = event.target.checked;
        setDraft((current) => ({ ...current, sapTarget: { ...target, bedrockImageInput: checked } }));
      }} /> : orchestration ? <><CatalogField name={t("Orchestration 模型", "Orchestration model")}><Input value={target.orchestrationModel}
      disabled={disabled} maxLength={128} onChange={(event) => setDraft((current) => ({ ...current,
        sapTarget: { ...target, orchestrationModel: event.target.value, orchestrationImageInput: undefined, orchestrationReasoning: undefined } }))} /></CatalogField>
      <Checkbox label={t("启用 Orchestration 图片输入", "Enable Orchestration image input")}
        checked={target.orchestrationImageInput === true}
        disabled={disabled || !validSapTarget(target) || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current, sapTarget: { ...target, orchestrationImageInput: checked } }));
        }} />
      <Checkbox label={t("启用 Orchestration 推理", "Enable Orchestration reasoning")}
        checked={target.orchestrationReasoning === true}
        disabled={disabled || !validSapTarget(target) || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current, sapTarget: { ...target, orchestrationReasoning: checked } }));
        }} />
    </> : <>
      <CatalogField name="API Version"><Input value={target.apiVersion} disabled={disabled} maxLength={18} placeholder={SAP_IMAGE_API_VERSION}
        onChange={(event) => setDraft((current) => ({ ...current,
          sapTarget: { ...target, apiVersion: event.target.value, imageInputDeployment: undefined, o3MiniDeployment: undefined, nativeResponses: undefined, nativeResponsesImageInput: undefined, storedResponses: undefined } }))} /></CatalogField>
      <Checkbox label={t("启用 GPT-4o 图片输入", "Enable GPT-4o image input")}
        checked={target.imageInputDeployment === draft.baseUrl}
        disabled={disabled || target.apiVersion !== SAP_IMAGE_API_VERSION || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current,
            sapTarget: { ...target, imageInputDeployment: checked ? current.baseUrl : undefined } }));
        }} />
      <Checkbox label={t("启用 o3-mini 推理参数", "Enable o3-mini reasoning parameters")}
        checked={target.o3MiniDeployment === draft.baseUrl}
        disabled={disabled || target.apiVersion !== SAP_O3_MINI_API_VERSION || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current,
            sapTarget: { ...target, o3MiniDeployment: checked ? current.baseUrl : undefined } }));
        }} />
      <Checkbox label={t("启用原生无状态 Responses", "Enable native stateless Responses")}
        checked={target.nativeResponses === true}
        disabled={disabled || !validSapTarget(target) || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current, sapTarget: { ...target, nativeResponses: checked, nativeResponsesImageInput: undefined, storedResponses: undefined } }));
        }} />
      <Checkbox label={t("启用原生 Responses 图片输入", "Enable native Responses image input")}
        checked={target.nativeResponsesImageInput === true}
        disabled={disabled || !target.nativeResponses || !validSapTarget(target) || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current, sapTarget: { ...target, nativeResponsesImageInput: checked } }));
        }} />
      <Checkbox label={t("启用持久化 Responses", "Enable stored Responses")}
        checked={target.storedResponses === true}
        disabled={disabled || !target.nativeResponses || !validSapTarget(target) || !sapEndpoint(draft.baseUrl)}
        onChange={(event) => {
          const checked = event.target.checked;
          setDraft((current) => ({ ...current, sapTarget: { ...target, storedResponses: checked } }));
        }} />
    </>}
    {discover && <SapAiCoreDeploymentPicker key={`${lookupProtocol}:${target.resourceGroup}`}
      resourceGroup={target.resourceGroup} protocol={lookupProtocol} baseUrl={draft.baseUrl}
      disabled={disabled} hasSecretAccess={hasSecretAccess} onRequestSecretAccess={onRequestSecretAccess} discover={discover}
      select={(baseUrl) => { picked.current = baseUrl; setDraft((current) => setCatalogDeploymentUrl(current, baseUrl)); }} />}
  </>;
}
