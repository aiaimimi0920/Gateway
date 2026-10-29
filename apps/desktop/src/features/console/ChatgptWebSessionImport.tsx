import { useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ChatgptWebBrowserImport, type WebBrowserCapture } from "./ChatgptWebBrowserImport";
import { ChatgptWebMaterialField } from "./ChatgptWebMaterialField";
import type { ChatgptWebMaterial } from "./chatgptWebMaterial";

/** Switching method unmounts its owner, fencing late file reads and browser results. */
export function ChatgptWebSessionImport({ capture, providerId, disabled, loaded, onMaterial, onClear }: {
  capture?: WebBrowserCapture;
  providerId: string;
  disabled: boolean;
  loaded: boolean;
  onMaterial(material: ChatgptWebMaterial, accountName?: string): void;
  onClear(): void;
}) {
  const { t } = useUiLocale();
  const [method, setMethod] = useState("browser");
  const browser = Boolean(capture && method === "browser");
  return <section className="nt-stack">
    {capture && <label className="nt-credential-row">
      <span>{t("录入方式", "Import method")}</span>
      <select className="nt-select" value={method} disabled={disabled} onChange={(event) => {
        onClear(); setMethod(event.currentTarget.value);
      }}>
        <option value="browser">{t("网页登录", "Browser sign-in")}</option>
        <option value="json">{t("网页会话 JSON", "Web session JSON")}</option>
      </select>
    </label>}
    {browser && capture
      ? <ChatgptWebBrowserImport capture={capture} providerId={providerId} disabled={disabled}
        onMaterial={onMaterial} onClear={onClear} />
      : <ChatgptWebMaterialField disabled={disabled} loaded={loaded} onMaterial={onMaterial} onClear={onClear} />}
  </section>;
}
