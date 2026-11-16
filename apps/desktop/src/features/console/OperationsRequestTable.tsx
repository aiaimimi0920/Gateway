import { Fragment, useState } from "react";
import { CopyButton } from "@neuro/ui";
import type { ConsoleRequestAudit } from "../../api/contracts";
import type { TranslateFn } from "./operations-contracts";
import { formatCount, formatMs, formatText, formatTimestamp } from "./OperationsPrimitives";

const object = (value: unknown): Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
const text = (value: unknown) => typeof value === "string" && value.trim() ? value : null;

/** Only use recorded identities, never infer a provider from a model name. */
export function requestOrigin(row: ConsoleRequestAudit) {
  const trace = object(row.routeTrace);
  const selected = object(trace.selectedCandidate);
  return {
    provider: text(selected.providerLabel) ?? text(selected.label) ?? row.providerAccountId,
    account: text(trace.realCredentialRef) ?? text(selected.realCredentialRef),
    code: text(trace.errorCode),
    attempts: Array.isArray(trace.attemptedProviderIds) ? trace.attemptedProviderIds.filter((id): id is string => typeof id === "string").slice(0, 100) : [],
  };
}

/** The row expands in place; stable request IDs preserve selection across refreshes. */
export function OperationsRequestTable({ rows, t }: { rows: ConsoleRequestAudit[]; t: TranslateFn }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const missing = t("未记录", "Not recorded");
  const status = (row: ConsoleRequestAudit) => ({ completed: t("成功", "Completed"), failed: t("失败", "Failed"), running: t("进行中", "Running"), cancelled: t("已取消", "Cancelled") } as Record<string, string>)[row.status] ?? row.status;
  const toggle = (id: string) => setSelectedId(current => current === id ? null : id);
  const headers = [t("时间", "Time"), t("请求模型", "Requested model"), t("供应商 · 账号", "Provider · account"), t("实际模型", "Resolved model"), t("输入", "Input"), t("输出", "Output"), t("费用", "Cost"), t("首字耗时", "First token"), t("耗时", "Duration"), t("速度", "Speed"), t("状态", "Status")];
  return <div className="nt-operations-table nt-operations-table--requests nt-request-ledger">
    <div className="n-table-scroll"><table className="n-table" aria-label={t("请求列表", "Requests")}>
      <thead><tr>{headers.map(label => <th key={label} scope="col">{label}</th>)}</tr></thead>
      <tbody>{rows.map(row => {
        const origin = requestOrigin(row);
        const open = selectedId === row.id;
        const detailId = `request-detail-${encodeURIComponent(row.id)}`;
        return <Fragment key={row.id}>
          <tr className="nt-request-ledger-row" data-open={open} onClick={() => { if (!window.getSelection()?.toString()) toggle(row.id); }}>
            <td><button type="button" className="nt-request-row-toggle" aria-label={`${t("请求详情", "Request details")}: ${row.id}`} aria-expanded={open} aria-controls={open ? detailId : undefined} onClick={event => { event.stopPropagation(); toggle(row.id); }}>{formatTimestamp(row.createdAt)}</button></td>
            <td>{formatText(row.requestedModel)}</td>
            <td><div className="nt-operations-request-origin"><span>{origin.provider ?? missing}</span><small className="nt-request-account">{t("账号：", "Account: ")}{origin.account ?? missing}</small></div></td>
            <td>{formatText(row.resolvedModel)}</td>
            <td>{formatCount(row.promptTokens)}</td><td>{formatCount(row.completionTokens)}</td>
            <td className="n-muted">—</td><td className="n-muted">—</td>
            <td>{formatMs(row.durationMs)}</td><td className="n-muted">—</td>
            <td><span className="nt-request-status" data-status={row.status}>{row.status === "failed" ? "× " : row.status === "completed" ? "✓ " : ""}{status(row)}</span></td>
          </tr>
          {open && <tr className="nt-request-ledger-detail"><td colSpan={headers.length}>
            <section id={detailId} aria-label={t("请求详情", "Request details")}>
              <dl className="nt-request-inline-fields">{[
                [t("状态", "Status"), status(row)],
                ...(row.errorSummary ? [[t("上游返回", "Upstream response"), row.errorSummary]] : []),
                ...(origin.code ? [[t("错误码", "Error code"), origin.code]] : []),
                [t("耗时", "Duration"), formatMs(row.durationMs)],
                [t("接口类型", "Endpoint type"), row.endpointKind],
                [t("请求模型", "Requested model"), formatText(row.requestedModel)],
                [t("实际模型", "Resolved model"), formatText(row.resolvedModel)],
                ...(row.upstreamStatus != null ? [[t("上游状态码", "Upstream status"), String(row.upstreamStatus)]] : []),
                ...(origin.attempts.length ? [[t("尝试过的供应商", "Attempted providers"), origin.attempts.join(" → ")]] : []),
                ...(row.sessionId ? [[t("会话 ID", "Session ID"), row.sessionId]] : []),
              ].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
              <div className="nt-request-inline-id"><span className="n-muted">{t("请求 ID", "Request ID")}</span><code>{row.id}</code><CopyButton compact label={t("复制请求 ID", "Copy request ID")} onCopy={() => navigator.clipboard.writeText(row.id)} /></div>
            </section>
          </td></tr>}
        </Fragment>;
      })}
      {!rows.length && <tr><td colSpan={headers.length} className="n-muted"><span role="status" aria-live="polite">{t("没有匹配的请求", "No matching requests")}</span></td></tr>}
      </tbody>
    </table></div>
  </div>;
}
