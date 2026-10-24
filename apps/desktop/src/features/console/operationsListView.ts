import type {
  ConsoleAnomalyIncident,
  ConsoleRequestAudit,
} from "../../api/contracts";
import type { OperationsPanelState, OperationsWorkspaceProps } from "./operations-contracts";

export type OperationsSort = "default" | "newest" | "oldest" | "name" | "name-desc";
export type OperationsListView = { query: string; sort: OperationsSort };
export const DEFAULT_OPERATIONS_LIST_VIEW: OperationsListView = {
  query: "", sort: "default",
};
type RowMetadata = {
  name: string;
  text: (string | number | null)[];
  time?: string | null;
};

/** Filter allowlisted public fields only. Never inspect arbitrary metadata, traces or payloads. */
export function selectOperationsRows<T>(
  rows: readonly T[],
  view: OperationsListView,
  describe: (row: T) => RowMetadata,
  defaultOrder?: (left: T, right: T) => number,
): T[] {
  const terms = view.query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const entries = rows.map((row) => ({ row, ...describe(row) })).filter((entry) => {
    const text = [entry.name, ...entry.text]
      .filter((value) => value !== null).join(" ").toLocaleLowerCase();
    return terms.every((term) => text.includes(term));
  });
  entries.sort((left, right) => {
    if (view.sort === "default") return defaultOrder?.(left.row, right.row) ?? 0;
    if (view.sort === "name" || view.sort === "name-desc") {
      const order = left.name.localeCompare(right.name, undefined, {
        numeric: true, sensitivity: "base",
      });
      return view.sort === "name-desc" ? -order : order;
    }
    const a = left.time ? Date.parse(left.time) : NaN;
    const b = right.time ? Date.parse(right.time) : NaN;
    // Missing timestamps stay last in either direction; untimed lists retain source order.
    if (!Number.isFinite(a) || !Number.isFinite(b)) {
      return Number.isFinite(a) ? -1 : Number.isFinite(b) ? 1 : 0;
    }
    return view.sort === "oldest" ? a - b : b - a;
  });
  return entries.map(({ row }) => row);
}

export const requestRowMetadata = (row: ConsoleRequestAudit): RowMetadata => ({
  name: row.resolvedModel ?? row.requestedModel ?? "",
  text: [row.requestedModel, row.status, row.errorSummary, row.upstreamStatus, row.createdAt],
  time: row.createdAt,
});

const incidentRowMetadata = (row: ConsoleAnomalyIncident): RowMetadata => ({
  name: row.code,
  text: [row.summary, row.status, row.severity, row.escalationStatus, row.lastSeenAt],
  time: row.lastSeenAt,
});
const incidentWeight = (status: string) =>
  status.toLowerCase() === "open" ? 0 : status.toLowerCase() === "acknowledged" ? 1 : 2;

/** Only replace detail arrays. Errors, loading flags, totals and server summary scope are unchanged. */
function mapPanel<T>(
  state: OperationsPanelState<T>, transform: (data: T) => T,
): OperationsPanelState<T> {
  return { ...state, data: state.data === null ? null : transform(state.data) };
}

type AnomalyLists = Pick<OperationsWorkspaceProps,
  "incidents" | "policies" | "alertQueue" | "remediationQueue" | "remediationRuns" | "exports">;

export function selectOperationsAnomalyLists(
  props: AnomalyLists, view: OperationsListView,
): AnomalyLists {
  return {
    incidents: mapPanel(props.incidents, (rows) => selectOperationsRows(
      rows, view, incidentRowMetadata,
      (left, right) => incidentWeight(left.status) - incidentWeight(right.status) ||
        Date.parse(right.lastSeenAt) - Date.parse(left.lastSeenAt),
    )),
    policies: mapPanel(props.policies, (rows) => selectOperationsRows(
      rows, view, (row) => ({
        name: row.name,
        text: [row.id, row.status, row.nextSyncDueAt],
        time: row.nextSyncDueAt,
      }),
    )),
    alertQueue: mapPanel(props.alertQueue, (data) => ({
      ...data,
      items: selectOperationsRows(data.items, view, (row) => ({
        name: row.incident.code,
        text: [
          row.incident.summary, row.webhookSeverity,
          ...row.remediationActionKeys, row.nextAlertDueAt,
        ],
        time: row.nextAlertDueAt,
      })),
    })),
    remediationQueue: mapPanel(props.remediationQueue, (data) => ({
      ...data,
      items: selectOperationsRows(data.items, view, (row) => ({
        name: row.action.title || row.action.actionKey,
        text: [
          row.action.actionKey, row.action.description, row.incident.id,
          row.incident.code, row.blockedReason, row.nextRunDueAt,
        ],
        time: row.nextRunDueAt,
      })),
    })),
    remediationRuns: mapPanel(props.remediationRuns, (rows) => selectOperationsRows(
      rows, view, (row) => ({
        name: row.title || row.actionKey,
        text: [
          row.actionKey, row.status, row.incidentId,
          row.errorSummary, row.completedAt ?? row.createdAt,
        ],
        time: row.completedAt ?? row.createdAt,
      }),
    )),
    exports: mapPanel(props.exports, (rows) => selectOperationsRows(
      rows, view, (row) => ({
        name: row.label ?? row.exportId,
        text: [row.exportId, row.status, row.createdAt],
        time: row.createdAt,
      }),
    )),
  };
}
