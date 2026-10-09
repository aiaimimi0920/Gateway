import { ChevronDown } from "lucide-react";
import type { ReactNode } from "react";
import type {
  ConsoleDependencyReadiness,
  ConsoleSummaryBucket,
} from "../../api/contracts";
import type {
  OperationsPanelState,
  OperationsSectionId,
  TranslateFn,
} from "./operations-contracts";

export const PLACEHOLDER = "—";

export function formatCount(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return PLACEHOLDER;
  }
  return new Intl.NumberFormat("en-US").format(value);
}

export function formatUsd(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return PLACEHOLDER;
  }
  return `$${value.toFixed(4)}`;
}

/** Backend rates are already fractions of one, not percentages. */
export function formatRate(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return PLACEHOLDER;
  }
  return `${(value * 100).toFixed(1)}%`;
}

export function formatMs(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return PLACEHOLDER;
  }
  return `${Math.round(value)} ms`;
}

export function formatTimestamp(value: string | null | undefined): string {
  if (!value) {
    return PLACEHOLDER;
  }
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
}

export function formatText(value: string | null | undefined): string {
  return value && value.trim().length > 0 ? value : PLACEHOLDER;
}

/** Trims long ids so a table row keeps its column widths. */
export function shortId(value: string | null | undefined): string {
  if (!value) {
    return PLACEHOLDER;
  }
  return value.length > 14 ? `${value.slice(0, 12)}…` : value;
}

/** `requestDurationMsSum / requestDurationMsCount`, guarded against a zero count. */
export function averageDurationMs(sum: number, count: number): number | null {
  return count > 0 ? sum / count : null;
}

export function severityBadgeClass(
  severity: string | null | undefined,
): string {
  const normalized = (severity ?? "").toLowerCase();
  if (
    normalized === "critical" ||
    normalized === "fatal" ||
    normalized === "high"
  ) {
    return "nt-badge nt-badge--danger";
  }
  if (
    normalized === "warning" ||
    normalized === "medium" ||
    normalized === "warn"
  ) {
    return "nt-badge nt-badge--warning";
  }
  return "nt-badge";
}

export function statusBadgeClass(status: string | null | undefined): string {
  const normalized = (status ?? "").toLowerCase();
  if (
    normalized === "resolved" ||
    normalized === "succeeded" ||
    normalized === "success" ||
    normalized === "completed" ||
    normalized === "active"
  ) {
    return "nt-badge nt-badge--success";
  }
  if (
    normalized === "failed" ||
    normalized === "error" ||
    normalized === "open"
  ) {
    return "nt-badge nt-badge--danger";
  }
  if (
    normalized === "acknowledged" ||
    normalized === "pending" ||
    normalized === "queued" ||
    normalized === "running"
  ) {
    return "nt-badge nt-badge--warning";
  }
  return "nt-badge";
}

type StatTone = "default" | "emerald" | "danger" | "blue";

