import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Button, Input, Notice, SegmentedControl } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { gatewayProxyApi, type ProxyPolicy, type ProxySnapshot } from "./gatewayProxyApi";
import { SettingRow } from "./SettingsControls";

/** Controls the selected backend's egress, never the browser or desktop machine proxy. */
export function GatewayProxySettings({ onInteractionChange }: { onInteractionChange?: (locked: boolean) => void }) {
  const { t } = useUiLocale();
  const host = useGatewayHost();
  const session = useManagementSession();
  const api = useMemo(() => gatewayProxyApi(host), [host]);
  const token = session.phase === "authenticated" ? session.managementToken : null;
  const id = useId();
  const [saved, setSaved] = useState<ProxySnapshot | null>(null);
  const [draft, setDraft] = useState<ProxyPolicy>({ mode: "auto", url: "" });
  const [busy, setBusy] = useState("");
  const [error, setError] = useState(false);
  const [latency, setLatency] = useState<number | null>(null);
  const request = useRef<AbortController | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    const version = ++generation.current;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 15_000);
    request.current = controller; setSaved(null); setError(false); setLatency(null); setBusy(token ? "load" : "");
    if (token) void api.load(token, controller.signal).then(value => {
      if (version === generation.current) { setSaved(value); setDraft({ mode: value.mode, url: value.url }); }
    }).catch(() => { if (version === generation.current) setError(true); })
      .finally(() => { clearTimeout(timer); if (version === generation.current) { request.current = null; setBusy(""); } });
    else { clearTimeout(timer); request.current = null; }
    return () => { generation.current++; clearTimeout(timer); controller.abort(); request.current?.abort(); };
  }, [api, token]);
  useEffect(() => { onInteractionChange?.(busy === "save" || busy === "test"); }, [busy, onInteractionChange]);
  useEffect(() => () => onInteractionChange?.(false), [onInteractionChange]);
  const change = (next: ProxyPolicy) => { setDraft(next); setError(false); setLatency(null); };
  const policy = { mode: draft.mode, url: draft.mode === "custom" ? draft.url.trim() : "" };
  const changed = !!saved && (policy.mode !== saved.mode || policy.url !== saved.url);
  const run = async (operation: "test" | "save") => {
    if (!token || !saved || request.current) return;
    const version = generation.current;
    const controller = new AbortController(); request.current = controller;
    const timer = setTimeout(() => controller.abort(), 15_000);
    setBusy(operation); setError(false); setLatency(null);
    try {
      if (operation === "save") {
        const value = await api.save(token, policy, controller.signal);
        if (version === generation.current) { setSaved(value); setDraft({ mode: value.mode, url: value.url }); }
      } else {
        const value = await api.test(token, policy, controller.signal);
        if (version === generation.current) setLatency(value.latencyMs);
      }
    } catch { if (version === generation.current) setError(true); }
    finally { clearTimeout(timer); if (version === generation.current) { request.current = null; setBusy(""); } }
  };
  const disabled = !saved || !!busy;
  return <>
    <SettingRow id="proxy" label={t("代理", "Proxy")}>
      <SegmentedControl<ProxyPolicy["mode"] | ""> hideLabel name={id} label={t("代理", "Proxy")} value={saved ? draft.mode : ""} disabled={disabled}
        options={[{ value: "auto", label: t("自动", "Auto") }, { value: "off", label: t("关闭", "Off") }, { value: "custom", label: t("自定义", "Custom") }]}
        onValueChange={mode => { if (mode) change({ ...draft, mode }); }} />
      {draft.mode === "custom" && <Input compact aria-label={t("代理地址", "Proxy URL")} value={draft.url} maxLength={2048} disabled={disabled}
        placeholder="http://127.0.0.1:7890" autoComplete="off" spellCheck={false} onChange={event => change({ ...draft, url: event.target.value })} />}
      <Button compact disabled={disabled || (draft.mode === "custom" && !draft.url.trim())} loading={busy === "test"} onClick={() => void run("test")}>{t("测试", "Test")}</Button>
      <Button compact disabled={disabled || !changed || (draft.mode === "custom" && !draft.url.trim())} loading={busy === "save"} onClick={() => void run("save")}>{t("应用", "Apply")}</Button>
    </SettingRow>
    {saved?.restartRequired && <Notice>{t("代理设置已保存，重启当前网关后生效。", "Proxy settings saved. Restart this Gateway to apply.")}</Notice>}
    {latency !== null && <Notice>{t(`HTTPS 测试通过 · ${latency} ms`, `HTTPS test passed · ${latency} ms`)}</Notice>}
    {error && <Notice tone="danger">{t(saved ? "代理操作失败，请检查地址、代理服务和网络后重试。" : "无法读取代理设置，请确认后端已更新且管理权限有效。", saved ? "Proxy operation failed. Check the URL, proxy and network, then retry." : "Cannot load proxy settings. Check backend version and management access.")}</Notice>}
  </>;
}
