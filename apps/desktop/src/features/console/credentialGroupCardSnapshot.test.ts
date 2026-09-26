import { describe, expect, it } from "vitest";

import type { CredentialGroupDirectoryItem } from "./accountManagementViewModel";
import { buildCredentialGroupCardSnapshot } from "./credentialGroupCardSnapshot";

const group: CredentialGroupDirectoryItem = {
  rowId: "row premium/1",
  groupId: "premium",
  name: "Premium",
  description: "",
  billingMultiplier: "1.5",
  enabled: true,
  notes: "",
  memberCount: 2,
  providerLabels: ["Anthropic", "OpenAI"],
  modelLabels: ["claude-4.5", "gpt-5"],
  providerScopes: [
    {
      providerId: "anthropic",
      providerLabel: "Anthropic",
      accountCount: 1,
      providerAccountCount: 1,
      modelCount: 1,
    },
    {
      providerId: "openai",
      providerLabel: "OpenAI",
      accountCount: 1,
      providerAccountCount: 1,
      modelCount: 1,
    },
  ],
  modelScopes: [],
  metrics: {
    concurrency: { used: 1, total: 4 },
    upstreamCost: 1,
    platformRevenue: 2,
    requests: 10,
    successWindows: [{ label: "10:00", success: 9, requests: 10, rate: 0.9 }],
    successSuccessCount: 9,
    successRequestCount: 10,
    successRate: 0.9,
  },
};

describe("buildCredentialGroupCardSnapshot", () => {
  it("precomputes stable card fields and provider selection", () => {
    const snapshot = buildCredentialGroupCardSnapshot(group, (_zh, en) => en, ["anthropic"]);

    expect(snapshot.label).toBe("Premium");
    expect(snapshot.accountPanelId).toBe("entitlement-group-accounts-row-premium-1");
    expect(snapshot.cardMenuKey).toBe("entitlement-group-card:row premium/1");
    expect(snapshot.providerScopeNarrowed).toBe(true);
    expect([...snapshot.selectedProviderIds]).toEqual(["openai"]);
    expect(snapshot.successWindows).toEqual(group.metrics?.successWindows);
    expect(snapshot.availabilityCells).toHaveLength(12);
  });

  it("uses the translated fallback and selects every provider by default", () => {
    const snapshot = buildCredentialGroupCardSnapshot(
      { ...group, name: "", groupId: "" },
      (_zh, en) => en,
    );

    expect(snapshot.label).toBe("New group");
    expect(snapshot.providerScopeNarrowed).toBe(false);
    expect([...snapshot.selectedProviderIds]).toEqual(["anthropic", "openai"]);
  });
});
