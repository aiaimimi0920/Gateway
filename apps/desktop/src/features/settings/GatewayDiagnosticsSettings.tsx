import { Check, Copy } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { gatewayDiagnosticsApi, type GatewayLogSettings } from "./gatewayDiagnosticsApi";
import { downloadGatewayExport } from "./diagnosticDownload";
import "./GatewayDiagnosticsSettings.css";

/** Both hosts operate on the selected Gateway. No native/client filesystem API is used here. */
export function GatewayDiagnosticsSettings({ onInteractionChange }: { onInteractionChange?: (locked: boolean) => void }) {
  const host = useGatewayHost();
  const session = useManagementSession();
  const { t } = useUiLocale();
  const id = useId();
  const api = useMemo(() => gatewayDiagnosticsApi(host), [host]);
  const [value, setValue] = useState<GatewayLogSettings | null>(null);
  const [operation, setOperation] = useState("");
  const [failed, setFailed] = useState(false);
  const [truncated, setTruncated] = useState(false);
  const [copied, setCopied] = useState(false);
  const generation = useRef(0);
  const request = useRef<AbortController | null>(null);
  const copyTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const token = session.phase === "authenticated" ? session.managementToken : null;
  useEffect(() => {
    const version = ++generation.current;
    const controller = new AbortController(); request.current = controller;
    setValue(null); setFailed(false); setTruncated(false); setCopied(false); setOperation(token ? "load" : "");
    if (token) void api.load(token, controller.signal).then(snapshot => {
      if (generation.current === version) setValue(snapshot);
    }).catch(() => { if (generation.current === version && !controller.signal.aborted) setFailed(true); })
      .finally(() => { if (generation.current === version) { request.current = null; setOperation(""); } });
    return () => { generation.current++; controller.abort(); request.current?.abort(); request.current = null; clearTimeout(copyTimer.current); };
  }, [api, token]);
  useEffect(() => { onInteractionChange?.(operation !== "" && operation !== "load"); }, [operation, onInteractionChange]);
  useEffect(() => () => onInteractionChange?.(false), [onInteractionChange]);

  const run = async (kind: "save" | "diagnostics" | "logs", policy?: GatewayLogSettings) => {
    if (!token || !value || request.current) return;
    const version = generation.current;
    const controller = new AbortController(); request.current = controller;
    setOperation(kind); setFailed(false); setTruncated(false);
    try {
      if (kind === "save") {
        const next = policy ?? value;
        const snapshot = await api.save(token, { retentionDays: next.retentionDays, logLimitMb: next.logLimitMb }, controller.signal);
        if (generation.current === version) setValue(snapshot);
      } else {
        const download = await api.export(token, kind, controller.signal);
        if (generation.current === version && !controller.signal.aborted) {
          downloadGatewayExport(download); setTruncated(download.truncated);
        }
      }
    } catch { if (generation.current === version && !controller.signal.aborted) setFailed(true); }
    finally { if (generation.current === version) { request.current = null; setOperation(""); } }
  };
  const disabled = !value || !!operation;
  const copy = async () => {
    if (!value) return;
    const version = generation.current;
    try {
      await navigator.clipboard.writeText(value.logDirectory);
      if (generation.current !== version) return;
      setCopied(true); clearTimeout(copyTimer.current); copyTimer.current = setTimeout(() => setCopied(false), 1500);
    } catch { if (generation.current === version) setFailed(true); }
  };
  return <>
    <div className="nt-settings-fields">
      <div className="nt-setting-row"><label htmlFor={`${id}-directory`}>{t("日志目录", "Log directory")}</label>
        <div className="nt-gateway-log-directory"><input id={`${id}-directory`} className="nt-input" readOnly value={value?.logDirectory ?? ""} />
          <button className="nt-btn nt-btn--outline" type="button" disabled={disabled} onClick={() => void copy()}>
            {copied ? <Check size={14} /> : <Copy size={14} />}{t("复制", "Copy")}</button></div></div>
      <div className="nt-setting-row"><label htmlFor={`${id}-retention`}>{t("日志保留时间", "Log retention")}</label>
        <select className="nt-input" id={`${id}-retention`} disabled={disabled} value={value?.retentionDays ?? 0}
          onChange={event => value && void run("save", { ...value, retentionDays: Number(event.target.value) as GatewayLogSettings["retentionDays"] })}>
          <option value="0">{t("不自动清理", "No automatic cleanup")}</option>
          {[3, 7, 30].map(days => <option key={days} value={days}>{t(`${days} 天`, `${days} days`)}</option>)}
        </select></div>
      <div className="nt-setting-row"><label htmlFor={`${id}-limit`}>{t("历史日志上限", "History limit")}</label>
        <select className="nt-input" id={`${id}-limit`} disabled={disabled} value={value?.logLimitMb ?? 0}
          onChange={event => value && void run("save", { ...value, logLimitMb: Number(event.target.value) as GatewayLogSettings["logLimitMb"] })}>
          <option value="0">{t("不限", "Unlimited")}</option>{[100, 200, 500].map(mb => <option key={mb} value={mb}>{mb} MB</option>)}
        </select></div>
      <div className="nt-setting-row"><span className="nt-setting-label">{t("诊断信息", "Diagnostics")}</span>
        <button className="nt-btn nt-btn--outline" type="button" disabled={disabled} onClick={() => void run("diagnostics")}>{t("导出", "Export")}</button></div>
      <div className="nt-setting-row"><span className="nt-setting-label">{t("运行日志", "Runtime logs")}</span>
        <button className="nt-btn nt-btn--outline" type="button" disabled={disabled} onClick={() => void run("logs")}>{t("导出", "Export")}</button></div>
    </div>
    <p className="nt-settings-feedback" role={failed ? "alert" : "status"}><span>{failed
      ? t("无法完成后端日志操作，请检查管理权限和后端状态后重试。", "Could not complete the backend log operation. Check access and backend status.")
      : !token ? t("登录后可管理此 Gateway 的诊断与日志。", "Sign in to manage this Gateway's diagnostics and logs.")
      : operation ? t("处理中…", "Working…") : truncated ? t("已导出最近日志，超出大小或文件数量上限的内容已截断。", "Recent logs exported; older or oversized content was truncated.") : ""}</span></p>
  </>;
}
