import { Eye, EyeOff } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export function CredentialSecretField(props: {
  value: string; existing: boolean; disabled: boolean; hasSecretAccess: boolean;
  onRequestSecretAccess(): void;
  onReveal?: (signal: AbortSignal) => Promise<string>;
  onLoaded(value: string): void; onChange(value: string): void;
}) {
  const { t } = useUiLocale();
  const [visible, setVisible] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const active = useRef<AbortController | null>(null);
  useEffect(() => {
    setVisible(false); setBusy(false); setError(null);
    return () => { active.current?.abort(); };
  }, [props.onReveal]);
  useEffect(() => {
    if (!props.hasSecretAccess) { active.current?.abort(); setVisible(false); setBusy(false); }
  }, [props.hasSecretAccess]);
  const reveal = async () => {
    if (visible) { setVisible(false); return; }
    if (!props.hasSecretAccess) { props.onRequestSecretAccess(); return; }
    if (props.value || !props.existing) { setVisible(true); return; }
    if (!props.onReveal || busy) return;
    const request = new AbortController(); active.current = request;
    setBusy(true); setError(null);
    try {
      const key = await props.onReveal(request.signal);
      if (!request.signal.aborted) { props.onLoaded(key); setVisible(true); }
    } catch (cause) {
      if (!request.signal.aborted) setError(cause instanceof Error ? cause.message : t("读取失败", "Unable to read key"));
    } finally { if (!request.signal.aborted) setBusy(false); }
  };
  return <div className="nt-credential-row">
    <label htmlFor="credential-api-key">API Key</label>
    <div>
      <div className="nt-credential-secret">
        <input id="credential-api-key" className="nt-input" type={visible && props.hasSecretAccess ? "text" : "password"}
          autoComplete="new-password" spellCheck={false} value={props.value}
          placeholder={props.existing ? "••••••••" : undefined}
          disabled={props.disabled || busy || !props.hasSecretAccess}
          onChange={(event) => { active.current?.abort(); setBusy(false); setError(null); props.onChange(event.currentTarget.value); }} />
        <button type="button" className="nt-icon-action" disabled={props.disabled || busy}
          aria-label={visible ? t("隐藏 API Key", "Hide API key") : t("显示 API Key", "Show API key")}
          aria-pressed={visible} onClick={() => void reveal()}>
          {visible ? <EyeOff size={16} /> : <Eye size={16} />}
        </button>
      </div>
      {busy && <span role="status">{t("读取中…", "Loading…")}</span>}
      {error && <span role="alert">{error}</span>}
    </div>
  </div>;
}
