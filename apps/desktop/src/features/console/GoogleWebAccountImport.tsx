import { useEffect, useState } from "react";
import type { ConsoleGeminiAuthFamily } from "../../api/contracts";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { GoogleWebJsonField } from "./GoogleWebJsonField";

/** Match the persisted preset, not the user-editable provider ID or brand label. */
export function googleWebAuthFamily(preset: unknown): ConsoleGeminiAuthFamily | null {
  if (preset === "aistudio-web-reverse") return "aistudio-web";
  if (preset === "flow-web") return "flow-web";
  if (preset === "stitch-web") return "stitch-web";
  if (["gemini-canvas", "gemini-canvas-browser-relay", "gemini-canvas-program-relay"].includes(String(preset))) return "gemini-canvas";
  if (preset === "gemini-canvas-chat") return "gemini-canvas-chat";
  if (preset === "gemini-web-chat" || preset === "gemini-web-chat-modular") return "gemini-web";
  return null;
}

type Props = {
  preset: unknown;
  providerId: string;
  disabled: boolean;
  onStart: (family: ConsoleGeminiAuthFamily, providerId: string, accountLabel?: string) => void;
  onImportJson?: (family: ConsoleGeminiAuthFamily, providerId: string, accountLabel: string | undefined, source: string) => void;
  hasSecretAccess?: boolean;
  onRequestSecretAccess?: () => void;
};

export function GoogleWebAccountImport({ preset, providerId, disabled, onStart, onImportJson, hasSecretAccess, onRequestSecretAccess }: Props) {
  const { t } = useUiLocale();
  const [label, setLabel] = useState("");
  const [method, setMethod] = useState("browser");
  useEffect(() => { setLabel(""); setMethod("browser"); }, [providerId, preset]);
  const family = googleWebAuthFamily(preset);
  if (!family) return null;
  const name = family === "stitch-web" ? "Google Stitch" : family === "flow-web" ? "Google Flow" : family === "aistudio-web" ? "AI Studio" : family.startsWith("gemini-canvas") ? "Gemini Canvas" : "Gemini Web";
  return <section className="nt-stack" aria-label={`${name} ${t("账号导入", "account import")}`}>
    <label className="nt-credential-row"><span>{t("账号名称", "Account name")}</span>
      <input className="nt-input" autoComplete="off" maxLength={128} value={label} disabled={disabled}
        onChange={(event) => setLabel(event.currentTarget.value)} />
    </label>
    {onImportJson && family !== "gemini-canvas-chat" && <label className="nt-credential-row"><span>{t("录入方式", "Import method")}</span>
      <select className="nt-select" disabled={disabled} value={method} onChange={event => setMethod(event.currentTarget.value)}>
        <option value="browser">{t("网页登录", "Browser sign-in")}</option>
        <option value="json">{t("网页会话 JSON", "Web session JSON")}</option>
      </select></label>}
    {method === "json" && onImportJson ? hasSecretAccess
      ? <GoogleWebJsonField key={`${providerId}:${family}`} provider={family} disabled={disabled}
        onImport={source => onImportJson(family, providerId, label.trim() || undefined, source)} />
      : <button type="button" className="nt-btn nt-btn--primary" disabled={disabled} onClick={onRequestSecretAccess}>{t("确认敏感信息访问权限", "Confirm secret access")}</button>
      : <button type="button" className="nt-btn nt-btn--primary" disabled={disabled}
      onClick={() => onStart(family, providerId, label.trim() || undefined)}>
      {`${name} ${t("网页登录并导入", "sign in and import")}`}
    </button>}
  </section>;
}
