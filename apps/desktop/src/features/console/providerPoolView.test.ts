import { describe, expect, it } from "vitest";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import { buildAccountLedgerRows } from "./accountManagementViewModel";
import { buildRouteAccountCatalog } from "./routeDocumentAccountCatalog";
import { matchesProviderSite, providerPoolEntries, providerPoolTableRows, type ProviderPoolSort } from "./providerPoolView";

const a = pilotSection({ providerId: "site-a", providerLabel: "Alpha", hostLabel: "alpha.example",
  directAccounts: [pilotAccount({ accountId: "a", requestCount: 20, successWindows: [{ label: "today", success: 9, requests: 10 }] })] });
const b = pilotSection({ providerId: "site-b", providerLabel: "Beta", hostLabel: "beta.example",
  directAccounts: [pilotAccount({ accountId: "b", requestCount: 100, successWindows: [{ label: "today", success: 1, requests: 10 }] }), pilotAccount({ accountId: "b2" })] });
const unknown = pilotSection({ providerId: "unknown", providerLabel: "Unknown", directAccounts: [pilotAccount({ usageWindowBadges: [] })] });
const ids = (sort: ProviderPoolSort) => providerPoolEntries([b, unknown, a], { query: "", sort }).map((entry) => entry.section.providerId);

describe("provider pool site view", () => {
  it("matches public site names, IDs and domains without filtering account contents", () => {
    expect(matchesProviderSite(a, " ALPHA.EXAMPLE ")).toBe(true);
    expect(matchesProviderSite(a, "site-a alpha")).toBe(true);
    expect(matchesProviderSite(a, "Account 1")).toBe(false);
    expect(providerPoolEntries([a, b], { query: "beta", sort: "default" })[0].snapshot.accounts).toHaveLength(2);
    expect(providerPoolEntries([a, b], { query: "not-found", sort: "default" })).toEqual([]);
  });
  it("sorts by displayed metrics with unknown data last and leaves source order intact", () => {
    expect(ids("default")).toEqual(["site-b", "unknown", "site-a"]);
    expect(ids("name")).toEqual(["site-a", "site-b", "unknown"]);
    expect(ids("name-desc")).toEqual(["unknown", "site-b", "site-a"]);
    expect(ids("available")[0]).toBe("site-b");
    expect(ids("success")).toEqual(["site-a", "site-b", "unknown"]);
    expect(ids("requests")).toEqual(["site-b", "site-a", "unknown"]);
    expect(providerPoolEntries([b, a], { query: "", sort: "default" }).map((entry) => entry.section)).toEqual([b, a]);
  });
  it("filters legacy table sites as whole groups", () => {
    const rows = buildAccountLedgerRows(buildRouteAccountCatalog({ providers: [
      { id: "a", label: "Alpha", base_url: "https://alpha.example", credentials: [{ id: "a1" }, { id: "a2" }] },
      { id: "b", label: "Beta", base_url: "https://beta.example", api_key: "fixture" },
    ], model_routes: [], aliases: {} }), {});
    const visible = providerPoolTableRows(rows, { query: "alpha.example", sort: "name" });
    expect(visible.map((row) => row.accountId)).toEqual(["a1", "a2"]);
    expect(rows).toHaveLength(3);
  });
});
