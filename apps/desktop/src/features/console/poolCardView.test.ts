import { expect, it } from "vitest";
import { visibleEntitlementCards, visibleModelCards } from "./poolCardView";
import { buildCredentialGroupCardSnapshot } from "./credentialGroupCardSnapshot";
import type { ModelPoolCard } from "./modelPoolViewModel";
import type { ProviderAggregateMetrics } from "./providerCardMetrics";

const metrics = (requests: number | null, successRate: number | null): ProviderAggregateMetrics => ({
  requests, successRate, concurrency: null, upstreamCost: null, platformRevenue: null,
  successWindows: [], successSuccessCount: 0, successRequestCount: 0,
});
function model(name: string, requests: number | null, rate: number | null, enabled = 1): ModelPoolCard {
  return { rowId: name, model: name, enabled: true, pinned: false, accountCount: 2,
    enabledAccountCount: enabled, routedPatterns: [name], metrics: metrics(requests, rate),
    chain: [{ providerId: "site", providerLabel: "My Site", accountCount: 2,
      enabledAccountCount: enabled, accountIds: ["private-account"], detached: false, metrics: null }] };
}

it("filters public model metadata and sorts cards without changing routing chains", () => {
  const cards = [model("model-10", null, null), model("model-2", 20, 0.9, 3), model("model-1", 0, 0.5)];
  const before = JSON.stringify(cards);
  const names = (sort: "default" | "name" | "name-desc" | "requests" | "success" | "available") =>
    visibleModelCards(cards, { query: "", sort }).map((card) => card.model);
  expect(names("default")).toEqual(["model-10", "model-2", "model-1"]);
  expect(names("name")).toEqual(["model-1", "model-2", "model-10"]);
  expect(names("name-desc")).toEqual(["model-10", "model-2", "model-1"]);
  expect(names("requests")).toEqual(["model-2", "model-1", "model-10"]);
  expect(names("success")).toEqual(["model-2", "model-1", "model-10"]);
  expect(names("available")).toEqual(["model-2", "model-10", "model-1"]);
  expect(visibleModelCards(cards, { query: "MODEL-2 my SITE", sort: "default" })).toEqual([cards[1]]);
  expect(visibleModelCards(cards, { query: "private-account", sort: "default" })).toEqual([]);
  expect(JSON.stringify(cards)).toBe(before);
});

it("keeps entitlement snapshots paired during filtering and ranking, excluding private notes", () => {
  const snapshots = ["Beta", "Alpha"].map((name, index) => buildCredentialGroupCardSnapshot({
    rowId: `row-${index}`, groupId: `pool-${index}`, name, description: "Shared entitlement",
    billingMultiplier: "1", enabled: true, notes: "private-note", memberCount: index + 1,
    providerLabels: ["My Site"], modelLabels: ["gpt-6"], providerScopes: [], modelScopes: [],
    metrics: metrics(index * 10, index ? 0.9 : null),
  }, (zh) => zh, ["deselected-provider"]));
  expect(visibleEntitlementCards(snapshots, { query: "alpha gpt-6", sort: "requests" })).toEqual([snapshots[1]]);
  for (const sort of ["name", "requests", "success", "available"] as const) {
    const result = visibleEntitlementCards(snapshots, { query: "", sort });
    expect(result).toEqual([snapshots[1], snapshots[0]]);
    expect(result[0]).toBe(snapshots[1]);
  }
  expect(visibleEntitlementCards(snapshots, { query: "private-note", sort: "default" })).toEqual([]);
  expect(snapshots[0].group.name).toBe("Beta");
});
