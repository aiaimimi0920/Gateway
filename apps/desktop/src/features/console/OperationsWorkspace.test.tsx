import { fireEvent, screen, within } from "@testing-library/react";
import { renderNeuro as render } from "../../test/renderNeuro";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleAnomalyIncident } from "../../api/contracts";
import {
  operationsRequestFixture,
  operationsSummaryFixture,
} from "./OperationsWorkspace.fixtures";

import {
  OPERATIONS_DEFAULT_REQUEST_FILTERS,
  OperationsWorkspace,
  type OperationsWorkspaceProps,
} from "./OperationsWorkspace";

function fixture(
  overrides: Partial<OperationsWorkspaceProps> = {},
): OperationsWorkspaceProps {
  const empty = { data: null, error: null, loading: false };
  return {
    t: (_zh, en) => en,
    editorLocked: false,
    refreshing: false,
    onRefresh: vi.fn(),
    pressure: empty,
    readiness: empty,
    operatorSummary: { ...empty, data: operationsSummaryFixture() },
    requests: empty,
    requestSummary: empty,
    usageSummary: empty,
    promptCache: empty,
    incidents: empty,
    incidentSummary: empty,
    alertQueue: empty,
    policies: empty,
    remediationQueue: empty,
    remediationRuns: empty,
    remediationEffectiveness: empty,
    hotspots: empty,
    exports: empty,
    exportInventory: empty,
    requestFilters: { ...OPERATIONS_DEFAULT_REQUEST_FILTERS },
    onRequestFiltersChange: vi.fn(),
    onApplyRequestFilters: vi.fn(),
    incidentBusyId: null,
    onAcknowledgeIncident: vi.fn(),
    onResolveIncident: vi.fn(),
    ...overrides,
  };
}

function incident(
  id: string,
  status: string,
  lastSeenAt: string,
): ConsoleAnomalyIncident {
  return {
    id,
    status,
    lastSeenAt,
    code: id,
    fingerprint: id,
    summary: id,
    policyId: null,
    projectId: null,
    routePolicyId: null,
    tag: null,
    textMode: null,
    severity: "high",
    ownerUserId: null,
    followUpStatus: "pending",
    syncHitCount: 1,
    escalationStatus: "none",
    escalatedAt: null,
    escalationReason: null,
    latestNote: null,
    resolutionNote: null,
    lastActionAt: null,
    lastAlertAttemptAt: null,
    lastAlertedAt: null,
    lastAlertSeverity: null,
    alertDeliveryCount: 0,
    latestExportId: null,
    previousExportId: null,
    latestValue: null,
    previousValue: null,
    deltaValue: null,
    deltaRatio: null,
    thresholdValue: null,
    firstSeenAt: lastSeenAt,
    acknowledgedAt: null,
    resolvedAt: null,
    createdAt: lastSeenAt,
    updatedAt: lastSeenAt,
  };
}

