import { describe, expect, it } from "vitest";

import {
  bootstrapStatusSchema,
  consoleAccessCatalogSchema,
  consoleAccountGroupSummaryResponseSchema,
  consoleEventSchema,
  consoleNullableAccessKeyBalanceSchema,
  consoleRequestAuditFullSummaryResponseSchema,
  operationSuccessSchema,
} from "./schemas";

describe("console wire schemas", () => {
  it("normalizes bootstrap compatibility defaults", () => {
    expect(bootstrapStatusSchema.parse({ needsBootstrap: true })).toEqual({
      needsBootstrap: true,
      managementConfigured: false,
      environmentOverride: false,
    });
    expect(bootstrapStatusSchema.parse({ needsBootstrap: false })).toEqual({
      needsBootstrap: false,
      managementConfigured: true,
      environmentOverride: false,
    });
  });

  it("normalizes both operation success envelopes", () => {
    expect(operationSuccessSchema.parse({ success: true, message: "saved" })).toEqual({
      success: true,
      message: "saved",
    });
    expect(operationSuccessSchema.parse({ ok: true })).toEqual({
      success: true,
      message: undefined,
    });
  });

  it("normalizes event identifiers and missing data", () => {
    expect(
      consoleEventSchema.parse({ id: 42, kind: "updated", timestamp: "2026-09-04T00:00:00Z" }),
    ).toEqual({
      id: "42",
      kind: "updated",
      timestamp: "2026-09-04T00:00:00Z",
      data: {},
    });
  });

  it("keeps legacy accounts routable when enabled is absent", () => {
    const response = consoleAccountGroupSummaryResponseSchema.parse({
      summary: {
        routeConfigRevision: "revision-1",
        source: "runtime",
        accountGroups: [],
        providers: [],
        accounts: [
          {
            id: "account-1",
            displayName: "Account 1",
            providerId: "provider-1",
            providerLabel: "Provider 1",
            mode: "api",
            supportedModels: [],
            groupIds: [],
          },
        ],
      },
    });

    expect(response.summary.accounts[0]?.enabled).toBe(true);
  });

  it("normalizes missing audit breakdowns to empty arrays", () => {
    const response = consoleRequestAuditFullSummaryResponseSchema.parse({
      summary: {
        totalRequests: 0,
        completedCount: 0,
        failedCount: 0,
        cancelledCount: 0,
        runningCount: 0,
        fallbackEligibleFailures: 0,
        fallbackExhaustedFailures: 0,
      },
    });

    expect(response.summary).toMatchObject({
      byStatus: [],
      byProviderAccount: [],
      byEndpointKind: [],
      byErrorCode: [],
      providerAccounts: [],
    });
  });

  it("normalizes an empty access catalog", () => {
    expect(consoleAccessCatalogSchema.parse({})).toEqual({
      providerCapabilities: [],
      platformAccessRows: [],
      bundles: [],
      bundleItems: [],
      accessKeys: [],
      keyBundleBindings: [],
      balances: [],
      aggregateMemberships: [],
    });
  });

  it("normalizes a missing access-key balance to null", () => {
    expect(consoleNullableAccessKeyBalanceSchema.parse(undefined)).toBeNull();
  });
});
