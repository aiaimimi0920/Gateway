import { useEffect, useMemo, useRef, useState } from "react";
import { z } from "zod";
import type { GatewayApiClient } from "../../api/client";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { siteHealthConfigSchema, siteHealthResponseSchema, siteSnapshotSchema,
  type SiteHealthConfig, type SiteHealthResponse } from "./siteHealth";

type Props = { document: ConsoleRouteDocument | null; client: GatewayApiClient; managementToken: string | null;
  locked: boolean; draftDirty: boolean; onSave(id: string, config: SiteHealthConfig | null): void;
  t(zh: string, en: string): string };

// Site telemetry is fetched only while expanded; cancellation and host/token changes fence stale UI.
export function SiteHealthWorkspace({ document, client, managementToken, locked, draftDirty, onSave, t }: Props) {
  const [open, setOpen] = useState(false);
  const [id, setId] = useState("");
  const [enabled, setEnabled] = useState(false);
  const [config, setConfig] = useState<SiteHealthConfig>(() => siteHealthConfigSchema.parse({}));
  const [groups, setGroups] = useState("");
  const [mapping, setMapping] = useState("");
  const [reports, setReports] = useState<SiteHealthResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const manual = useRef<AbortController | null>(null);
  const providers = useMemo(() => (document?.providers ?? []).filter(isRecord), [document]);
  const provider = providers.find((p) => p.id === id) ?? providers[0];
  const providerId = String(provider?.id ?? "");
  const saved = siteHealthConfigSchema.safeParse(provider?.site_health);
  const policyEdited = enabled !== saved.success || (enabled && saved.success && (
    JSON.stringify(config) !== JSON.stringify(saved.data)
    || groups !== saved.data.monitor_groups.join(", ")
    || mapping !== Object.entries(saved.data.model_map).map(([a,b]) => `${a}=${b}`).join("\n")));
  useEffect(() => {
    setBusy(false);
    return () => { manual.current?.abort(); manual.current = null; };
  }, [client, managementToken, providerId, open]);
  useEffect(() => {
    const parsed = siteHealthConfigSchema.safeParse(provider?.site_health);
    setEnabled(parsed.success); setConfig(parsed.success ? parsed.data : siteHealthConfigSchema.parse({}));
    setGroups(parsed.success ? parsed.data.monitor_groups.join(", ") : "");
    setMapping(parsed.success ? Object.entries(parsed.data.model_map).map(([a,b]) => `${a}=${b}`).join("\n") : "");
  }, [provider]);
  useEffect(() => {
    if (!open || !managementToken) return;
    const controller = new AbortController(); let timer: ReturnType<typeof setTimeout>;
    const load = async () => {
      try {
        const result = await client.request("/v1/internal/gateway/site-health", siteHealthResponseSchema,
          { managementToken, signal: controller.signal });
        if (!controller.signal.aborted) { setReports(result); setError(null); }
      } catch (error) { if (!controller.signal.aborted) setError(String(error)); }
      if (!controller.signal.aborted) timer = setTimeout(() => void load(), 15000);
    };
    setReports(null); void load();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [open, managementToken, client, refresh]);
  const report = reports?.items.find((r) => r.provider_id === providerId)?.snapshot;
  const run = () => { setRefresh((value) => value + 1); };
  const save = () => {
    try {
      const model_map = Object.fromEntries(mapping.split("\n").filter((v) => v.trim()).map((line) => {
        const at = line.indexOf("="); if (at < 1 || at === line.length - 1) throw new Error(t("模型映射格式：本地模型=站点模型", "Model mapping: local=site"));
        return [line.slice(0,at).trim(), line.slice(at+1).trim()];
      }));
      onSave(providerId, enabled ? siteHealthConfigSchema.parse({ ...config, model_map,
        monitor_groups: groups.split(",").map((s) => s.trim()).filter(Boolean) }) : null);
      setError(null);
    } catch (error) { setError(String(error)); }
  };
  return <details className="nt-panel" onToggle={(event) => setOpen(event.currentTarget.open)}>
    <summary>{t("站点健康", "Site health")}</summary>
    {open && <div className="nt-stack">
      <label className="nt-field"><span>{t("站点", "Site")}</span><select className="nt-select" value={providerId} onChange={(e) => setId(e.target.value)}>
        {providers.map((p) => <option key={String(p.id)} value={String(p.id)}>{String(p.label ?? p.id)}</option>)}
      </select></label>
      <fieldset disabled={locked || busy || !providerId} className="nt-grid nt-grid--2">
        <label><input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />{t("启用自动接口识别与站点监控", "Enable automatic site health discovery")}</label>
        <label className="nt-field"><span>{t("控制策略", "Policy")}</span><select className="nt-select" value={config.mode} onChange={(e) => setConfig({ ...config, mode: e.target.value as SiteHealthConfig["mode"] })}>
          <option value="advisory">{t("仅提示", "Advisory")}</option><option value="restrict">{t("限制异常模型路由", "Restrict unhealthy models")}</option></select></label>
        <label className="nt-field"><span>{t("监控站地址（留空使用本站）", "Monitor URL (empty: this site)")}</span><input className="nt-input" type="url" value={config.site_url ?? ""} onChange={(e) => setConfig({ ...config, site_url: e.target.value || null })} /></label>
        <label className="nt-field"><span>{t("最低成功率 %", "Minimum success rate %")}</span><input className="nt-input" type="number" min={0} max={100} value={config.minimum_success_rate} onChange={(e) => setConfig({ ...config, minimum_success_rate: e.target.valueAsNumber })} /></label>
        <label className="nt-field"><span>{t("刷新间隔（秒）", "Refresh interval (seconds)")}</span><input className="nt-input" type="number" min={30} value={config.refresh_interval_secs} onChange={(e) => setConfig({ ...config, refresh_interval_secs: e.target.valueAsNumber })} /></label>
        <label className="nt-field"><span>{t("信息有效期（秒）", "Maximum age (seconds)")}</span><input className="nt-input" type="number" min={30} value={config.max_age_secs} onChange={(e) => setConfig({ ...config, max_age_secs: e.target.valueAsNumber })} /></label>
        <label className="nt-field"><span>{t("未知或过期信息", "Unknown or stale information")}</span><select className="nt-select" value={config.unknown_policy} onChange={(e) => setConfig({ ...config, unknown_policy: e.target.value as SiteHealthConfig["unknown_policy"] })}>
          <option value="allow">{t("放行", "Allow")}</option><option value="block">{t("阻止", "Block")}</option></select></label>
        <label className="nt-field"><span>{t("监控分组（逗号分隔）", "Monitor groups (comma separated)")}</span><input className="nt-input" value={groups} onChange={(e) => setGroups(e.target.value)} /></label>
        <label className="nt-field"><span>{t("模型映射", "Model mappings")}</span><textarea className="nt-textarea" value={mapping} onChange={(e) => setMapping(e.target.value)} placeholder="upstream-model=monitor-model" /></label>
        <label><input type="checkbox" checked={config.allow_private_network} onChange={(e) => setConfig({ ...config, allow_private_network: e.target.checked })} />{t("允许访问所选站点的内网地址", "Allow private network addresses for this site")}</label>
      </fieldset>
      <div className="dialog-actions"><button className="nt-btn nt-btn--primary" disabled={locked || busy || !providerId} onClick={save}>{t("保存站点策略", "Save site policy")}</button>
        <button className="nt-btn nt-btn--outline" disabled={locked || busy || draftDirty || policyEdited || !enabled || !managementToken} onClick={() => {
          const controller = new AbortController(); manual.current?.abort(); manual.current = controller;
          setBusy(true); void client.request(`/v1/internal/gateway/site-health/${encodeURIComponent(providerId)}/refresh`, z.object({snapshot:siteSnapshotSchema}), {method:"POST",managementToken,signal:controller.signal})
            .then(() => { if (!controller.signal.aborted) run(); })
            .catch((error) => { if (!controller.signal.aborted) setError(String(error)); })
            .finally(() => { if (!controller.signal.aborted) setBusy(false); });
        }}>{busy ? t("识别中", "Detecting") : t("立即识别接口", "Detect interfaces now")}</button></div>
      {error && <div role="alert">{error}</div>}
      {report ? <><div>{t("检测时间", "Checked")}: {new Date(report.checked_at * 1000).toLocaleString()} · {report.selected_template ?? t("未确认接口", "No confirmed interface")}
        {Date.now()/1000-report.checked_at >= config.max_age_secs ? ` · ${t("已过期", "Stale")}` : ""}</div>
        <table className="nt-table"><thead><tr><th>{t("自动模板", "Detected template")}</th><th>{t("状态", "Status")}</th><th>HTTP</th><th>{t("模型数", "Models")}</th></tr></thead>
          <tbody>{report.probes.map((p) => <tr key={p.template}><td title={p.endpoint}>{p.template}</td><td>{p.status}</td><td>{p.http_status ?? "—"}</td><td>{p.model_count}</td></tr>)}</tbody></table>
        <div>{Object.entries(report.models).map(([name,rate]) => <span className="nt-badge" key={name}>{name}: {rate === null ? t("无有效样本", "No valid samples") : `${rate.toFixed(1)}%`} </span>)}</div>
      </> : <div role="status">{t("尚无站点健康结果", "No site health results yet")}</div>}
    </div>}
  </details>;
}
