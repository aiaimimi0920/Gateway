import { CheckboxGroup, Field, Input, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { kiroScopes, type KiroDirectory, type KiroScope } from "../../api/console/kiro-auth";

export type KiroOAuthFieldsValue = { authMethod: "" | KiroDirectory["authMethod"]; directoryRegion: string; startUrl: string; scopes: KiroScope[]; machineId: string };
export const emptyKiroOAuthFields: KiroOAuthFieldsValue = { authMethod: "", directoryRegion: "", startUrl: "", scopes: [], machineId: "" };
function validStart(value: string) {
  if (!value || value.length > 4096 || !/^[\x21-\x7e]+$/.test(value)) return false;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.port && !url.username && !url.password && !url.search && !value.includes("#")
      && /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.awsapps\.com$/.test(url.hostname) && ["/start", "/start/"].includes(url.pathname);
  } catch { return false; }
}
export function validKiroOAuthFields(value: KiroOAuthFieldsValue, editing: boolean) {
  const machine = value.machineId.trim();
  return (value.authMethod === "builder_id" || value.authMethod === "identity_center" && /^[a-z]{2}(?:-[a-z]+)+-\d$/.test(value.directoryRegion) && validStart(value.startUrl))
    && value.scopes.length > 0 && value.scopes.length <= 5 && new Set(value.scopes).size === value.scopes.length && value.scopes.every((scope) => kiroScopes.includes(scope))
    && (editing || /^[a-f\d]{64}$/i.test(machine) || /^[a-f\d]{32}$/i.test(machine.replaceAll("-", "")));
}
export function kiroDirectory(value: KiroOAuthFieldsValue): KiroDirectory {
  if (value.authMethod === "builder_id") return { authMethod: "builder_id" };
  return { authMethod: "identity_center", directoryRegion: value.directoryRegion, startUrl: value.startUrl };
}
export function KiroOAuthFields({ value, disabled, editing, onChange }: {
  value: KiroOAuthFieldsValue; disabled: boolean; editing: boolean; onChange(value: KiroOAuthFieldsValue): void;
}) {
  const { t } = useUiLocale();
  return <fieldset className="nt-credential-fields" disabled={disabled}>
    <Field label={t("授权目录", "Authorization directory")}>
      <Select value={value.authMethod} onChange={(event) => onChange({ ...value, authMethod: event.target.value as KiroOAuthFieldsValue["authMethod"] })}>
        <option value="">{t("请选择目录", "Select a directory")}</option>
        <option value="builder_id">AWS Builder ID</option><option value="identity_center">IAM Identity Center</option>
      </Select>
    </Field>
    {value.authMethod === "identity_center" && <>
      <Field label="Directory Region"><Input autoComplete="off" maxLength={32} value={value.directoryRegion} onChange={(event) => onChange({ ...value, directoryRegion: event.target.value })} /></Field>
      <Field label="Start URL"><Input autoComplete="off" maxLength={4096} value={value.startUrl} onChange={(event) => onChange({ ...value, startUrl: event.target.value })} /></Field>
    </>}
    <CheckboxGroup label={t("授权范围", "Authorization scopes")} orientation="vertical" values={value.scopes}
      options={kiroScopes.map((scope) => ({ value: scope, label: scope }))}
      onValuesChange={(scopes) => onChange({ ...value, scopes: scopes as KiroScope[] })} />
    {editing ? <p className="n-muted">{t("重授权沿用此账号的固定 Machine ID。", "Reauthorization retains this account's fixed Machine ID.")}</p>
      : <Field label="Machine ID"><Input autoComplete="off" maxLength={128} placeholder="64 hex / UUID / 32 hex" value={value.machineId} onChange={(event) => onChange({ ...value, machineId: event.target.value })} /></Field>}
  </fieldset>;
}
