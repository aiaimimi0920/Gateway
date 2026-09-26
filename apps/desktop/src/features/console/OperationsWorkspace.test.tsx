import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleAnomalyIncident } from "../../api/contracts";

import {
  OPERATIONS_DEFAULT_REQUEST_FILTERS,
  OperationsWorkspace,
  type OperationsWorkspaceProps,
} from "./OperationsWorkspace";

function fixture(overrides: Partial<OperationsWorkspaceProps> = {}): OperationsWorkspaceProps {
  const empty = { data: null, error: null, loading: false };
  return {
    t: (_zh, en) => en,
    editorLocked: false, refreshing: false, onRefresh: vi.fn(),
    pressure: empty, readiness: empty, operatorSummary: empty,
    requests: empty, requestSummary: empty, usageSummary: empty, promptCache: empty,
    incidents: empty, incidentSummary: empty, alertQueue: empty, policies: empty,
    remediationQueue: empty, remediationRuns: empty, remediationEffectiveness: empty,
    hotspots: empty, exports: empty, exportInventory: empty,
    requestFilters: { ...OPERATIONS_DEFAULT_REQUEST_FILTERS },
    onRequestFiltersChange: vi.fn(), onApplyRequestFilters: vi.fn(),
    incidentBusyId: null, onAcknowledgeIncident: vi.fn(), onResolveIncident: vi.fn(),
    ...overrides,
  };
}

function incident(id: string, status: string, lastSeenAt: string): ConsoleAnomalyIncident {
  return {
    id, status, lastSeenAt, code: id, fingerprint: id, summary: id,
    policyId: null, projectId: null, routePolicyId: null, tag: null, textMode: null,
    severity: "high", ownerUserId: null, followUpStatus: "pending", syncHitCount: 1,
    escalationStatus: "none", escalatedAt: null, escalationReason: null,
    latestNote: null, resolutionNote: null, lastActionAt: null, lastAlertAttemptAt: null,
    lastAlertedAt: null, lastAlertSeverity: null, alertDeliveryCount: 0,
    latestExportId: null, previousExportId: null, latestValue: null, previousValue: null,
    deltaValue: null, deltaRatio: null, thresholdValue: null,
    firstSeenAt: lastSeenAt, acknowledgedAt: null, resolvedAt: null,
    createdAt: lastSeenAt, updatedAt: lastSeenAt,
  };
}

