/** Display-only pool filtering/sorting; never changes membership or routing priority. */
import type { CredentialGroupCardSnapshot } from "./credentialGroupCardSnapshot";
import type { ModelPoolCard } from "./modelPoolViewModel";
import type { ProviderPoolView } from "./providerPoolView";

export const DEFAULT_POOL_CARD_VIEW: ProviderPoolView = { query: "", sort: "default" };
type CardRank = { name: string; search: string[]; count: number; requests: number | null; success: number | null };

function descending(a: number | null, b: number | null): number {
  if (a === null || !Number.isFinite(a)) return b === null || !Number.isFinite(b) ? 0 : 1;
  if (b === null || !Number.isFinite(b)) return -1;
  return b - a;
}

function visibleCards<T>(cards: readonly T[], view: ProviderPoolView, rank: (card: T) => CardRank): T[] {
  const words = view.query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return cards.map((card, index) => ({ card, index, ...rank(card) }))
    .filter((entry) => {
      const text = entry.search.join(" ").toLocaleLowerCase();
      return words.every((word) => text.includes(word));
    })
    .sort((a, b) => {
      let order = 0;
      if (view.sort === "name" || view.sort === "name-desc") {
        order = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });
        if (view.sort === "name-desc") order = -order;
      } else if (view.sort === "available") order = descending(a.count, b.count);
      else if (view.sort === "requests") order = descending(a.requests, b.requests);
      else if (view.sort === "success") order = descending(a.success, b.success);
      return order || a.index - b.index;
    }).map((entry) => entry.card);
}

export function visibleModelCards(cards: readonly ModelPoolCard[], view: ProviderPoolView): ModelPoolCard[] {
  return visibleCards(cards, view, (card) => ({
    name: card.model,
    search: [card.model, ...card.chain.flatMap((link) => [link.providerId, link.providerLabel])],
    count: card.enabledAccountCount, requests: card.metrics?.requests ?? null, success: card.metrics?.successRate ?? null,
  }));
}

export function visibleEntitlementCards(cards: readonly CredentialGroupCardSnapshot[], view: ProviderPoolView): CredentialGroupCardSnapshot[] {
  return visibleCards(cards, view, ({ group, label }) => ({
    name: label, search: [label, group.groupId, group.description, ...group.providerLabels, ...group.modelLabels],
    count: group.memberCount, requests: group.metrics?.requests ?? null, success: group.metrics?.successRate ?? null,
  }));
}
