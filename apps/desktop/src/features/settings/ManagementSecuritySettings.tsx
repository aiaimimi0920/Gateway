import { Plus, ShieldCheck } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { SettingsSection } from "./SettingsSection";
import { ManagementKeyEditor } from "./ManagementKeyEditor";
import { ManagementKeyRow } from "./ManagementKeyRow";
import { ManagementKeyDeleteDialog } from "./ManagementKeyDeleteDialog";
import { managementKeysApi, type ManagementKey } from "./managementKeysApi";
import "./ManagementSecuritySettings.css";

export function ManagementSecuritySettings() {
  const { t } = useUiLocale();
  const host = useGatewayHost();
  const session = useManagementSession();
  const api = useMemo(() => managementKeysApi(host), [host]);
  const [keys, setKeys] = useState<ManagementKey[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  const inFlight = useRef(false);
  const addButton = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const current = session.managementToken;
  const available = session.phase === "authenticated" && !!current;
  const locked = !available || !loaded || pending || session.busy;

  useEffect(() => {
    if (editing === null && !locked && restoreFocus.current) {
      restoreFocus.current = false; addButton.current?.focus();
    }
  }, [editing, locked]);

  useEffect(() => {
    const version = ++generation.current;
    const controller = new AbortController();
    inFlight.current = false;
    setKeys([]); setLoaded(false); setEditing(null); setDeleting(null);
    setPending(false); setError("");
    if (available && current) void api.list(current, controller.signal).then(result => {
      if (generation.current === version) { setKeys(result.keys); setLoaded(true); }
    }).catch(() => {
      if (generation.current === version && !controller.signal.aborted) setError(t("无法加载管理密钥，请重新打开设置。", "Could not load management keys. Reopen settings."));
    });
    return () => { generation.current++; controller.abort(); };
  }, [api, current, available, t]);

  const mutate = async (action: "save" | "delete", key?: ManagementKey, name = "", token = "") => {
    if (!current || locked || inFlight.current) return;
    const version = generation.current;
    inFlight.current = true; setPending(true); setError("");
    try {
      if (action === "delete" && key) await api.revoke(current, key.id);
      else if (key) await api.edit(current, key.id, name, token || undefined);
      else await api.add(current, name, token);
      if (generation.current !== version) return;
      if (key?.current && (action === "delete" || (token && token !== current))) {
        await session.logout(); return;
      }
      const result = await api.list(current);
      if (generation.current !== version) return;
      setKeys(result.keys); setEditing(null); setDeleting(null);
      restoreFocus.current = true;
    } catch {
      if (generation.current === version) setError(t("未能确认操作结果。请保留密钥并重新打开设置后确认。", "Could not confirm the result. Keep the token and reopen settings to verify."));
    } finally {
      if (generation.current === version) { inFlight.current = false; setPending(false); }
    }
  };

  const cancelEdit = () => { restoreFocus.current = true; setEditing(null); };
  return <SettingsSection label={t("管理安全", "Management security")} icon={<ShieldCheck size={18} />}>
    <div className="nt-management-keys">
      <div className="nt-management-keys__toolbar">
        <button ref={addButton} type="button" className="nt-btn nt-btn--primary" disabled={locked || editing !== null || keys.length >= 16}
          onClick={() => { setEditing("new"); setDeleting(null); setError(""); }}><Plus size={16} />{t("添加", "Add")}</button>
      </div>
      {!loaded && !error && <p role="status">{t("加载中…", "Loading…")}</p>}
      <ul className="nt-management-keys__list" aria-label={t("管理密钥列表", "Management keys")}>
        {keys.map(key => <li key={`${generation.current}:${key.id}`} className="nt-management-keys__item">
          {editing === key.id ? <ManagementKeyEditor entry={key} locked={locked} onCancel={cancelEdit}
            onSave={(name, token) => void mutate("save", key, name, token)} /> :
            <ManagementKeyRow entry={key} api={api} managementToken={current ?? ""} locked={locked} last={keys.length <= 1}
              onEdit={() => { setEditing(key.id); setDeleting(null); setError(""); }}
              onDelete={() => { setDeleting(key.id); setEditing(null); setError(""); }} />}
        </li>)}
        {editing === "new" && <li className="nt-management-keys__item"><ManagementKeyEditor locked={locked}
          onCancel={cancelEdit} onSave={(name, token) => void mutate("save", undefined, name, token)} /></li>}
      </ul>
      <ManagementKeyDeleteDialog entry={keys.find(key => key.id === deleting) ?? null}
        pending={pending} locked={locked || keys.length <= 1} error={error}
        onCancel={() => { setDeleting(null); setError(""); }}
        onConfirm={key => void mutate("delete", key)} />
      {error && !deleting && <p className="nt-management-keys__message" role="alert">{error}</p>}
    </div>
  </SettingsSection>;
}