describe("operations workspace boundaries", () => {
  it("orders actionable incidents without mutating input and enforces per-row action locks", () => {
    const rows = [
      incident("resolved-case", "resolved", "2026-09-09T12:00:00Z"),
      incident("open-old", "open", "2026-09-09T09:00:00Z"),
      incident("ack-case", "acknowledged", "2026-09-09T11:00:00Z"),
      incident("open-new", "OPEN", "2026-09-09T10:00:00Z"),
    ];
    Object.freeze(rows);
    const props = fixture({ incidents: { data: rows, error: "stale incident data", loading: true } });
    const { container, rerender } = render(<OperationsWorkspace {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Anomalies & remediation" }));
    expect(screen.getByText("stale incident data")).toBeVisible();
    expect(Array.from(container.querySelectorAll(".nt-table--incidents .nt-table__row"))
      .map((row) => row.firstElementChild?.textContent)).toEqual([
      "open-new", "open-old", "ack-case", "resolved-case",
    ]);
    const rowFor = (id: string) => {
      const row = screen.getByText(id).closest(".nt-table__row");
      if (!(row instanceof HTMLElement)) throw new Error("Missing incident row");
      return within(row);
    };
    fireEvent.click(rowFor("open-new").getByRole("button", { name: "Acknowledge" }));
    expect(props.onAcknowledgeIncident).toHaveBeenCalledWith("open-new");
    expect(rowFor("ack-case").getByRole("button", { name: "Acknowledge" })).toBeDisabled();
    fireEvent.click(rowFor("ack-case").getByRole("button", { name: "Resolve" }));
    expect(props.onResolveIncident).toHaveBeenCalledWith("ack-case");
    expect(rowFor("resolved-case").getByRole("button", { name: "Resolve" })).toBeDisabled();
    rerender(<OperationsWorkspace {...props} incidentBusyId="open-new" />);
    expect(rowFor("open-new").getByRole("button", { name: "Resolve" })).toBeDisabled();
    expect(rowFor("open-old").getByRole("button", { name: "Resolve" })).toBeEnabled();
    rerender(<OperationsWorkspace {...props} editorLocked />);
    expect(rowFor("open-old").getByRole("button", { name: "Acknowledge" })).toBeDisabled();
    expect(rows[0].id).toBe("resolved-case");
  });

  it("distinguishes unavailable queue depth from zero and formats fractional cache metrics", () => {
    render(<OperationsWorkspace {...fixture({
      usageSummary: { loading: false, error: null, data: {
        queueDepth: -1, recentRequestCount: 2, recentFailureCount: 1,
        recentTotalTokens: 1200, archiveFailureCount: 0, alerts: [],
      } },
      promptCache: { loading: false, error: null, data: {
        totalRequests: 4, cacheHitRequests: 2, cacheCreationRequests: 1,
        clientMarkedRequests: 1, autoAppliedRequests: 1, cacheControlCoverageRequests: 2,
        totalTokensSaved: 100, totalCacheCreationInputTokens: 300,
        estimatedCostSavedUsd: 0.01, cacheHitRate: 0.5, cacheControlCoverageRate: 0.5,
        inputPricePerMillion: 1, cachedInputPricePerMillion: 0.1,
      } },
    })} />);
    fireEvent.click(screen.getByRole("button", { name: "Request log" }));
    const queueCard = screen.getByText("Queue depth").closest("article");
    expect(queueCard).toHaveTextContent("\u2014");
    expect(queueCard).toHaveTextContent("Redis unavailable");
    expect(screen.getAllByText("50.0%")).toHaveLength(2);
    expect(screen.getByText("$0.0100")).toBeVisible();
  });

  it("opens live only and preserves independent panel errors and loading state", () => {
    render(<OperationsWorkspace {...fixture({
      pressure: { data: null, error: "pressure unavailable", loading: false },
      readiness: { data: null, error: null, loading: true },
      notice: <p>operator notice</p>,
    })} />);
    expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Request log" })).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("pressure unavailable")).toBeVisible();
    expect(screen.getByText("Loading…")).toBeVisible();
    expect(screen.getByText("operator notice")).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Incident summary" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Anomalies & remediation" }));
    expect(screen.getByRole("heading", { name: "Incident summary" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Runtime pressure" })).toBeVisible();
  });

  it("merges controlled filters and retains callbacks across collapse and reopen", () => {
    const props = fixture({ requestFilters: { ...OPERATIONS_DEFAULT_REQUEST_FILTERS, projectId: "project-a" } });
    render(<OperationsWorkspace {...props} />);
    const toggle = screen.getByRole("button", { name: "Request log" });
    fireEvent.click(toggle);
    fireEvent.change(screen.getByLabelText("Status"), { target: { value: "failed" } });
    expect(props.onRequestFiltersChange).toHaveBeenCalledWith({ ...props.requestFilters, status: "failed" });
    fireEvent.click(screen.getByRole("button", { name: "Apply filters" }));
    expect(props.onApplyRequestFilters).toHaveBeenCalledTimes(1);
    fireEvent.click(toggle);
    expect(screen.queryByLabelText("Project")).not.toBeInTheDocument();
    fireEvent.click(toggle);
    expect(screen.getByLabelText("Project")).toHaveValue("project-a");
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(props.onRequestFiltersChange).toHaveBeenLastCalledWith(OPERATIONS_DEFAULT_REQUEST_FILTERS);
  });

  it("keeps refresh and request mutation locks independent", () => {
    const props = fixture({ editorLocked: true });
    const { rerender } = render(<OperationsWorkspace {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Request log" }));
    expect(screen.getByRole("button", { name: "Apply filters" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reset" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(props.onRefresh).toHaveBeenCalledTimes(1);
    rerender(<OperationsWorkspace {...props} refreshing />);
    expect(screen.getByRole("button", { name: "Refreshing…" })).toBeDisabled();
  });
});
