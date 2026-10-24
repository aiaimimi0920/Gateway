import { Check, Copy } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import "./BackendConnectionForm.css";
import "./AppearanceSettings.css";

export type BackendConnectionFormProps = {
  current: string; localAddress: string; mode: "local" | "existing"; remote: string;
  busy: "" | "test" | "apply"; message: string; error: boolean; native?: boolean;
  onMode(mode: "local" | "existing"): void; onRemote(value: string): void;
  onTest(): void; onApply(): void;
};
export function BackendConnectionForm(props: BackendConnectionFormProps) {
  const { t } = useUiLocale();
  const id = useId();
  const [copy, setCopy] = useState<"" | "ok" | "error">("");
  const copyEpoch = useRef(0);
  useEffect(() => { if (!copy) return; const timer = window.setTimeout(() => setCopy(""), 1800); return () => window.clearTimeout(timer); }, [copy]);
  useEffect(() => { copyEpoch.current++; setCopy(""); return () => { copyEpoch.current++; }; }, [props.current]);
  return <>
    <div className="nt-backend-summary">
      <div className="nt-setting-row"><span className="nt-setting-label">{t("当前后端", "Current")}</span>
        <div className="nt-backend-current"><code title={props.current}>{props.current || "—"}</code>
          <button type="button" className="nt-btn nt-btn--outline nt-backend-copy" disabled={!props.current}
            title={copy === "error" ? t("复制失败，请手动选择地址", "Copy failed. Select the address manually") : undefined}
            onClick={async () => { const epoch = copyEpoch.current; try { await navigator.clipboard.writeText(props.current); if (copyEpoch.current === epoch) setCopy("ok"); } catch { if (copyEpoch.current === epoch) setCopy("error"); } }}>
            {copy === "ok" ? <Check size={15} aria-hidden="true" /> : <Copy size={15} aria-hidden="true" />}{t("复制", "Copy")}
          </button>
          <span className="nt-visually-hidden" role="status">{copy === "ok" ? t("已复制", "Copied") : copy === "error" ? t("复制失败", "Copy failed") : ""}</span>
        </div>
      </div>
      <div className="nt-setting-row"><span className="nt-setting-label">{t("连接状态", "Status")}</span>
        <span className="nt-backend-status">{props.current ? t("已连接", "Connected") : t("尚未连接", "Not connected")}</span></div>
    </div>
    <div className="nt-settings-fields">
      <div className="nt-setting-row"><label htmlFor={`${id}-mode`}>{t("连接方式", "Mode")}</label>
        <select className="nt-input" id={`${id}-mode`} disabled={!!props.busy} value={props.mode}
          onChange={event => props.onMode(event.target.value === "existing" ? "existing" : "local")}>
          <option value="local">{t("本机Gateway", "Local Gateway")}</option>
          <option value="existing">{t("远程Gateway", "Remote Gateway")}</option>
        </select>
      </div>
      <div className="nt-setting-row nt-backend-address-row"><label htmlFor={`${id}-address`}>{t("后端地址", "Address")}</label>
        <div className="nt-backend-editor">
          <input className="nt-input" id={`${id}-address`} type="url" value={props.mode === "local" ? props.localAddress : props.remote}
            readOnly={props.mode === "local"} disabled={!!props.busy} maxLength={2048} autoComplete="off" spellCheck={false}
            placeholder="https://gateway.example.com" aria-describedby={`${id}-feedback`} aria-invalid={props.error || undefined}
            onChange={event => props.onRemote(event.target.value)} onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); props.onApply(); } }} />
          <div className="nt-backend-actions">
            <button type="button" className="nt-btn nt-btn--outline" disabled={!!props.busy} onClick={props.onTest}>{props.busy === "test" ? t("测试中…", "Testing…") : t("测试连接", "Test")}</button>
            <button type="button" className="nt-btn nt-btn--primary" disabled={!!props.busy} onClick={props.onApply}>{props.busy === "apply" ? t("连接中…", "Connecting…") : t("应用并连接", "Apply & connect")}</button>
          </div>
        </div>
      </div>
    </div>
    <p className="nt-settings-feedback nt-backend-feedback" id={`${id}-feedback`} role={props.error ? "alert" : "status"} title={props.message}><span>{props.message}</span></p>
  </>;
}
