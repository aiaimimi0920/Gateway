import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Notice, SegmentedControl } from "@neuro/ui";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { SettingRow } from "./SettingsControls";
import { gatewayLanSharingApi } from "./gatewayLanSharingApi";

/** The backend owns this policy; a browser preference must never simulate access control. */
export function GatewayLanSharingSettings({ disabled = false, onInteractionChange }: {
  disabled?: boolean; onInteractionChange?: (locked: boolean) => void;
}) {
  const host = useGatewayHost();
  const session = useManagementSession();
  const { t } = useUiLocale();
  const id = useId();
  const api = useMemo(() => gatewayLanSharingApi(host), [host]);
  const token = session.phase === "authenticated" ? session.managementToken : null;
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const generation = useRef(0);
  const request = useRef<AbortController | null>(null);
  useEffect(() => {
    const version = ++generation.current;
    const controller = new AbortController(); request.current = controller;
    const timeout = setTimeout(() => controller.abort(), 15_000);
    setEnabled(null); setBusy(false); setFailed(false);
    if (token) void api.load(token, controller.signal).then(value => {
      if (version === generation.current) setEnabled(value.enabled);
    }).catch(() => { if (version === generation.current) setFailed(true); })
      .finally(() => { clearTimeout(timeout); if (version === generation.current) request.current = null; });
    else { clearTimeout(timeout); request.current = null; }
    return () => { generation.current++; clearTimeout(timeout); controller.abort(); request.current?.abort(); };
  }, [api, token]);
  useEffect(() => { onInteractionChange?.(busy); }, [busy, onInteractionChange]);
  useEffect(() => () => onInteractionChange?.(false), [onInteractionChange]);
  const save = async (next: boolean) => {
    if (!token || enabled === null || next === enabled || disabled || request.current) return;
    const version = generation.current;
    const controller = new AbortController(); request.current = controller;
    const timeout = setTimeout(() => controller.abort(), 15_000);
    setBusy(true); setFailed(false);
    try {
      const value = await api.save(token, next, controller.signal);
      if (version === generation.current) setEnabled(value.enabled);
    } catch { if (version === generation.current) setFailed(true); }
    finally { clearTimeout(timeout); if (version === generation.current) { request.current = null; setBusy(false); } }
  };
  return <>
    <SettingRow id="lan" label={t("局域网共享", "LAN sharing")}>
      <SegmentedControl<"" | "off" | "on"> hideLabel name={id} label={t("局域网共享", "LAN sharing")}
        value={enabled === null ? "" : enabled ? "on" : "off"} disabled={disabled || enabled === null || busy}
        options={[{ value: "off", label: t("关闭", "Off") }, { value: "on", label: t("开启", "On") }]}
        onValueChange={value => { if (value) void save(value === "on"); }} />
    </SettingRow>
    {failed && <Notice tone="danger">{t(enabled === null ? "无法读取局域网共享设置，请确认后端已更新且管理权限有效。" : "未能确认共享设置，请重新打开设置确认；关闭共享后，其他设备将无法访问。", enabled === null ? "Cannot load LAN sharing settings. Check backend version and management access." : "Could not confirm sharing settings. Reopen settings to verify; other devices cannot access the backend after sharing is disabled.")}</Notice>}
  </>;
}
