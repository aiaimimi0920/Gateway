import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { Disclosure, Field, Input, Select, Stack } from "@neuro/ui";
import type { GrokPrincipal } from "../../api/console/grok-auth";

export type GrokOAuthFieldsValue = {
  clientId: string; scopes: string; redirectUri: string;
  selectionKind: GrokPrincipal["kind"]; selectionId: string;
  requiredKind: GrokPrincipal["kind"]; requiredId: string;
};
export const emptyGrokOAuthFields: GrokOAuthFieldsValue = {
  clientId: "", scopes: "", redirectUri: "", selectionKind: "Team", selectionId: "", requiredKind: "Team", requiredId: "",
};

export function GrokOAuthFields({ value, disabled, onChange, device = false }: {
  value: GrokOAuthFieldsValue; disabled: boolean; onChange(value: GrokOAuthFieldsValue): void; device?: boolean;
}) {
  const { t } = useUiLocale();
  const field = (key: "clientId" | "scopes" | "redirectUri" | "selectionId" | "requiredId", label: string, placeholder?: string) =>
    <Field label={label}><Input autoComplete="off"
      value={value[key]} maxLength={key === "scopes" ? 2048 : 512} disabled={disabled} placeholder={placeholder}
      onChange={(event) => onChange({ ...value, [key]: event.target.value })} /></Field>;
  return <fieldset className="nt-credential-fields" disabled={disabled}>
    <legend>{t("Grok Build 授权参数", "Grok Build authorization parameters")}</legend>
    {field("clientId", "OAuth Client ID")}
    {field("scopes", t("Scopes（空格分隔）", "Scopes (space-separated)"))}
    {!device && field("redirectUri", "Redirect URI", "http://127.0.0.1:34567/callback")}
    <p className="n-muted">{t("请填写有权用于此集成的客户端与 scopes；不内置或借用官方 CLI 身份。代码授权需手动粘贴回调；设备授权不启动本地监听器。", "Use a client and scopes permitted for this integration. No official CLI identity is bundled. Code flow uses a manual callback; device flow starts no local listener.")}</p>
    <Disclosure title={t("可选主体约束", "Optional principal constraints")}>
      <Stack>
        <Field label={t("Consent 预选类型", "Consent selection type")}>
          <Select value={value.selectionKind} onChange={(event) => onChange({ ...value, selectionKind: event.target.value as GrokPrincipal["kind"] })}>
            <option value="Team">Team</option><option value="User">User</option>
          </Select>
        </Field>
        {!device && field("selectionId", t("Consent 预选 ID（仅提示）", "Consent selection ID (hint only)"))}
        {device && value.selectionId && <p role="alert">{t("设备授权不接受 Consent 预选；请切回代码授权并清空该字段。", "Device flow does not accept consent selection; switch back to code flow and clear it.")}</p>}
        <Field label={t("强制匹配类型", "Required principal type")}>
          <Select value={value.requiredKind} onChange={(event) => onChange({ ...value, requiredKind: event.target.value as GrokPrincipal["kind"] })}>
            <option value="Team">Team</option><option value="User">User</option>
          </Select>
        </Field>
        {field("requiredId", t("强制匹配 ID（独立约束）", "Required principal ID (independent constraint)"))}
        <p className="n-muted">{t("预选不会自动成为强制约束；未验签的主体提示不是已验证用户身份。", "Selection does not imply a required principal. Unsigned principal hints are not verified user identity.")}</p>
      </Stack>
    </Disclosure>
  </fieldset>;
}
