import type { ConsoleRequestAudit } from "../../api/contracts";
import type { TranslateFn } from "./operations-contracts";

/** Summary, chart and table use the same loaded audit scope. */
export function OperationsStatusStrip({ rows, t }: { rows: ConsoleRequestAudit[]; t: TranslateFn }) {
  const sum = (field: "totalTokens" | "promptTokens" | "completionTokens") => {
    const values = rows.map(row => row[field]);
    return values.every(value => typeof value === "number") ? values.reduce<number>((total, value) => total + (value ?? 0), 0).toLocaleString() : "—";
  };
  const failed = rows.filter(row => row.status === "failed").length;
  const metrics = [
    ["Token", sum("totalTokens"), t(`输入 ${sum("promptTokens")} · 输出 ${sum("completionTokens")}`, `Input ${sum("promptTokens")} · Output ${sum("completionTokens")}`)],
    [t("请求", "Requests"), String(rows.length), failed ? t(`${failed} 个失败`, `${failed} failed`) : t("没有失败", "No failures")],
    [t("费用", "Cost"), "—", t("暂无计费数据", "Billing unavailable")],
    [t("缓存命中率", "Cache hit rate"), "—", t("暂无缓存用量", "Cache usage unavailable")],
    [t("输出速度", "Output speed"), "—", t("暂无流式计时", "Stream timing unavailable")],
  ];
  return <div className="nt-operations-overview">
    <small className="n-muted">{t("统计当前已加载的请求", "Currently loaded requests")}</small>
    <div className="nt-operations-dashboard">{metrics.map(([label, value, detail]) => <dl className="nt-operations-dashboard-group" key={label}>
      <dt>{label}</dt><dd>{value}</dd><small>{detail}</small>
    </dl>)}</div>
  </div>;
}