describe("operations workspace boundaries", () => {
  it("shows newest requests first and expands the recorded upstream failure in place", () => {
    const older = operationsRequestFixture("older");
    const newer = { ...operationsRequestFixture("newer"), createdAt: "2026-10-09T01:00:00Z" };
    const rows = [older, newer];
    const props = fixture({ requests: { data: rows, loading: false, error: null } });
    render(<OperationsWorkspace {...props} />);
    const toggles = screen.getAllByRole("button", { name: /Request details:/ });
    expect(toggles.map(node => node.getAttribute("aria-label"))).toEqual(["Request details: newer", "Request details: older"]);
    expect(screen.queryByRole("tab", { name: "Overview" })).not.toBeInTheDocument();
    fireEvent.click(toggles[0]);
    expect(screen.getByRole("region", { name: "Request details" })).toHaveTextContent("upstream rate limited");
    expect(toggles[0]).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(toggles[0]);
    expect(screen.queryByRole("region", { name: "Request details" })).not.toBeInTheDocument();
    expect(rows[0].id).toBe("older");
    expect(props.onApplyRequestFilters).not.toHaveBeenCalled();
  });

  it("filters loaded requests by model, provider and request identity and resets without a server mutation", () => {
    const props = fixture({ requests: { data: [
      { ...operationsRequestFixture("one"), requestedModel: "alpha", providerAccountId: "provider-a" },
      { ...operationsRequestFixture("two"), requestedModel: "beta", providerAccountId: "provider-b" },
    ], loading: false, error: null } });
    render(<OperationsWorkspace {...props} />);
    for (const query of ["ALPHA", "provider-a", "one"]) {
      fireEvent.change(screen.getByRole("textbox", { name: "Request ID" }), { target: { value: query } });
      expect(screen.getByRole("button", { name: "Request details: one" })).toBeVisible();
      expect(screen.queryByRole("button", { name: "Request details: two" })).not.toBeInTheDocument();
      expect(screen.getByText("Loaded records: 1 / 2")).toBeVisible();
    }
    fireEvent.change(screen.getByRole("textbox", { name: "Request ID" }), { target: { value: "missing" } });
    expect(screen.getByRole("status")).toHaveTextContent("No matching requests");
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(screen.getAllByRole("button", { name: /Request details:/ })).toHaveLength(2);
    expect(props.onRequestFiltersChange).not.toHaveBeenCalled();
    expect(props.onApplyRequestFilters).not.toHaveBeenCalled();
  });

  it("keeps read errors and notices visible without exposing removed diagnostic panels", () => {
    render(<OperationsWorkspace {...fixture({ requests: { data: null, error: "request read failed", loading: false }, notice: <p>operator notice</p> })} />);
    expect(screen.getByText("request read failed")).toBeVisible();
    expect(screen.getByText("operator notice")).toBeVisible();
    expect(screen.queryByText("Usage archive")).not.toBeInTheDocument();
    expect(screen.queryByText("Dependency & configuration checks")).not.toBeInTheDocument();
  });

  it("omits database incident actions for a local-only gateway", () => {
    render(<OperationsWorkspace {...fixture({ operatorSummary: { data: operationsSummaryFixture(false), error: null, loading: false } })} />);
    expect(screen.queryByRole("button", { name: "Incident records" })).not.toBeInTheDocument();
  });
  it("applies explicit incident search/order without changing summary badges, errors or mutation targets", () => {
    const props = fixture({
      incidents: { data: [incident("resolved-z", "resolved", "2026-10-24T09:00:00Z"), incident("open-a", "open", "2026-10-24T08:00:00Z")], error: "incident read warning", loading: false },
    });
    const { container, rerender } = render(<OperationsWorkspace {...props} listView={{ query: "", sort: "newest" }} />);
    fireEvent.click(screen.getByRole("button", { name: "Incident records" }));
    const codes = () => screen.queryAllByRole("rowheader").map((row) => row.textContent);
    expect(codes()).toEqual(["resolved-z", "open-a"]);
    rerender(<OperationsWorkspace {...props} listView={{ query: "OPEN-A", sort: "name" }} />);
    expect(codes()).toEqual(["open-a"]);
    fireEvent.click(screen.getByRole("button", { name: "Resolve" }));
    expect(props.onResolveIncident).toHaveBeenCalledWith("open-a");
    rerender(<OperationsWorkspace {...props} listView={{ query: "missing", sort: "name" }} />);
    expect(codes()).toEqual([]);
    expect(screen.getByText("incident read warning")).toBeVisible();
    expect(props.onApplyRequestFilters).not.toHaveBeenCalled();
  });




  it("orders actionable incidents without mutating input and enforces per-row action locks", () => {
    const rows = [
      incident("resolved-case", "resolved", "2026-09-09T12:00:00Z"),
      incident("open-old", "open", "2026-09-09T09:00:00Z"),
      incident("ack-case", "acknowledged", "2026-09-09T11:00:00Z"),
      incident("open-new", "OPEN", "2026-09-09T10:00:00Z"),
    ];
    Object.freeze(rows);
    const props = fixture({
      incidents: { data: rows, error: "stale incident data", loading: true },
    });
    const { container, rerender } = render(<OperationsWorkspace {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Incident records" }));
    expect(screen.getByText("stale incident data")).toBeVisible();
    expect(
      Array.from(
        within(screen.getByRole("table", { name: "Incidents" })).getAllByRole("rowheader"),
      ).map((row) => row.textContent),
    ).toEqual(["open-new", "open-old", "ack-case", "resolved-case"]);
    const rowFor = (id: string) => {
      const row = screen.getByRole("rowheader", { name: id }).closest("tr");
      if (!(row instanceof HTMLElement))
        throw new Error("Missing incident row");
      return within(row);
    };
    fireEvent.click(
      rowFor("open-new").getByRole("button", { name: "Acknowledge" }),
    );
    expect(props.onAcknowledgeIncident).toHaveBeenCalledWith("open-new");
    expect(
      rowFor("ack-case").getByRole("button", { name: "Acknowledge" }),
    ).toBeDisabled();
    fireEvent.click(
      rowFor("ack-case").getByRole("button", { name: "Resolve" }),
    );
    expect(props.onResolveIncident).toHaveBeenCalledWith("ack-case");
    expect(
      rowFor("resolved-case").getByRole("button", { name: "Resolve" }),
    ).toBeDisabled();
    rerender(<OperationsWorkspace {...props} incidentBusyId="open-new" />);
    expect(
      rowFor("open-new").getByRole("button", { name: "Resolve" }),
    ).toBeDisabled();
    expect(
      rowFor("open-old").getByRole("button", { name: "Resolve" }),
    ).toBeEnabled();
    rerender(<OperationsWorkspace {...props} editorLocked />);
    expect(
      rowFor("open-old").getByRole("button", { name: "Acknowledge" }),
    ).toBeDisabled();
    expect(rows[0].id).toBe("resolved-case");
  });





});
