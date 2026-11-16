import { Badge, Disclosure, EmptyState, Notice, Panel as Surface, type Tone } from "@neuro/ui";
import { useEffect, useState, type ReactNode } from "react";
import type { ConsoleSummaryBucket } from "../../api/contracts";
import type {
  OperationsPanelState,
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

export function severityTone(
  severity: string | null | undefined,
): Tone {
  const normalized = (severity ?? "").toLowerCase();
  if (
    normalized === "critical" ||
    normalized === "fatal" ||
    normalized === "high"
  ) {
    return "danger";
  }
  if (
    normalized === "warning" ||
    normalized === "medium" ||
    normalized === "warn"
  ) {
    return "warning";
  }
  return "neutral";
}

export function statusTone(status: string | null | undefined): Tone {
  const normalized = (status ?? "").toLowerCase();
  if (
    normalized === "resolved" ||
    normalized === "succeeded" ||
    normalized === "success" ||
    normalized === "completed" ||
    normalized === "active"
  ) {
    return "success";
  }
  if (
    normalized === "failed" ||
    normalized === "error" ||
    normalized === "open"
  ) {
    return "danger";
  }
  if (
    normalized === "acknowledged" ||
    normalized === "pending" ||
    normalized === "queued" ||
    normalized === "running"
  ) {
    return "warning";
  }
  return "neutral";
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
  const valueTone = { default: "neutral", emerald: "success", danger: "danger", blue: "info" } as const;
  return (
    <article className="nt-operations-stat">
      <span className="n-muted">{label}</span>
      <strong data-tone={valueTone[tone]}>{value}</strong>
      {hint ? <small className="n-muted">{hint}</small> : null}
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
        <Notice tone="warning">
          {state.error}
        </Notice>
      ) : null}
      {state.loading && state.data === null ? (
        <Notice>
          {t("正在读取…", "Loading…")}
        </Notice>
      ) : state.data === null && !state.error ? (
        <div role="status"><EmptyState compact title={emptyLabel ?? t("暂无数据", "No data yet")} /></div>
      ) : state.data !== null ? (
        children(state.data)
      ) : null}
    </>
  );
  return collapsed ? (
    <Disclosure className="nt-operations-detail" title={<span className="n-row">
      <span>{title}</span>
      {state.error ? (
        <Badge tone="danger">
          {t("读取失败", "Read failed")}
        </Badge>
      ) : null}
      {state.loading ? (
        <span className="n-muted">{t("读取中…", "Loading…")}</span>
      ) : null}
    </span>}>
      <div className="nt-operations-detail__body">{body}</div>
    </Disclosure>
  ) : (
    <Surface className="nt-operations-card">
      <h3>{title}</h3>
      {body}
    </Surface>
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
    <div className="nt-operations-bucket">
      <h4>{title}</h4>
      <dl className="nt-operations-metrics">
        {buckets.slice(0, 8).map((bucket) => (
          <div key={bucket.key}>
            <dt title={bucket.key}>{formatText(bucket.key)}</dt>
            <dd>{formatCount(bucket.count)}</dd>
          </div>
        ))}
      </dl>
      {buckets.length > 8 ? (
        <p className="n-muted">
          {t(
            `另有 ${buckets.length - 8} 项未显示`,
            `${buckets.length - 8} more not shown`,
          )}
        </p>
      ) : null}
    </div>
  );
}

/** Telemetry transitions reopen diagnostics; an unchanged warning still allows manual collapse. */
export function DiagnosticDisclosure({
  active,
  title,
  children,
}: {
  active: boolean;
  title: string;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(active);
  useEffect(() => setOpen(active), [active]);
  return (
    <Disclosure className="nt-operations-detail" title={title} open={open} onOpenChange={setOpen}>
      {children}
    </Disclosure>
  );
}
