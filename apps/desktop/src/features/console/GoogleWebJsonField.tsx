import { useLayoutEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export const GOOGLE_WEB_JSON_MAX_BYTES = 64 * 1024;

/** Sensitive input stays in this transient owner; replacing it fences pending file reads. */
export function GoogleWebJsonField({ provider, disabled, onImport }: {
  provider: string;
  disabled: boolean;
  onImport(source: string): void;
}) {
  const { t } = useUiLocale();
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  useLayoutEffect(() => {
    generation.current += 1;
    setSource(""); setError(null); setBusy(false);
    return () => { generation.current += 1; };
  }, [provider, disabled]);

  function validate(input: string) {
    if (new TextEncoder().encode(input).length > GOOGLE_WEB_JSON_MAX_BYTES) throw new Error(t("网页会话 JSON 不能超过 64 KiB。", "Web session JSON must not exceed 64 KiB."));
    let value;
    try { value = JSON.parse(input); } catch { throw new Error(t("网页会话 JSON 格式无效。", "Invalid web session JSON.")); }
    if (!value || value.schemaVersion !== 1 || value.provider !== provider) throw new Error(t("网页会话 JSON 的版本或服务商类型不匹配。", "Web session JSON version or provider does not match."));
  }
  return <section className="nt-stack">
    <label className="nt-credential-row"><span>{t("网页会话 JSON", "Web session JSON")}</span>
      <textarea className="nt-input" rows={5} autoComplete="off" spellCheck={false} value={source} disabled={disabled || busy}
        onChange={event => { generation.current += 1; setSource(event.currentTarget.value); setError(null); }} />
    </label>
    <label className="nt-credential-row"><span>{t("选择 JSON 文件", "Choose JSON file")}</span>
      <input type="file" accept=".json,application/json" disabled={disabled || busy} onChange={async event => {
        const file = event.currentTarget.files?.[0]; event.currentTarget.value = "";
        if (!file || disabled) return;
        const current = ++generation.current;
        setSource(""); setError(null);
        if (file.size > GOOGLE_WEB_JSON_MAX_BYTES) { setError(t("网页会话 JSON 不能超过 64 KiB。", "Web session JSON must not exceed 64 KiB.")); return; }
        setBusy(true);
        try {
          const value = await file.text();
          if (current !== generation.current) return;
          validate(value); setSource(value);
        } catch { if (current === generation.current) setError(t("无法读取该服务商的网页会话 JSON，请检查格式和大小。", "Cannot read web session JSON for this provider; check its format and size.")); }
        finally { if (current === generation.current) setBusy(false); }
      }} />
    </label>
    {error && <span role="alert">{error}</span>}
    <button type="button" className="nt-btn nt-btn--primary" disabled={disabled || busy || !source.trim()} onClick={() => {
      if (disabled || busy) return;
      try { validate(source); }
      catch (cause) { setError(cause instanceof Error ? cause.message : t("网页会话 JSON 无效。", "Invalid web session JSON.")); return; }
      const value = source; setSource(""); setError(null);
      try { onImport(value); } catch { setError(t("网页会话 JSON 导入失败，请重试。", "Web session JSON import failed; try again.")); }
    }}>{t("导入 JSON", "Import JSON")}</button>
  </section>;
}
