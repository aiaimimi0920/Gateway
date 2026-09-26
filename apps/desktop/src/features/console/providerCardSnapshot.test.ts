import { describe, expect, it } from "vitest";

import type { AccountsLedgerPilotAccount } from "./ProviderAccountCard";
import { buildProviderCardSnapshot } from "./providerCardSnapshot";

function account(
  accountId: string,
  overrides: Partial<AccountsLedgerPilotAccount> = {},
): AccountsLedgerPilotAccount {
  return {
    accountId,
    providerId: "provider-1",
    displayName: accountId,
    mode: "credential",
    enabled: true,
    logicalLabels: [],
    capacityLabel: "1 slot",
    statusLabel: "正常",
    dispatchEnabled: true,
    dispatchEditable: true,
    previewOnly: false,
    usageWindowBadges: [],
    recentUseLabel: "never",
    verificationStatus: "not-tested",
    verificationFamilies: [],
    verificationCheckedAt: null,
    verificationEvidenceRef: null,
    verificationNote: "",
    ...overrides,
  };
}

describe("buildProviderCardSnapshot", () => {
  it("deduplicates category overlap and resolves stable card aggregates", () => {
    const shared = account("shared", {
      requestCount: 7,
      upstreamCost: 1.25,
      userCost: 2.5,
      successWindows: [{ label: "10:00", success: 3, requests: 4 }],
      quotaRemainingUsd: 4,
    });
    const recovering = account("recovering", {
      statusLabel: "限流等待恢复",
      requestCount: 2,
      dispatchEnabled: false,
    });

    const snapshot = buildProviderCardSnapshot({
      supportsIdentityCategories: true,
      identityCategories: [{ accounts: [shared] }, { accounts: [shared, recovering] }],
      directAccounts: [],
      poolTargetSize: 4,
      telemetry: null,
    });

    expect(snapshot.accounts.map((item) => item.accountId)).toEqual(["shared", "recovering"]);
    expect(snapshot.availablePoolCount).toBe(1);
    expect(snapshot.poolSegments.segments.map((segment) => segment.count)).toEqual([1, 1, 0, 2]);
    expect(snapshot.requestCount).toBe(9);
    expect(snapshot.costs).toEqual({ upstream: 1.25, user: 2.5 });
    expect(snapshot.successRate).toBe(0.75);
    expect(snapshot.quotaRemainingUsd).toBe(4);
    expect(snapshot.actionableAccounts).toHaveLength(2);
    expect(snapshot.hasProbeTargets).toBe(true);
  });
});
