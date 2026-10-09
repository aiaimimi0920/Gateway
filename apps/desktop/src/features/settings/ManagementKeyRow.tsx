import { Eye, EyeOff } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { managementKeysApi, ManagementKey } from "./managementKeysApi";

type Props = {
  entry: ManagementKey;
  api: ReturnType<typeof managementKeysApi>;
  managementToken: string;
  locked: boolean;
  last: boolean;
  onEdit: () => void;
  onDelete: () => void;
};

export function ManagementKeyRow({ entry, api, managementToken, locked, last, onEdit, onDelete }: Props) {
  const { t } = useUiLocale();
  const [revealed, setRevealed] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const request = useRef<AbortController | null>(null);
  useEffect(() => () => { request.current?.abort(); }, []);
  useEffect(() => {
    const hide = () => { request.current?.abort(); setRevealed(null); setLoading(false); };
    const onVisibility = () => { if (document.hidden) hide(); };
    window.addEventListener("blur", hide);
    document.addEventListener("visibilitychange", onVisibility);
    return () => { window.removeEventListener("blur", hide); document.removeEventListener("visibilitychange", onVisibility); };
  }, []);
  useEffect(() => {
    setRevealed(null); setError(""); setLoading(false); request.current?.abort();
  }, [api, managementToken, entry]);
  useEffect(() => {
    if (revealed === null) return;
    const hide = () => setRevealed(null);
    const timer = window.setTimeout(hide, 60_000);
    return () => { window.clearTimeout(timer); };
  }, [revealed]);
  const toggle = async () => {
    if (revealed !== null) { setRevealed(null); return; }
    if (loading || locked) return;
    request.current?.abort();
    const controller = new AbortController(); request.current = controller;
    setLoading(true); setError("");
    try {
      const result = await api.reveal(managementToken, entry.id, controller.signal);
      if (!controller.signal.aborted) {
        if (result.token === null) setError(t("旧密钥未保存可恢复内容，请通过编辑重新设置。", "This legacy key cannot be recovered. Set a new value using Edit."));
        else setRevealed(result.token);
      }
    } catch {
      if (!controller.signal.aborted) setError(t("无法显示密钥，请重试。", "Could not show the key. Try again."));
    } finally { if (!controller.signal.aborted) setLoading(false); }
  };
  const label = revealed === null ? t(`显示 ${entry.name} 的密钥`, `Show key for ${entry.name}`) : t(`隐藏 ${entry.name} 的密钥`, `Hide key for ${entry.name}`);
  return <>
    <div className="nt-management-keys__row">
      <div className="nt-management-keys__identity"><strong title={entry.name}>{entry.name}</strong>
        {entry.current && <span className="nt-management-keys__current">{t("当前登录", "Current session")}</span>}</div>
      <input className="nt-input nt-management-keys__secret" aria-label={t(`${entry.name} 的管理密钥`, `Management key for ${entry.name}`)}
        readOnly type={revealed === null ? "password" : "text"} value={revealed ?? "••••••••••••"} autoComplete="off" spellCheck={false} />
      <div className="nt-management-keys__actions">
        <button type="button" className="nt-btn nt-btn--secondary nt-management-keys__eye" aria-label={label} title={label}
          aria-pressed={revealed !== null} disabled={locked || loading} onClick={() => void toggle()}>{revealed === null ? <Eye size={16} /> : <EyeOff size={16} />}</button>
        <button type="button" className="nt-btn nt-btn--danger" aria-label={t(`删除 ${entry.name}`, `Delete ${entry.name}`)}
          title={last ? t("至少保留一个有效密钥", "Keep at least one valid key") : undefined} disabled={locked || last} onClick={onDelete}>{t("删除", "Delete")}</button>
        <button type="button" className="nt-btn nt-btn--secondary" aria-label={t(`编辑 ${entry.name}`, `Edit ${entry.name}`)} disabled={locked} onClick={onEdit}>{t("编辑", "Edit")}</button>
      </div>
    </div>
    {error && <p className="nt-management-keys__message" role="alert">{error}</p>}
  </>;
}
