import { describe, expect, it } from "vitest";
import { createConsoleApi } from "./console";
import type { GatewayApiClient } from "./client";
import { consoleRequestAuditSummaryResponseSchema } from "./schemas/audit";

const summary = { totalRequests: 4, completedCount: 3, failedCount: 1,
  cancelledCount: 0, runningCount: 0, providerAccounts: [] };
const total = { providerAccountId: "pool", credentialRef: null, model: "model",
  requestCount: 1400, successCount: 1200 };

describe("retained model traffic contract", () => {
  it("requests unsampled model totals separately from the recent row/time filter", async () => {
    const requests: string[] = [];
    const client: GatewayApiClient = { async request<T>(path: string) {
      requests.push(path);
      return undefined as T;
    } };
    await createConsoleApi(client).getRequestAuditSummary!("token", { limit: 5, createdFrom: "2026-10-04T00:00:00Z" });
    const url = new URL(requests[0], "http://localhost");
    expect(url.searchParams.get("includeModelTotals")).toBe("true");
    expect(url.searchParams.get("limit")).toBe("5");
    expect(url.searchParams.get("createdFrom")).toBe("2026-10-04T00:00:00Z");
  });

  it("accepts old servers without falsely claiming complete totals", () => {
    expect(consoleRequestAuditSummaryResponseSchema.parse({ summary }).summary.retainedModelTotals).toBeUndefined();
  });

  it("keeps retained counters distinct from the sampled summary and validates their bounds", () => {
    const value = consoleRequestAuditSummaryResponseSchema.parse({ summary: { ...summary, retainedModelTotals: [total] } });
    expect(value.summary.totalRequests).toBe(4);
    expect(value.summary.retainedModelTotals).toEqual([total]);
    for (const invalid of [{ requestCount: -1 }, { successCount: 1401 }, { successCount: 0.5 }, { credentialRef: "" }]) {
      expect(consoleRequestAuditSummaryResponseSchema.safeParse({ summary: { ...summary,
        retainedModelTotals: [{ ...total, ...invalid }] } }).success).toBe(false);
    }
  });
});
