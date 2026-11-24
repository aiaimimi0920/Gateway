import { Field, Input, Notice, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { BedrockMaterialField } from "./BedrockMaterialField";
import { ChatgptWebSessionImport } from "./ChatgptWebSessionImport";
import { CredentialModelEditors } from "./CredentialModelEditors";
import { CredentialSecretField } from "./CredentialSecretField";
import { ProviderSessionMaterialField } from "./ProviderSessionMaterialField";
import { QwenCredentialEndpoint } from "./QwenCredentialEndpoint";
import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import type { CredentialDialogProps } from "./credentialDialogTypes";
import { editModelSources } from "./credentialModelSelection";
import { clearQwenEvidence } from "./qwenCredentialDraft";
import { sapEndpoint } from "./sapAiCore";
import type { CredentialDialogState } from "./useCredentialDialog";

/** Account-specific fields and secret audience transitions remain owned by Gateway. */
export function CredentialAccountFields({ model, ...props }: CredentialDialogProps & { model: CredentialDialogState }) {
  const { t } = useUiLocale();
  const { open, mode, locked, providerOptions, supportsBasispoints, initialValue, hasSecretAccess,
    onCaptureWebBrowser, onRequestSecretAccess, onRevealKey } = props;
  const { value, setValue, authentication, manualCodex, qwen, canFetch, fetchModels, busy, catalog, official, customCatalog,
    webSession, managedWeb, productSession, bedrock, bedrockMaterial, manual } = model;
  return <fieldset className="nt-credential-fields" disabled={locked || busy}>
    <Field label="Provider">
      <Select compact value={value.providerId} disabled={locked || mode === "edit"} onChange={(event) => {
        const providerId = event.currentTarget.value;
        setValue((current) => ({
          ...current, providerId, accountName: "", baseUrl: "", qwenEndpoint: undefined,
          apiKeyValue: "", webSessionMaterial: undefined, sessionMaterial: undefined, bedrockMaterial: undefined,
          chatgptAccountId: "", chatgptGroup: undefined, apiKeyOperation: "replace", discovery: undefined,
          commandcodeZdr: undefined,
          modelCatalog: undefined, supportedModelsText: "", fetchedModelsText: "", manualModelsText: ""
        }));
      }}>
        <option value="">{t("请选择 Provider", "Select provider")}</option>
        {providerOptions.map((provider) => <option value={provider.id} key={provider.id}>{provider.label}</option>)}
      </Select>
    </Field>
    {!authentication && <>
      {props.catalogProviderKind?.(value.providerId) === "commandcode" && <Field label={t("零数据保留（ZDR）", "Zero data retention (ZDR)")}>
        <Select compact value={value.commandcodeZdr ?? "inherit"} onChange={(event) => setValue((current) => ({ ...current, commandcodeZdr: event.target.value === "required" ? "required" : "inherit" }))}>
          <option value="inherit">{t("继承服务商设置", "Inherit provider setting")}</option>
          <option value="required">{t("强制开启", "Required")}</option>
        </Select>
      </Field>}
      {supportsBasispoints?.(value.providerId) && <Field label={t("订阅分组", "Subscription group")}>
        <Select compact value={value.chatgptGroup ?? "free"} onChange={(event) => setValue((current) => ({ ...current, chatgptGroup: event.target.value }))}>
          {CHATGPT_POOL_CATEGORIES.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}
        </Select>
      </Field>}
      {manualCodex && <Field label="ChatGPT Account ID">
        <Input compact autoComplete="off" value={value.chatgptAccountId ?? ""} onChange={(event) => setValue((current) => ({ ...current, chatgptAccountId: event.target.value }))} />
      </Field>}
      {supportsBasispoints?.(value.providerId) && <Field label={t("Codex 调用通道", "Codex channel")}>
        <Select compact value={value.codexBasispointsEnabled === undefined ? "inherit" : value.codexBasispointsEnabled ? "basispoints" : "native"}
          onChange={(event) => setValue((current) => ({ ...current, codexBasispointsEnabled: event.target.value === "inherit" ? undefined : event.target.value === "basispoints" }))}>
          <option value="inherit">{t("继承服务商设置", "Inherit provider setting")}</option>
          <option value="native">{t("原生 Codex", "Native Codex")}</option><option value="basispoints">Basispoints</option>
        </Select>
      </Field>}
      <Field label={t("账号 ID", "Account ID")}>
        <Input compact value={value.credentialId} disabled={locked || mode === "edit"} onChange={(event) => {
          const credentialId = event.currentTarget.value; setValue((current) => ({ ...current, credentialId }));
        }} />
      </Field>
      {qwen && <QwenCredentialEndpoint provider={qwen} value={value} onChange={setValue} disabled={locked || busy} t={t} />}
      {model.sap ? <Field label={t("部署 ID", "Deployment ID")}><Input compact readOnly value={sapEndpoint(String(model.sap.base_url ?? ""))?.deployment ?? ""} /></Field> : <CredentialModelEditors value={value} disabled={locked || busy} canFetch={canFetch && value.apiKeyOperation !== "clear"}
        manualOnly={Boolean(model.express || model.vertex || model.lumaAgents)} busy={catalog.busy || official.busy || customCatalog.busy} onFetch={fetchModels} onChange={setValue} />}
      {open && webSession && <ChatgptWebSessionImport key={`web:${value.providerId}:${initialValue?.credentialId ?? ""}`}
        capture={onCaptureWebBrowser} providerId={value.providerId} disabled={locked || busy || !hasSecretAccess}
        loaded={Boolean(value.webSessionMaterial)}
        onClear={() => setValue((current) => ({ ...current, webSessionMaterial: undefined, apiKeyValue: "", apiKeyOperation: mode === "edit" ? "keep" : "replace" }))}
        onMaterial={(webSessionMaterial, accountName) => setValue((current) => {
          const imported = { ...current, webSessionMaterial, accountName: current.accountName || accountName || "", apiKeyOperation: "replace" as const, apiKeyValue: webSessionMaterial.accessToken };
          return webSessionMaterial.modelSlugs?.length ? editModelSources(imported, webSessionMaterial.modelSlugs.join("\n"), current.manualModelsText ?? "") : imported;
        })} />}
      {open && managedWeb && <Notice tone="success">{t("网页登录凭据已保存", "Web sign-in credentials saved")}</Notice>}
      {open && productSession && <ProviderSessionMaterialField key={`session:${mode}:${value.providerId}:${initialValue?.credentialId ?? ""}`}
        product={productSession} resetKey={initialValue} loaded={Boolean(value.sessionMaterial)} disabled={locked || busy || !hasSecretAccess}
        onClear={() => setValue((current) => ({ ...current, sessionMaterial: undefined, apiKeyValue: "", apiKeyOperation: mode === "edit" ? "keep" : "replace" }))}
        onMaterial={(sessionMaterial) => setValue((current) => ({ ...current, sessionMaterial, apiKeyValue: "", apiKeyOperation: "replace" }))} />}
      {open && bedrock && bedrockMaterial && <BedrockMaterialField value={bedrockMaterial} keeping={value.apiKeyOperation === "keep"}
        disabled={locked || busy || !hasSecretAccess} onReplace={() => setValue((current) => ({ ...current, apiKeyOperation: "replace" }))}
        onChange={(material) => setValue((current) => ({ ...current, bedrockMaterial: material, apiKeyOperation: "replace" }))} />}
      {open && !props.reauthenticationOnly?.(value.providerId) && !webSession && !managedWeb && !productSession && !bedrock && <CredentialSecretField key={`${value.providerId}:${value.credentialId}`}
        label={model.sap ? "Service Key JSON" : manual?.secretLabel} value={value.apiKeyValue} existing={mode === "edit" && value.apiKeyOperation === "keep"}
        disabled={locked || busy} hasSecretAccess={hasSecretAccess} onRequestSecretAccess={onRequestSecretAccess} onReveal={onRevealKey}
        onLoaded={(apiKeyValue) => setValue((current) => ({ ...current, apiKeyValue }))}
        onChange={(apiKeyValue) => setValue((current) => {
          const next = { ...current, apiKeyValue, apiKeyOperation: apiKeyValue ? "replace" as const : mode === "edit" ? "clear" as const : "keep" as const, discovery: undefined, modelCatalog: undefined };
          return qwen ? clearQwenEvidence(next) : next;
        })} />}
    </>}
  </fieldset>;
}
