import { DataTable, Disclosure, EmptyState, Notice } from "@neuro/ui";
import type { z } from "zod";
import type { siteSnapshotSchema } from "./siteHealth";
import {
  officialHealthStatus,
  officialHealthScopeNote,
} from "./officialHealthAdapters";

type Props = {
  report: z.infer<typeof siteSnapshotSchema> | null | undefined;
  maxAge: number;
  edited: boolean;
  t(zh: string, en: string): string;
};
export function SiteHealthReport({ report, maxAge, edited, t }: Props) {
  if (!report)
    return (
      <EmptyState title={t("尚无站点健康结果", "No site health results yet")} />
    );
  const rows = [
    ...Object.entries(report.models).map(([name, rate]) => ({
      id: `model:${name}`, name,
      availability: rate === null ? "—" : `${rate.toFixed(1)}%`,
      scope: rate === null ? t("无有效样本", "No valid samples") : t("监控成功率", "Observed success rate"),
    })),
    ...report.probes.flatMap((probe) => (probe.observations ?? []).map((item, index) => ({
      id: `probe:${probe.template}:${index}`, name: item.model ?? item.name,
      availability: officialHealthStatus(item.status, t),
      scope: probe.template === "openai-status" ? t("本方案全部模型统一继承", "Inherited by all models in this plan")
        : probe.template === "qwen-status" ? t("公告来源，非模型状态", "Public notices, not model status")
          : item.model ? item.name : t("官方组件状态", "Official component status"),
    }))),
  ];
  return (
    <div className="n-stack nt-health-report-content">
      {edited && (
        <Notice tone="warning">
          {t(
            "有未保存修改，以下结果属于已保存的检测策略。",
            "Unsaved edits: results below use the saved policy.",
          )}
        </Notice>
      )}
      <div>
        {t("检测时间", "Checked")}:{" "}
        {new Date(report.checked_at * 1000).toLocaleString()} ·{" "}
        {report.selected_template ? t("已识别健康来源", "Health source identified") : t("未确认接口", "No confirmed interface")}
        {Date.now() / 1000 - report.checked_at >= maxAge
          ? ` · ${t("已过期", "Stale")}`
          : ""}
      </div>
      <div className="n-stack">
        {report.probes.map(
          (probe) =>
            officialHealthScopeNote(probe.template, t) && (
              <p key={probe.template} role="note">
                {officialHealthScopeNote(probe.template, t)}
              </p>
            ),
        )}
        <DataTable label={t("模型 / 官方组件", "Model / official component")} rows={rows} getRowKey={(row) => row.id}
          empty={<EmptyState title={t("暂无模型或组件数据", "No model or component data")} />}
          columns={[
            { key: "name", header: t("模型 / 官方组件", "Model / official component"), width: "43%", rowHeader: true, cell: (row) => row.name },
            { key: "availability", header: t("可用性", "Availability"), width: "25%", cell: (row) => row.availability },
            { key: "scope", header: t("状态 / 范围", "Status / scope"), cell: (row) => row.scope },
          ]} />
      </div>
      <Disclosure title={t("接口诊断详情", "Probe diagnostics")}>
        <DataTable label={t("接口诊断详情", "Probe diagnostics")} rows={report.probes} getRowKey={(p) => p.template}
          columns={[
            {
              key: "reader", header: t("内部读取器 / 官方来源", "Internal reader / official source"), rowHeader: true,
              cell: (p) => <span title={p.endpoint}>{p.template}{p.template === report.selected_template ? ` · ${t("已采用", "Selected")}` : ""}</span>
            },
            {
              key: "status", header: t("状态", "Status"), cell: (p) => p.status === "authentication_target_unconfirmed"
                ? t("认证目标未确认，未发送密钥", "Auth target unconfirmed; no credential sent")
                : p.status === "capability_detected"
                  ? t("仅公开能力识别，未作为本方案健康结果", "Public capability only; not this plan's health evidence")
                  : p.status
            },
            { key: "http", header: "HTTP", width: "12%", cell: (p) => p.http_status ?? "—" },
            { key: "count", header: t("模型数", "Models"), width: "12%", cell: (p) => p.model_count },
            {
              key: "coverage", header: t("覆盖信息", "Coverage"), cell: (p) => <>
                {p.coverage?.complete === false
                  ? t("部分覆盖", "Partial")
                  : p.coverage?.complete === true
                    ? t("完整覆盖", "Complete")
                    : "—"}
                {p.coverage?.aggregation_lag_secs != null
                  ? ` · ${p.coverage.aggregation_lag_secs}s lag`
                  : ""}
              </>
            },
          ]} />
      </Disclosure>
    </div>
  );
}
