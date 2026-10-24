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
      <div role="status">
        {t("尚无站点健康结果", "No site health results yet")}
      </div>
    );
  return (
    <div className="nt-stack">
      {edited && (
        <div role="status">
          {t(
            "有未保存修改，以下结果属于已保存的检测策略。",
            "Unsaved edits: results below use the saved policy.",
          )}
        </div>
      )}
      <div>
        {t("检测时间", "Checked")}:{" "}
        {new Date(report.checked_at * 1000).toLocaleString()} ·{" "}
        {report.selected_template ? t("已识别健康来源", "Health source identified") : t("未确认接口", "No confirmed interface")}
        {Date.now() / 1000 - report.checked_at >= maxAge
          ? ` · ${t("已过期", "Stale")}`
          : ""}
      </div>
      <div className="nt-site-health__models">
        {report.probes.map(
          (probe) =>
            officialHealthScopeNote(probe.template, t) && (
              <p key={probe.template} role="note">
                {officialHealthScopeNote(probe.template, t)}
              </p>
            ),
        )}
        <table className="nt-table nt-health-model-table">
          <thead>
            <tr>
              <th>{t("模型 / 官方组件", "Model / official component")}</th>
              <th>{t("可用性", "Availability")}</th>
              <th>{t("状态 / 范围", "Status / scope")}</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(report.models).map(([name, rate]) => (
              <tr key={name}>
                <td>{name}</td>
                <td>{rate === null ? "—" : `${rate.toFixed(1)}%`}</td>
                <td>
                  {rate === null
                    ? t("无有效样本", "No valid samples")
                    : t("监控成功率", "Observed success rate")}
                </td>
              </tr>
            ))}
            {report.probes.flatMap((probe) =>
              (probe.observations ?? []).map((item, index) => (
                <tr key={`${probe.template}:${index}`}>
                  <td>{item.model ?? item.name}</td>
                  <td>{officialHealthStatus(item.status, t)}</td>
                  <td>
                    {probe.template === "openai-status"
                      ? t(
                          "本方案全部模型统一继承",
                          "Inherited by all models in this plan",
                        )
                      : probe.template === "qwen-status"
                        ? t(
                            "公告来源，非模型状态",
                            "Public notices, not model status",
                          )
                        : item.model
                          ? item.name
                          : t("官方组件状态", "Official component status")}
                  </td>
                </tr>
              )),
            )}
            {!Object.keys(report.models).length &&
              !report.probes.some((p) => p.observations?.length) && (
                <tr>
                  <td colSpan={3}>
                    {t("暂无模型或组件数据", "No model or component data")}
                  </td>
                </tr>
              )}
          </tbody>
        </table>
      </div>
      <details className="nt-site-health__results">
        <summary>{t("接口诊断详情", "Probe diagnostics")}</summary>
        <table className="nt-table">
          <thead>
            <tr>
              <th>{t("内部读取器 / 官方来源", "Internal reader / official source")}</th>
              <th>{t("状态", "Status")}</th>
              <th>HTTP</th>
              <th>{t("模型数", "Models")}</th>
              <th>{t("覆盖信息", "Coverage")}</th>
            </tr>
          </thead>
          <tbody>
            {report.probes.map((p) => (
              <tr key={p.template}>
                <td title={p.endpoint}>{p.template}{p.template === report.selected_template ? ` · ${t("已采用", "Selected")}` : ""}</td>
                <td>{p.status === "authentication_target_unconfirmed"
                  ? t("认证目标未确认，未发送密钥", "Auth target unconfirmed; no credential sent")
                  : p.status === "capability_detected"
                    ? t("仅公开能力识别，未作为本方案健康结果", "Public capability only; not this plan's health evidence")
                    : p.status}</td>
                <td>{p.http_status ?? "—"}</td>
                <td>{p.model_count}</td>
                <td>
                  {p.coverage?.complete === false
                    ? t("部分覆盖", "Partial")
                    : p.coverage?.complete === true
                      ? t("完整覆盖", "Complete")
                      : "—"}
                  {p.coverage?.aggregation_lag_secs != null
                    ? ` · ${p.coverage.aggregation_lag_secs}s lag`
                    : ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}
