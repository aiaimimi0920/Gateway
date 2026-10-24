import type { AccountLedgerRow } from "./accountManagementViewModel";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";
import { buildProviderCardSnapshot } from "./providerCardSnapshot";

export type ProviderPoolSort = "default" | "name" | "name-desc" | "available" | "success" | "requests";
export type ProviderPoolView = { query: string; sort: ProviderPoolSort };
type Site = { providerId: string; providerLabel: string; vendorLabel: string; hostLabel: string | null; providerPreset: string | null };
type RankedSite = { site: Site; index: number; available: number; success: number | null; requests: number | null };

/** Filter public site metadata only; credentials and account contents are not search inputs. */
export function matchesProviderSite(site: Site, query: string): boolean {
  const text = [site.providerLabel, site.providerId, site.vendorLabel, site.hostLabel, site.providerPreset]
    .filter(Boolean).join(" ").toLocaleLowerCase();
  return query.trim().toLocaleLowerCase().split(/\s+/).every((word) => text.includes(word));
}

function descending(a: number | null, b: number | null): number {
  if (a === null) return b === null ? 0 : 1;
  if (b === null) return -1;
  return b - a;
}

function compare(a: RankedSite, b: RankedSite, sort: ProviderPoolSort): number {
  let order = 0;
  if (sort === "name" || sort === "name-desc") {
    order = a.site.providerLabel.localeCompare(b.site.providerLabel, undefined, { numeric: true, sensitivity: "base" });
    if (sort === "name-desc") order = -order;
  } else if (sort === "available") order = descending(a.available, b.available);
  else if (sort === "success") order = descending(a.success, b.success);
  else if (sort === "requests") order = descending(a.requests, b.requests);
  // Keep the original display order on ties; unknown telemetry belongs after known values, not at zero.
  return order || a.index - b.index;
}

export function providerPoolEntries(sections: AccountsLedgerPilotSection[], view: ProviderPoolView) {
  return sections.filter((section) => matchesProviderSite(section, view.query)).map((section, index) => {
    const snapshot = buildProviderCardSnapshot(section);
    return { section, snapshot, site: section, index, available: snapshot.availablePoolCount,
      success: snapshot.successRate, requests: snapshot.requestCount };
  }).sort((a, b) => compare(a, b, view.sort));
}

/** Legacy table-only sites obey the same site filter without filtering individual accounts. */
export function providerPoolTableRows(rows: AccountLedgerRow[], view: ProviderPoolView): AccountLedgerRow[] {
  const sites = new Map<string, { rows: AccountLedgerRow[]; rank: RankedSite }>();
  for (const row of rows) {
    if (!matchesProviderSite(row, view.query)) continue;
    let entry = sites.get(row.providerId);
    if (!entry) {
      entry = { rows: [], rank: { site: row, index: sites.size, available: 0, success: null, requests: null } };
      sites.set(row.providerId, entry);
    }
    entry.rows.push(row);
    if (row.enabled) entry.rank.available += 1;
  }
  return [...sites.values()].sort((a, b) => compare(a.rank, b.rank, view.sort)).flatMap((entry) => entry.rows);
}
