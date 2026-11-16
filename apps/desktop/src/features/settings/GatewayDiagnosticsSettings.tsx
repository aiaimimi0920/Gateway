import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Button, CopyButton, Input, Notice, Select } from "@neuro/ui";
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
  const generation = useRef(0);
  const request = useRef<AbortController | null>(null);
  const token = session.phase === "authenticated" ? session.managementToken : null;
  useEffect(() => {
    const version = ++generation.current;
    const controller = new AbortController(); request.current = controller;
    setValue(null); setFailed(false); setTruncated(false); setOperation(token ? "load" : "");
    if (token) void api.load(token, controller.signal).then(snapshot => {
      if (generation.current === version) setValue(snapshot);
    }).catch(() => { if (generation.current === version && !controller.signal.aborted) setFailed(true); })
      .finally(() => { if (generation.current === version) { request.current = null; setOperation(""); } });
    return () => { generation.current++; controller.abort(); request.current?.abort(); request.current = null; };
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
    } catch (error) { if (generation.current === version) setFailed(true); throw error; }
  };
  return <>
    <div className="nt-settings-fields">
      <div className="nt-setting-row" data-setting-id="log-directory" tabIndex={-1}><label htmlFor={`${id}-directory`}>{t("日志保存位置", "Log directory")}</label>
        <div className="nt-gateway-log-directory"><Input compact id={`${id}-directory`} readOnly value={value?.logDirectory ?? ""} />
          <CopyButton key={generation.current} compact label={t("复制", "Copy")} disabled={disabled} onCopy={copy}
            copiedLabel={t("已复制", "Copied")} errorLabel={t("复制失败", "Copy failed")} feedbackDuration={1500} /></div></div>
      <div className="nt-setting-row" data-setting-id="log-retention" tabIndex={-1}><label htmlFor={`${id}-retention`}>{t("日志保留天数", "Log retention")}</label>
        <Select compact id={`${id}-retention`} disabled={disabled} value={value?.retentionDays ?? 0}
          onChange={event => value && void run("save", { ...value, retentionDays: Number(event.target.value) as GatewayLogSettings["retentionDays"] })}>
          <option value="0">{t("不自动清理", "No automatic cleanup")}</option>
          {[3, 7, 30].map(days => <option key={days} value={days}>{t(`${days} 天`, `${days} days`)}</option>)}
        </Select></div>
      <div className="nt-setting-row" data-setting-id="log-limit" tabIndex={-1}><label htmlFor={`${id}-limit`}>{t("日志占用上限", "History limit")}</label>
        <Select compact id={`${id}-limit`} disabled={disabled} value={value?.logLimitMb ?? 0}
          onChange={event => value && void run("save", { ...value, logLimitMb: Number(event.target.value) as GatewayLogSettings["logLimitMb"] })}>
          <option value="0">{t("不限", "Unlimited")}</option>{[100, 200, 500].map(mb => <option key={mb} value={mb}>{mb} MB</option>)}
        </Select></div>
      <div className="nt-setting-row" data-setting-id="diagnostic-export" tabIndex={-1}><span className="nt-setting-label">{t("导出诊断信息", "Diagnostics")}</span>
        <Button compact disabled={disabled} loading={operation === "diagnostics"} onClick={() => void run("diagnostics")}>{t("导出", "Export")}</Button></div>
      <div className="nt-setting-row" data-setting-id="log-export" tabIndex={-1}><span className="nt-setting-label">{t("导出运行日志", "Runtime logs")}</span>
        <Button compact disabled={disabled} loading={operation === "logs"} onClick={() => void run("logs")}>{t("导出", "Export")}</Button></div>
    </div>
    <div className="nt-settings-feedback-slot">{(failed || !token || operation || truncated) && <Notice tone={failed ? "danger" : "info"}>{failed
      ? t("无法完成后端日志操作，请检查管理权限和后端状态后重试。", "Could not complete the backend log operation. Check access and backend status.")
      : !token ? t("登录后可管理此 Gateway 的诊断与日志。", "Sign in to manage this Gateway's diagnostics and logs.")
      : operation ? t("处理中…", "Working…") : truncated ? t("已导出最近日志，超出大小或文件数量上限的内容已截断。", "Recent logs exported; older or oversized content was truncated.") : ""}</Notice>}</div>
  </>;
}
