import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { Field, Input, Select } from "@neuro/ui";
import type { AccioAuthInput } from "../../api/console/accio-auth";

export type AccioOAuthFieldsValue = { clientId: string; loginSite: "" | AccioAuthInput["loginSite"]; callbackPort: string; utdid: string };
export const emptyAccioOAuthFields: AccioOAuthFieldsValue = { clientId: "", loginSite: "", callbackPort: "", utdid: "" };
const atom = (value: string, limit: number) => value.length > 0 && value.length <= limit && /^[\x21-\x7e]+$/.test(value);
export function validAccioOAuthFields(value: AccioOAuthFieldsValue, editing: boolean) {
  return atom(value.clientId, 512) && (value.loginSite === "china" || value.loginSite === "global")
    && /^\d{1,5}$/.test(value.callbackPort) && Number(value.callbackPort) >= 1 && Number(value.callbackPort) <= 65535
    && (editing && value.utdid === "" || atom(value.utdid, 1024));
}

export function AccioOAuthFields({ value, disabled, editing, onChange }: {
  value: AccioOAuthFieldsValue; disabled: boolean; editing: boolean; onChange(value: AccioOAuthFieldsValue): void;
}) {
  const { t } = useUiLocale();
  return <fieldset className="nt-credential-fields" disabled={disabled}>
    <Field label="Client ID">
      <Input autoComplete="off" value={value.clientId} maxLength={512}
        onChange={(event) => onChange({ ...value, clientId: event.target.value })} />
    </Field>
    <p className="n-muted">{t("请提供获准使用的 client；不内置或借用第一方 client 身份。", "Supply a client you are permitted to use; no first-party client identity is bundled or borrowed.")}</p>
    <Field label={t("授权站点", "Authorization site")}>
      <Select value={value.loginSite} onChange={(event) => onChange({ ...value, loginSite: event.target.value as AccioOAuthFieldsValue["loginSite"] })}>
        <option value="">{t("请选择站点", "Select a site")}</option>
        <option value="china">China — www.accio-ai.com</option><option value="global">Global — zh.accio.com</option>
      </Select>
    </Field>
    <Field label={t("回调端口", "Callback port")}>
      <Input inputMode="numeric" autoComplete="off" value={value.callbackPort} maxLength={5}
        onChange={(event) => onChange({ ...value, callbackPort: event.target.value })} />
    </Field>
    <p className="n-muted">{t("端口范围 1–65535；仅使用 http://127.0.0.1:<port>/auth/callback，没有自动监听。", "Port range 1–65535; only http://127.0.0.1:<port>/auth/callback is used, with no automatic listener.")}</p>
    <Field label="utdid">
      <Input type="password" autoComplete="off" value={value.utdid} maxLength={1024}
        onChange={(event) => onChange({ ...value, utdid: event.target.value })} />
    </Field>
    <p className="n-muted">{editing
      ? t("留空仅保留此账号自身已配置且无歧义的设备值；不借用服务商设备。", "Leave empty only to retain this account's own unambiguous device value, never the provider's device.")
      : t("请输入已有设备值：1–1024 个非空格可打印 ASCII 字符；不会生成或 trim。", "Enter an existing device value: 1–1024 printable non-space ASCII characters; it will not be generated or trimmed.")}</p>
  </fieldset>;
}
