import { useEffect, useState } from "react";
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
  return <section className="nt-stack" aria-label="FlowMusic 账号导入">
    <label className="nt-credential-row"><span>{t("账号名称", "Account name")}</span>
      <input className="nt-input" maxLength={128} disabled={disabled} value={label} autoComplete="off" onChange={e => setLabel(e.currentTarget.value)} /></label>
    <label className="nt-credential-row"><span>{t("录入方式", "Import method")}</span>
      <select className="nt-select" value={method} disabled={disabled} onChange={e => { setMethod(e.currentTarget.value); setSource(""); setError(""); }}>
        <option value="browser">{t("网页登录", "Browser sign-in")}</option>
        <option value="json">{t("FlowMusic 会话 JSON", "FlowMusic session JSON")}</option>
      </select></label>
    {method === "browser" ? <button type="button" className="nt-btn nt-btn--primary" disabled={disabled}
      onClick={() => onStart("flowmusic-web", providerId, label.trim() || undefined)}>{t("FlowMusic 网页登录并导入", "FlowMusic sign in and import")}</button>
      : !hasSecretAccess ? <button type="button" className="nt-btn" disabled={disabled} onClick={onRequestSecretAccess}>{t("确认敏感信息访问权限", "Confirm secret access")}</button>
      : <>
        <label className="nt-credential-row"><span>FlowMusic session JSON</span>
          <textarea className="nt-input" rows={6} value={source} disabled={disabled} autoComplete="off" spellCheck={false}
            placeholder={'{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"...","refreshToken":"..."}'}
            onChange={e => { setSource(e.currentTarget.value); setError(""); }} /></label>
        {error && <p role="alert">{error}</p>}
        <button type="button" className="nt-btn nt-btn--primary" disabled={disabled || !source.trim()} onClick={() => {
          if (new TextEncoder().encode(source).length > 24 * 1024) { setError("FlowMusic JSON 不能超过 24 KiB。"); return; }
          try { JSON.parse(source); } catch { setError("JSON 格式无效。"); return; }
          onImportJson("flowmusic-web", providerId, label.trim() || undefined, source); setSource("");
        }}>{t("导入 JSON", "Import JSON")}</button>
      </>}
  </section>;
}
