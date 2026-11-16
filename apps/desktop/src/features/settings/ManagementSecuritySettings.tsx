import { useEffect, useMemo, useRef, useState } from "react";
import { Input, Notice, Switch } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { managementKeysApi, type ManagementKey } from "./managementKeysApi";
import { SettingRow } from "./SettingsControls";
import "./ManagementSecuritySettings.css";

/** Disabled slots retain their stored key; only confirmed backend state drives switches. */
export function ManagementSecuritySettings({ onInteractionChange }: { onInteractionChange?: (locked: boolean) => void }) {
  const { t } = useUiLocale();
  const host = useGatewayHost();
  const session = useManagementSession();
  const api = useMemo(() => managementKeysApi(host), [host]);
  const [keys, setKeys] = useState<ManagementKey[]>([]);
  const [drafts, setDrafts] = useState(["", ""]);
  const [supported, setSupported] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<"" | "load" | "save">("");
  const generation = useRef(0);
  const inFlight = useRef(false);
  const current = session.managementToken;
  const available = session.phase === "authenticated" && !!current;
  const locked = !available || !loaded || !supported || pending || session.busy || keys.length > 2;
  const dirty = drafts.some(Boolean);
  useEffect(() => { onInteractionChange?.(dirty || pending); }, [dirty, pending, onInteractionChange]);
  useEffect(() => () => onInteractionChange?.(false), [onInteractionChange]);
  useEffect(() => {
    const version = ++generation.current;
    const controller = new AbortController();
    inFlight.current = false; setLoaded(false); setSupported(false); setKeys([]); setDrafts(["", ""]); setPending(false); setError("");
    if (available && current) void api.list(current, controller.signal).then(result => {
      if (version === generation.current) { setKeys(result.keys); setSupported(result.slotsSupported === true); setLoaded(true); }
    }).catch(() => { if (version === generation.current && !controller.signal.aborted) setError("load"); });
    return () => { generation.current++; controller.abort(); };
  }, [api, current, available]);

  const save = async (slot: number, enabled: boolean) => {
    if (!current || locked || inFlight.current) return;
    const replacement = drafts[slot].trim();
    if (!keys[slot] && !replacement) return;
    const version = generation.current;
    inFlight.current = true; setPending(true); setError("");
    try {
      await api.setSlot(current, slot, enabled, replacement || undefined);
      if (version !== generation.current) return;
      setDrafts(values => values.map((value, index) => index === slot ? "" : value));
      session.clearSecretGrant();
      if ((keys[slot]?.current && (!enabled || replacement)) || current.startsWith("passwordless:")) {
        if (enabled && replacement) await session.login(replacement);
        else await session.retryInitialization();
      } else {
        const result = await api.list(current);
        if (version === generation.current) setKeys(result.keys);
      }
    } catch { if (version === generation.current) setError("save"); }
    finally { if (version === generation.current) { inFlight.current = false; setPending(false); } }
  };

  return <div className="nt-management-key-slots">
    {[0, 1].map(slot => {
      const label = slot === 0 ? t("管理密钥一", "Management key 1") : t("管理密钥二", "Management key 2");
      const enabled = !!keys[slot] && keys[slot].enabled !== false;
      return <SettingRow key={slot} id={`management-key-${slot + 1}`} label={label}>
        <Input compact className="nt-management-key-slots__input" type="password" aria-label={label} value={drafts[slot]}
          placeholder={keys[slot] ? "••••••••" : t("未设置", "Not set")} autoComplete="new-password" maxLength={4096} disabled={locked}
          onChange={event => { const value = event.target.value; setDrafts(values => values.map((old, index) => index === slot ? value : old)); setError(""); }}
          onBlur={event => { if (drafts[slot].trim() && !(event.relatedTarget instanceof Element && event.relatedTarget.getAttribute("role") === "switch")) void save(slot, enabled); }}
          onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); void save(slot, enabled); } if (event.key === "Escape") setDrafts(values => values.map((value, index) => index === slot ? "" : value)); }} />
        <Switch label={enabled ? t("启用", "Enabled") : t("禁用", "Disabled")} aria-label={`${label} ${t("启用", "Enabled")}`}
          checked={enabled} disabled={locked || (!keys[slot] && !drafts[slot].trim())} onChange={() => void save(slot, !enabled)} />
      </SettingRow>;
    })}
    {error && <Notice tone="danger">{error === "load" ? t("无法加载管理密钥，请重新打开设置。", "Could not load management keys. Reopen settings.") : t("未能确认保存结果，请重新打开设置后确认。", "Could not confirm the save. Reopen settings to verify.")}</Notice>}
    {loaded && !supported && <Notice>{t("启用设置需要更新网关后端。", "Update the Gateway backend to change enabled states.")}</Notice>}
    {keys.length > 2 && <Notice tone="danger">{t("存在超过两把历史密钥，已保留；请先整理历史密钥。", "More than two existing keys were preserved. Resolve the legacy keys first.")}</Notice>}
  </div>;
}
