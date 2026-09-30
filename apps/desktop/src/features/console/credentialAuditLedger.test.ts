import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { consoleRequestAuditSummaryResponseSchema } from "../../api/schemas/audit";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildAccountLedgerRows, buildRouteAccountCatalog } from "./accountManagementViewModel";
import { buildConsoleTelemetrySnapshot } from "./telemetry";

const document: ConsoleRouteDocument = {
  providers: [{ id: "nvidia", preset: "nvidia", base_url: "https://integrate.api.nvidia.com/v1",
    credentials: ["one", "two"].map((id) => ({ id, enabled: true })),
  }], model_routes: [], aliases: {}, account_groups: [],
};

function stats(totalRequests: number) {
  return { providerAccountId: "nvidia", totalRequests, completedCount: totalRequests,
    failedCount: 0, cancelledCount: 0, runningCount: 0,
    lastRequestAt: "2026-09-27T01:00:00Z", models: [{
      model: "nvidia/nemotron-3-super-120b-a12b", totalRequests, completedCount: totalRequests,
      failedCount: 0, cancelledCount: 0, runningCount: 0, lastRequestAt: null,
      windows: [{ label: "01:00", bucketStart: "2026-09-27T01:00:00Z",
        totalRequests, successCount: totalRequests, failureCount: 0 }],
    }],
    windows: [{ label: "01:00", bucketStart: "2026-09-27T01:00:00Z",
      totalRequests, successCount: totalRequests, failureCount: 0 }],
  };
}

describe("credential audit to ledger", () => {
  it("keeps each NVIDIA account's calls separate from the provider rollup", () => {
    const { summary } = consoleRequestAuditSummaryResponseSchema.parse({ summary: {
      ...stats(7), providerAccounts: [stats(7)],
      credentials: [{ ...stats(2), credentialRef: "one" }, { ...stats(5), credentialRef: "two" }],
    } });
    const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
      credentialModelStates: [], credentialInventory: null, requestAuditSummary: summary,
    });
    const sections = buildAccountLedgerSections(document,
      buildAccountLedgerRows(buildRouteAccountCatalog(document), {}), new Map(), snapshot, "zh-CN");
    const accounts = sections.flatMap((section) => [
      ...section.directAccounts, ...section.identityCategories.flatMap((category) => category.accounts),
    ]);
    expect(accounts.map(({ accountId, requestCount }) => ({ accountId, requestCount })))
      .toEqual([{ accountId: "one", requestCount: 2 }, { accountId: "two", requestCount: 5 }]);
    expect(accounts.map((account) => account.successWindows?.[0].requests)).toEqual([2, 5]);
    expect(accounts.map((account) => account.modelTraffic?.models.get("nvidia/nemotron-3-super-120b-a12b")?.requestCount))
      .toEqual([2, 5]);
    expect(sections[0].modelTraffic?.models.get("nvidia/nemotron-3-super-120b-a12b")?.successWindows[0].requests).toBe(7);
    expect(accounts.every((account) => account.upstreamCost === null)).toBe(true);
    expect(accounts.every((account) => account.statusLabel === "待观测")).toBe(true);
  });
});
