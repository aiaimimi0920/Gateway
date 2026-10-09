import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import { consoleOperatorSummaryResponseSchema } from "../../api/schemas/operations";
import { operationsSummaryFixture } from "./OperationsWorkspace.fixtures";
import { useOperationsData } from "./useOperationsData";

const databaseMethods = [
  "listAnomalyIncidents",
  "getAnomalyIncidentSummary",
  "getAnomalyAlertQueue",
  "listAnomalyPolicies",
  "listRemediationQueue",
  "listRemediationRuns",
  "getRemediationEffectiveness",
  "listPersistedAnalysisExports",
  "getAnalysisExportInventorySummary",
  "getRateLimitHotspots",
  "getUsageAggregateSummary",
  "getPromptCacheSummary",
] as const;

function client(postgresql: boolean) {
  const advanced = Object.fromEntries(
    databaseMethods.map((name) => [
      name,
      vi.fn().mockRejectedValue(new Error("real server query failed")),
    ]),
  ) as Record<(typeof databaseMethods)[number], ReturnType<typeof vi.fn>>;
  return {
    ...advanced,
    getOperatorSummary: vi
      .fn()
      .mockResolvedValue({ summary: operationsSummaryFixture(postgresql) }),
    listRequestAudits: vi.fn().mockResolvedValue({ requests: [] }),
    getRequestAuditFullSummary: vi.fn().mockResolvedValue({ summary: {} }),
  };
}

describe("operations storage availability", () => {
  it("preserves the actual SQLite dependency from the server response", () => {
    const parsed = consoleOperatorSummaryResponseSchema.parse({
      summary: operationsSummaryFixture(false),
    });
    expect(parsed.summary.readiness.dependencies.sqlite?.configured).toBe(true);
  });

  it("loads local request data but never calls PostgreSQL-only endpoints on SQLite", async () => {
    const api = client(false);
    const { result } = renderHook(() =>
      useOperationsData({
        api: api as unknown as ConsoleApi,
        managementToken: "synthetic",
        active: true,
      }),
    );
    await waitFor(() =>
      expect(result.current.panels.operatorSummary.data).not.toBeNull(),
    );
    await waitFor(() => expect(result.current.refreshing).toBe(false));
    expect(api.listRequestAudits).toHaveBeenCalledOnce();
    expect(result.current.panels.requests.data).toEqual([]);
    for (const name of databaseMethods)
      expect(api[name]).not.toHaveBeenCalled();
    expect(result.current.panels.incidents).toEqual({
      data: null,
      error: null,
      loading: false,
    });
  });

  it("does not mask genuine database errors when PostgreSQL is configured", async () => {
    const api = client(true);
    const { result } = renderHook(() =>
      useOperationsData({
        api: api as unknown as ConsoleApi,
        managementToken: "synthetic",
        active: true,
      }),
    );
    await waitFor(() =>
      expect(result.current.panels.incidents.error).toBe(
        "real server query failed",
      ),
    );
    for (const name of databaseMethods)
      expect(api[name]).toHaveBeenCalledOnce();
  });

  it("does not guess support when the capability source fails", async () => {
    const api = client(false);
    api.getOperatorSummary.mockRejectedValue(new Error("summary offline"));
    const { result } = renderHook(() =>
      useOperationsData({
        api: api as unknown as ConsoleApi,
        managementToken: "synthetic",
        active: true,
      }),
    );
    await waitFor(() =>
      expect(result.current.panels.operatorSummary.error).toBe(
        "summary offline",
      ),
    );
    for (const name of databaseMethods)
      expect(api[name]).not.toHaveBeenCalled();
    expect(api.listRequestAudits).toHaveBeenCalledOnce();
  });
});
