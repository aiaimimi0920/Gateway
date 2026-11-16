import { useMemo, useState } from "react";
import { SegmentedControl } from "@neuro/ui";
import { ChartNoAxesColumn, ChartLine } from "lucide-react";
import type { ConsoleRequestAudit } from "../../api/contracts";
import type { TranslateFn } from "./operations-contracts";
import { requestOrigin } from "./OperationsRequestTable";

const colors = ["var(--n-signal)", "var(--n-info)", "var(--n-success)", "var(--n-danger)", "#d9a441", "#9da9b8"];

/** Both chart styles project the same time buckets and category totals. */
export function OperationsTrend({ rows, t }: { rows: ConsoleRequestAudit[]; t: TranslateFn }) {
  const [metric, setMetric] = useState("tokens");
  const [group, setGroup] = useState("model");
  const [chart, setChart] = useState("bar");
  const series = useMemo(() => {
    if (metric === "cost" || metric === "speed") return { times: [], groups: [] };
    const points = new Map<number, Map<string, number>>();
    const totals = new Map<string, number>();
    for (const row of rows) {
      const time = Date.parse(row.createdAt);
      const value = metric === "tokens" ? row.totalTokens : 1;
      if (!Number.isFinite(time) || typeof value !== "number" || !Number.isFinite(value)) continue;
      const provider = requestOrigin(row).provider ?? t("未记录", "Unknown");
      const model = row.resolvedModel ?? row.requestedModel ?? t("未记录", "Unknown");
      const label = group === "provider" ? provider : group === "combined" ? `${model} · ${provider}` : model;
      const minute = Math.floor(time / 60000) * 60000;
      const bucket = points.get(minute) ?? new Map<string, number>();
      bucket.set(label, (bucket.get(label) ?? 0) + value);
      points.set(minute, bucket);
      totals.set(label, (totals.get(label) ?? 0) + value);
    }
    const times = [...points.keys()].sort((a, b) => a - b);
    // Bound SVG complexity without dropping the contribution of smaller categories.
    const ranked = [...totals].sort((a, b) => b[1] - a[1]);
    const labels = ranked.slice(0, 11).map(([label]) => [label]);
    if (ranked.length > 11) labels.push(ranked.slice(11).map(([label]) => label));
    const groups = labels.map((members, index) => ({
      label: members.length > 1 ? t("其他", "Other") : members[0],
      color: colors[index % colors.length],
      total: members.reduce((sum, label) => sum + totals.get(label)!, 0),
      values: times.map(time => members.reduce((sum, label) => sum + (points.get(time)!.get(label) ?? 0), 0)),
    }));
    return { times, groups };
  }, [rows, metric, group, t]);
  const { times, groups } = series;
  const totals = times.map((_, index) => groups.reduce((sum, item) => sum + item.values[index], 0));
  const peak = Math.max(1, ...totals);
  const x = (index: number) => 40 + (index + 0.5) * 920 / Math.max(1, times.length);
  const y = (value: number) => 265 - value / peak * 235;
  const below = (groupIndex: number, index: number) => groups.slice(0, groupIndex).reduce((sum, item) => sum + item.values[index], 0);
  return <section className="nt-operations-trend" aria-label={t("请求分布图", "Request distribution")}>
    <div className="nt-operations-trend-toolbar">
      <div className="nt-operations-chart-controls">
        <SegmentedControl name="operations-metric" hideLabel label={t("统计指标", "Metric")} value={metric} onValueChange={setMetric}
          options={[{ value: "tokens", label: "Token" }, { value: "cost", label: t("费用", "Cost") }, { value: "requests", label: t("请求", "Requests") }, { value: "speed", label: t("速度", "Speed") }]} />
        <div className="nt-operations-chart-types" role="group" aria-label={t("图表类型", "Chart type")}>
          <button type="button" aria-label={t("柱状图", "Bar chart")} title={t("柱状图", "Bar chart")} aria-pressed={chart === "bar"} onClick={() => setChart("bar")}><ChartNoAxesColumn size={17} /></button>
          <button type="button" aria-label={t("折线图", "Line chart")} title={t("折线图", "Line chart")} aria-pressed={chart === "line"} onClick={() => setChart("line")}><ChartLine size={17} /></button>
        </div>
      </div>
      <div className="nt-operations-chart-breakdown">
        <SegmentedControl name="operations-group" hideLabel label={t("拆分维度", "Breakdown")} value={group} onValueChange={setGroup}
          options={[{ value: "model", label: t("模型", "Model") }, { value: "combined", label: t("模型 · 供应商", "Model · provider") }, { value: "provider", label: t("供应商", "Provider") }]} />
      </div>
    </div>
    <div className="nt-operations-chart-viewport">
      {times.length ? <div className="nt-operations-series-chart">
        <svg viewBox="0 0 1000 300" role="img" aria-label={chart === "line" ? t("每分钟请求趋势", "Per-minute request trend") : t("每分钟分类柱状图", "Per-minute grouped bars")}>
          <path d="M40 20V265H960" fill="none" stroke="var(--n-line)" />
          {groups.map((item, groupIndex) => {
            const tops = item.values.map((value, index) => [x(index), y(below(groupIndex, index) + value)]);
            const bottoms = item.values.map((_, index) => [x(index), y(below(groupIndex, index))]).reverse();
            return <g key={item.label} data-series={item.label}>
              {chart === "line" ? <>
                <polygon points={[...tops, ...bottoms].map(point => point.join(",")).join(" ")} fill={item.color} opacity="0.12" />
                <polyline points={tops.map(point => point.join(",")).join(" ")} fill="none" stroke={item.color} strokeWidth="2" />
                {tops.map(([cx, cy], index) => <circle key={times[index]} cx={cx} cy={cy} r="3" fill={item.color}><title>{`${item.label} · ${new Date(times[index]).toLocaleString()}: ${item.values[index]}`}</title></circle>)}
              </> : item.values.map((value, index) => {
                const width = Math.max(1, Math.min(32, 920 / times.length * 0.7));
                return <rect key={times[index]} x={x(index) - width / 2} y={y(below(groupIndex, index) + value)} width={width} height={value / peak * 235} fill={item.color}><title>{`${item.label} · ${new Date(times[index]).toLocaleString()}: ${value}`}</title></rect>;
              })}
            </g>;
          })}
          <text x="40" y="290">{new Date(times[0]).toLocaleTimeString()}</text><text x="960" y="290" textAnchor="end">{new Date(times.at(-1)!).toLocaleTimeString()}</text>
          <text x="32" y="25" textAnchor="end">{peak}</text>
        </svg>
        <div className="nt-operations-chart-legend">{groups.map(item => <span key={item.label} title={item.label}><i style={{ background: item.color }} />{item.label}<strong>{item.total.toLocaleString()}</strong></span>)}</div>
      </div> : <div className="nt-operations-chart-empty">{rows.length ? t("这些请求没有可用的用量数据", "No usage data for these requests") : t("这段时间没有已加载的请求", "No loaded requests in this period")}</div>}
    </div>
    <small className="n-muted">{t("按当前已加载记录统计", "Based on currently loaded records")}</small>
  </section>;
}