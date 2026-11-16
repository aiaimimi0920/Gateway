import { OperationsRequestTable, requestOrigin } from "./OperationsRequestTable";
import { useState, type ReactNode } from "react";
import { Button, Input, SegmentedControl } from "@neuro/ui";
import { OperationsTrend } from "./OperationsTrend";
import { OperationsStatusStrip } from "./OperationsStatusStrip";
import type { OperationsWorkspaceProps, OperationsRequestFilterDraft } from "./operations-contracts";
import { Panel } from "./OperationsPrimitives";
import { operationsPreviewData } from "./operationsPreviewData";

type Props = Pick<OperationsWorkspaceProps,
  "t" | "operatorSummary" | "editorLocked" | "requests" | "requestSummary" |
  "usageSummary" | "promptCache" | "requestFilters" | "onRequestFiltersChange" | "onApplyRequestFilters"
> & { patchFilters: (patch: Partial<OperationsRequestFilterDraft>) => void; summary?: ReactNode };

/** Show query-scoped outcomes, not internal routing, storage or cache diagnostics. */
export function OperationsRequestPanels(props: Props) {
  const { t } = props;
  const [samples] = useState(() => new URLSearchParams(window.location.search).get("operationsDemo") === "1" ? operationsPreviewData() : null);
  const requests = samples ? { ...props.requests, data: samples, loading: false, error: null } : props.requests;
  const [model, setModel] = useState("__all");
  const [provider, setProvider] = useState("__all");
  const [requestId, setRequestId] = useState("");
  const [period, setPeriod] = useState("all");
  const [status, setStatus] = useState("all");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const visible = (requests.data ?? []).filter(row => {
    const time = Date.parse(row.createdAt);
    const start = period === "today" ? new Date().setHours(0, 0, 0, 0) : period === "custom" ? (from ? new Date(from + "T00:00:00").getTime() : -Infinity) : period === "all" ? -Infinity : Date.now() - Number(period) * 86400000;
    const end = period === "custom" && to ? new Date(to + "T23:59:59.999").getTime() : Infinity;
    return (period === "all" || (time >= start && time <= end)) &&
      (status === "all" || (status === "failed" ? row.status === "failed" : row.status === "completed" && Number(row.routeTrace?.routeAttemptCount) > 1)) &&
      (model === "__all" || (row.resolvedModel ?? row.requestedModel) === model) && (provider === "__all" || requestOrigin(row).provider === provider) &&
      [row.id, row.requestedModel, row.resolvedModel, requestOrigin(row).provider, row.sessionId].some(value => value?.toLowerCase().includes(requestId.trim().toLowerCase()));
  });
  return <>
    {samples && <small className="n-muted">{t("演示数据 · 仅用于界面预览，不计入真实请求", "Demo data · visual preview only, excluded from real requests")}</small>}
    <div className="nt-operations-period-row">
      <SegmentedControl name="operations-period" hideLabel label={t("时间范围", "Time range")} value={period} onValueChange={setPeriod} options={[
        {value: "today", label: t("今天", "Today")}, {value: "7", label: t("7 天", "7 days")}, {value: "30", label: t("30 天", "30 days")}, {value: "all", label: t("全部", "All")}, {value: "custom", label: t("自定义", "Custom")},
      ]} />
      {period === "custom" && <div className="nt-operations-date-range"><Input compact type="date" aria-label={t("开始日期", "Start date")} value={from} onChange={event => setFrom(event.target.value)} /><span>—</span><Input compact type="date" aria-label={t("结束日期", "End date")} value={to} onChange={event => setTo(event.target.value)} /></div>}
    </div>
    <div className="nt-operations-request-filters">
      <SegmentedControl name="operations-status" hideLabel label={t("请求状态", "Request status")} value={status} onValueChange={setStatus} options={[{value: "all", label: t("全部", "All")}, {value: "failed", label: t("失败", "Failed")}, {value: "retry", label: t("重试后成功", "Succeeded after retry")}]} />
      <div className="nt-operations-filter-spacer" />
      <Input compact aria-label={t("请求 ID", "Request ID")} placeholder={t("搜索模型、服务商、请求 ID", "Search model, provider, request ID")} value={requestId} onChange={event => setRequestId(event.target.value)} />
      <Button compact onClick={() => {setModel("__all"); setProvider("__all"); setRequestId(""); setStatus("all"); setPeriod("all"); setFrom(""); setTo("");}}>{t("重置", "Reset")}</Button>
    </div>
    <OperationsStatusStrip rows={visible} t={t} />
    <OperationsTrend rows={visible} t={t} />
    <small className="n-muted">{t(`已加载记录：${visible.length} / ${requests.data?.length ?? 0}`, `Loaded records: ${visible.length} / ${requests.data?.length ?? 0}`)}</small>
    <Panel emptyLabel={t("没有匹配的请求", "No matching requests")} state={requests} t={t} title={t("请求列表", "Requests")}>
      {() => <OperationsRequestTable rows={visible} t={t} />}
    </Panel>
  </>;
}
