import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { OperationsAnomalyPanels } from "./OperationsAnomalyPanels";
import {
  operationsRequestFixture,
  operationsSummaryFixture,
} from "./OperationsWorkspace.fixtures";

function props(
  postgresql = false,
): ComponentProps<typeof OperationsAnomalyPanels> {
  const summary = operationsSummaryFixture(postgresql);
  const empty = { data: null, loading: false, error: null };
  return {
    t: (_, en) => en,
    editorLocked: false,
    incidentBusyId: null,
    onOpenRequests: vi.fn(),
    onAcknowledgeIncident: vi.fn(),
    onResolveIncident: vi.fn(),
    operatorSummary: { ...empty, data: summary },
    pressure: {
      ...empty,
      data: {
        totalRunningRequests: 0,
        totalProjectConcurrency: 0,
        totalProviderConcurrency: 0,
        providers: [],
        projects: [],
      },
    },
    readiness: {
      ...empty,
      data: {
        ok: true,
        dependencies: summary.readiness.dependencies,
        checks: {
          database: true,
          databaseConfigured: true,
          redis: false,
          objectStorage: true,
          apiKeySecret: false,
          publicBaseUrl: false,
          draining: false,
        },
        activeRequests: 0,
        draining: false,
        drainStartedAt: null,
        drainReason: null,
        providerStats: {},
      },
    },
    requests: { ...empty, data: [] },
    incidents: empty,
    incidentSummary: empty,
    alertQueue: empty,
    policies: empty,
    remediationEffectiveness: empty,
    remediationQueue: empty,
    remediationRuns: empty,
    hotspots: empty,
    exportInventory: empty,
    exports: empty,
  };
}

describe("operations anomaly layout", () => {
  it("uses real local observations rather than failed server-only cards", () => {
    const input = props();
    input.requests.data = Array.from({ length: 12 }, (_, index) =>
      operationsRequestFixture(String(index)),
    );
    render(<OperationsAnomalyPanels {...input} />);
    expect(screen.getByText("No runtime issues detected")).toBeVisible();
    expect(screen.getAllByText("upstream rate limited")).toHaveLength(10);
    expect(screen.getByText("12 failed out of 12 queried")).toBeVisible();
    expect(
      screen.getByText(/Server incident management is not enabled/),
    ).toBeVisible();
    expect(screen.queryByText("Read failed")).not.toBeInTheDocument();
    expect(screen.queryByText("Incident summary")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "View requests" }));
    expect(input.onOpenRequests).toHaveBeenCalledOnce();
    expect(input.onAcknowledgeIncident).not.toHaveBeenCalled();
  });

  it("keeps incomplete and failed checks distinct from healthy", () => {
    const input = props();
    input.pressure = {
      data: null,
      error: "pressure unavailable",
      loading: false,
    };
    input.operatorSummary = {
      data: null,
      error: "summary unavailable",
      loading: false,
    };
    render(<OperationsAnomalyPanels {...input} />);
    expect(
      screen.queryByText("No runtime issues detected"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("pressure unavailable")).toBeVisible();
    expect(screen.getByText("summary unavailable")).toBeVisible();
    expect(
      screen.queryByText(/Server incident management is not enabled/),
    ).not.toBeInTheDocument();
  });

  it("shows SQLite failure and provider breakers without treating unused dependencies as errors", () => {
    const input = props();
    input.readiness.data!.ok = false;
    input.readiness.data!.dependencies.sqlite!.ready = false;
    input.pressure.data!.providers = [
      {
        providerAccountId: "provider",
        label: "Account A",
        status: "active",
        protocolFamily: "openai",
        activeConcurrency: 0,
        concurrencyLimit: 1,
        concurrencyAvailable: 1,
        runningRequestCount: 0,
        breakerOpen: true,
      },
    ];
    render(<OperationsAnomalyPanels {...input} />);
    expect(screen.getByText("SQLite unavailable")).toBeVisible();
    expect(screen.getByText("Account A · Breaker open")).toBeVisible();
    expect(
      screen.queryByText("PostgreSQL unavailable"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("No runtime issues detected"),
    ).not.toBeInTheDocument();
  });

  it("separates server incidents, remediation and exports instead of stacking all panels", () => {
    render(<OperationsAnomalyPanels {...props(true)} />);
    expect(
      screen.getByRole("heading", { name: "Incident summary" }),
    ).toBeVisible();
    expect(screen.queryByText("Remediation impact")).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Automated remediation" }),
    );
    expect(screen.getByText("Remediation impact")).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Incident summary" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Analysis & exports" }));
    expect(screen.getByText("Export inventory")).toBeVisible();
    expect(screen.queryByText("Remediation impact")).not.toBeInTheDocument();
  });
});
