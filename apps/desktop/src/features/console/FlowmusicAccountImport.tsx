import { useEffect, useState } from "react";
import { Button, Field, Input, Notice, Select, Textarea } from "@neuro/ui";
import type { ConsoleGeminiAuthFamily } from "../../api/contracts";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

type Props = {
  providerId: string; disabled: boolean; hasSecretAccess: boolean;
  onRequestSecretAccess: () => void;
  onStart: (family: ConsoleGeminiAuthFamily, id: string, label?: string) => void;
  onImportJson: (family: ConsoleGeminiAuthFamily, id: string, label: string | undefined, source: string) => void;
};

/** A distinct Supabase contract: never accept Google cookies or a full browser storageState. */
export function FlowmusicAccountImport({ providerId, disabled, hasSecretAccess, onRequestSecretAccess, onStart, onImportJson }: Props) {
  const { t } = useUiLocale();
  const [label, setLabel] = useState("");
  const [method, setMethod] = useState("browser");
  const [source, setSource] = useState("");
  const [error, setError] = useState("");
  useEffect(() => { setLabel(""); setMethod("browser"); setSource(""); setError(""); }, [providerId]);
  useEffect(() => { if (!hasSecretAccess) setSource(""); }, [hasSecretAccess]);
  return <section className="n-stack" aria-label="FlowMusic 账号导入">
    <Field label={t("账号名称", "Account name")}>
      <Input maxLength={128} disabled={disabled} value={label} autoComplete="off" onChange={e => setLabel(e.currentTarget.value)} /></Field>
    <Field label={t("录入方式", "Import method")}>
      <Select value={method} disabled={disabled} onChange={e => { setMethod(e.currentTarget.value); setSource(""); setError(""); }}>
        <option value="browser">{t("网页登录", "Browser sign-in")}</option>
        <option value="json">{t("FlowMusic 会话 JSON", "FlowMusic session JSON")}</option>
      </Select></Field>
    {method === "browser" ? <Button variant="primary" wrap disabled={disabled}
      onClick={() => onStart("flowmusic-web", providerId, label.trim() || undefined)}>{t("FlowMusic 网页登录并导入", "FlowMusic sign in and import")}</Button>
      : !hasSecretAccess ? <Button disabled={disabled} onClick={onRequestSecretAccess}>{t("确认敏感信息访问权限", "Confirm secret access")}</Button>
        : <>
          <Field label="FlowMusic session JSON">
            <Textarea rows={6} value={source} disabled={disabled} autoComplete="off" spellCheck={false}
              placeholder={'{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"...","refreshToken":"..."}'}
              onChange={e => { setSource(e.currentTarget.value); setError(""); }} /></Field>
          {error && <Notice tone="danger">{error}</Notice>}
          <Button variant="primary" disabled={disabled || !source.trim()} onClick={() => {
            if (new TextEncoder().encode(source).length > 24 * 1024) { setError("FlowMusic JSON 不能超过 24 KiB。"); return; }
            try { JSON.parse(source); } catch { setError("JSON 格式无效。"); return; }
            onImportJson("flowmusic-web", providerId, label.trim() || undefined, source); setSource("");
          }}>{t("导入 JSON", "Import JSON")}</Button>
        </>}
  </section>;
}