export function StatCard({
  label,
  value,
  hint,
  tone = "default",
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: StatTone;
}) {
  const toneClass = tone === "default" ? "" : ` nt-pilot-stat-card--${tone}`;
  return (
    <article className={`nt-pilot-stat-card${toneClass}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      {hint ? <small>{hint}</small> : null}
    </article>
  );
}

/**
 * Shared frame for every panel: heading, loading/error state, then the body. A
 * failed panel still shows its heading so the operator can tell a missing
 * dependency from a missing feature.
 */
export function Panel<T>({
  t,
  title,
  state,
  emptyLabel,
  collapsed = false,
  children,
}: {
  t: TranslateFn;
  title: string;
  state: OperationsPanelState<T>;
  emptyLabel?: string;
  collapsed?: boolean;
  children: (data: T) => ReactNode;
}) {
  const body = (
    <>
      {state.error ? (
        <div className="nt-alert nt-alert--warning" role="status">
          {state.error}
        </div>
      ) : null}
      {state.loading && state.data === null ? (
        <div className="nt-pilot-stats-state" role="status">
          {t("正在读取…", "Loading…")}
        </div>
      ) : state.data === null && !state.error ? (
        <div className="nt-pilot-stats-state" role="status">
          {emptyLabel ?? t("暂无数据", "No data yet")}
        </div>
      ) : state.data !== null ? (
        children(state.data)
      ) : null}
    </>
  );
  return collapsed ? (
    <details className="nt-pilot-metric-card nt-operations-detail">
      <summary>
        <span>{title}</span>
        {state.error ? (
          <span className="nt-badge nt-badge--danger">
            {t("读取失败", "Read failed")}
          </span>
        ) : null}
        {state.loading ? (
          <span className="nt-copy">{t("读取中…", "Loading…")}</span>
        ) : null}
      </summary>
      <div className="nt-operations-detail__body">{body}</div>
    </details>
  ) : (
    <article className="nt-pilot-metric-card">
      <h3>{title}</h3>
      {body}
    </article>
  );
}

/** `{key, count}` buckets are how the gateway reports every breakdown. */
export function BucketList({
  t,
  title,
  buckets,
}: {
  t: TranslateFn;
  title: string;
  buckets: ConsoleSummaryBucket[];
}) {
  if (buckets.length === 0) {
    return null;
  }
  return (
    <div className="nt-console-bucket-group">
      <h4>{title}</h4>
      <dl className="nt-pilot-metric-list">
        {buckets.slice(0, 8).map((bucket) => (
          <div key={bucket.key}>
            <dt title={bucket.key}>{formatText(bucket.key)}</dt>
            <dd>{formatCount(bucket.count)}</dd>
          </div>
        ))}
      </dl>
      {buckets.length > 8 ? (
        <p className="nt-pilot-stats-note">
          {t(
            `另有 ${buckets.length - 8} 项未显示`,
            `${buckets.length - 8} more not shown`,
          )}
        </p>
      ) : null}
    </div>
  );
}

export function DependencyRow({
  t,
  name,
  dependency,
}: {
  t: TranslateFn;
  name: string;
  dependency: ConsoleDependencyReadiness;
}) {
  const stateClass = dependency.ready
    ? "nt-badge nt-badge--success"
    : dependency.configured
      ? "nt-badge nt-badge--danger"
      : "nt-chip nt-chip--muted";
  const stateLabel = dependency.ready
    ? t("就绪", "Ready")
    : dependency.configured
      ? t("异常", "Failing")
      : t("未配置", "Unconfigured");
  const notes: string[] = [];
  if (dependency.required) {
    notes.push(t("必需", "Required"));
  }
  if (dependency.timedOut) {
    notes.push(t("检查超时", "Check timed out"));
  }
  if (dependency.driver) {
    notes.push(dependency.driver);
  }
  return (
    <div className="nt-table__row">
      <span>{name}</span>
      <span>
        <span className={stateClass}>{stateLabel}</span>
      </span>
      <span className="nt-copy">
        {notes.length > 0 ? notes.join(" · ") : PLACEHOLDER}
      </span>
    </div>
  );
}

export function Section({
  id,
  title,
  icon,
  status,
  open,
  onToggle,
  children,
}: {
  id: OperationsSectionId;
  title: string;
  icon: ReactNode;
  status?: ReactNode;
  open: boolean;
  onToggle: (id: OperationsSectionId) => void;
  children: ReactNode;
}) {
  const bodyId = `nt-operations-section-${id}`;
  return (
    <section
      className={`nt-settings-section${open ? " nt-settings-section--open" : ""}`}
      data-section={id}
    >
      <h2 className="nt-settings-section__heading">
        <button
          aria-controls={bodyId}
          aria-expanded={open}
          className="nt-settings-section__trigger"
          onClick={() => onToggle(id)}
          type="button"
        >
          <span className="nt-settings-section__icon" aria-hidden="true">
            {icon}
          </span>
          <span>{title}</span>
          {status}
          <span className="nt-settings-section__chevron" aria-hidden="true">
            <ChevronDown size={18} />
          </span>
        </button>
      </h2>
      {open ? (
        <div className="nt-settings-section__body" id={bodyId}>
          {children}
        </div>
      ) : null}
    </section>
  );
}
