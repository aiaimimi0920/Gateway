import type { RouteAccountCatalog, RouteManagedAccount } from "./accountManagementViewModel";
import {
  aggregateProviderMetrics,
  type ProviderAggregateMetrics,
  type ProviderMetricsAccountLike,
  type ProviderMetricsResolver,
} from "./providerCardMetrics";

/**
 * A `model_routes` entry as the console holds it while editing: the parsed
 * pattern plus the raw record, so unknown keys survive a rewrite.
 */
export type ModelRouteRowLike = {
  id: string;
  pattern: string;
  route: Record<string, unknown>;
};

/** A `model_routes` entry reduced to the fields the model pool reasons about. */
export type ModelRouteChain = {
  rowId: string;
  pattern: string;
  providerIds: string[];
  priority: number;
  /**
   * `enabled` as the gateway reads it: absent means on. A disabled rule still
   * matches upstream, so the model it names dispatches nowhere.
   */
  enabled: boolean;
};

/** One link of a model's provider fallback chain. */
export type ModelPoolProviderLink = {
  providerId: string;
  providerLabel: string;
  accountCount: number;
  enabledAccountCount: number;
  /** Member account ids, so the expanded panel can mount the pool's cards. */
  accountIds: string[];
  /**
   * True when the model's own route lists this provider but no pooled account
   * serves the model through it. Kept visible rather than dropped so a rewrite
   * of the chain never silently deletes an operator's entry.
   */
  detached: boolean;
  metrics: ProviderAggregateMetrics | null;
};

/** One model card: every provider that can serve it, in dispatch order. */
export type ModelPoolCard = {
  /** The model name doubles as the row id: it is what the route pattern keys on. */
  rowId: string;
  model: string;
  /** Providers in fallback order — index 0 is tried first. */
  chain: ModelPoolProviderLink[];
  accountCount: number;
  enabledAccountCount: number;
  /** True when an exact-pattern route pins this order rather than inheriting it. */
  pinned: boolean;
  /**
   * False only when the model's own route is explicitly disabled. Inherited
   * chains are always on, matching the gateway's default.
   */
  enabled: boolean;
  /** Patterns of the routes that decided the order, for the card's own tooltip. */
  routedPatterns: string[];
  metrics: ProviderAggregateMetrics | null;
};

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

export function modelRoutePriority(route: Record<string, unknown>): number {
  const priority = route.priority;
  return typeof priority === "number" && Number.isFinite(priority) ? priority : 0;
}

export function modelRouteChainsFromRows(rows: readonly ModelRouteRowLike[]): ModelRouteChain[] {
  return rows.map((row) => ({
    rowId: row.id,
    pattern: row.pattern.trim(),
    providerIds: readStringArray(row.route.provider_ids),
    priority: modelRoutePriority(row.route),
    enabled: row.route.enabled !== false,
  }));
}

/**
 * Mirrors `glob_match` in `src/routing/config.rs`: `"*"` matches everything, a
 * trailing `*` is a prefix match, a leading `*` is a suffix match, and anything
 * else is exact. The order matters — `"*foo*"` is a prefix match on `"*foo"`
 * upstream, so it has to be one here too.
 */
export function modelPatternMatches(pattern: string, model: string): boolean {
  if (pattern === "*") {
    return true;
  }
  if (pattern.endsWith("*")) {
    return model.startsWith(pattern.slice(0, -1));
  }
  if (pattern.startsWith("*")) {
    return model.endsWith(pattern.slice(1));
  }
  return pattern === model;
}

type ResolvedChainOrder = {
  providerIds: string[];
  patterns: string[];
};

/**
 * The dispatch order the gateway would compute for `model`: matching routes
 * sorted by descending priority, then their provider ids concatenated with an
 * order-preserving dedupe. Same algorithm as `resolve` upstream, so the card
 * shows the chain that actually runs.
 */
function resolveChainOrder(chains: readonly ModelRouteChain[], model: string): ResolvedChainOrder {
  const matching = chains
    .filter((chain) => chain.pattern.length > 0 && modelPatternMatches(chain.pattern, model))
    .sort((left, right) => right.priority - left.priority);
  const seen = new Set<string>();
  const providerIds: string[] = [];
  const patterns: string[] = [];
  for (const chain of matching) {
    let contributed = false;
    for (const providerId of chain.providerIds) {
      if (seen.has(providerId)) {
        continue;
      }
      seen.add(providerId);
      providerIds.push(providerId);
      contributed = true;
    }
    if (contributed) {
      patterns.push(chain.pattern);
    }
  }
  return { providerIds, patterns };
}

type ProviderBucket = {
  providerLabel: string;
  accounts: RouteManagedAccount[];
};

type LinkContext = {
  /** The card's model, which is what the link's usage has to be scoped to. */
  model: string;
  metricsByAccountId: ReadonlyMap<string, ProviderMetricsAccountLike> | undefined;
  resolver: ProviderMetricsResolver | undefined;
};

function buildLink(
  providerId: string,
  bucket: ProviderBucket | undefined,
  context: LinkContext,
): ModelPoolProviderLink {
  const accounts = bucket?.accounts ?? [];
  const accountIds = accounts.map((account) => account.id);
  // One link is one provider account serving one model, which is exactly what
  // the resolver answers. The per-account map is the fallback for callers that
  // have no telemetry snapshot, and it reads `—` for pooled providers because
  // per-credential attribution is suppressed there.
  const resolved = context.resolver
    ? context.resolver.providerAccountModel(providerId, context.model, {
        credentialRefs: accountIds,
      })
    : null;
  const linkMetrics = resolved
    ? [resolved]
    : context.metricsByAccountId
      ? accounts
          .map((account) => context.metricsByAccountId?.get(account.id))
          .filter((metrics): metrics is ProviderMetricsAccountLike => Boolean(metrics))
      : [];
  return {
    providerId,
    providerLabel: bucket?.providerLabel ?? providerId,
    accountCount: accounts.length,
    enabledAccountCount: accounts.filter((account) => account.enabled).length,
    accountIds,
    detached: bucket == null,
    metrics: linkMetrics.length > 0 ? aggregateProviderMetrics(linkMetrics) : null,
  };
}

/**
 * One card per model the pool can serve, each carrying the ordered provider
 * chain. Models come from the accounts and from the exact-pattern routes, so a
 * provider that supports a model is listed whether or not a route mentions it,
 * and a model an operator added by hand is listed before any account backs it.
 * The routes decide the order.
 *
 * `providerOrder` is the `providers` order of the route document. It matters
 * because when no route matches a model the gateway falls back to every provider
 * that declares support, in document order — so that, not an alphabetical
 * guess, is the default chain an operator is looking at.
 */
export function buildModelPoolDirectory(
  catalog: RouteAccountCatalog,
  chains: readonly ModelRouteChain[],
  metricsByAccountId?: ReadonlyMap<string, ProviderMetricsAccountLike>,
  providerOrder: readonly string[] = [],
  resolver?: ProviderMetricsResolver,
): ModelPoolCard[] {
  const providerRank = new Map(providerOrder.map((providerId, index) => [providerId, index]));
  const compareUnrouted = (
    left: [string, ProviderBucket],
    right: [string, ProviderBucket],
  ): number => {
    const leftRank = providerRank.get(left[0]) ?? Number.MAX_SAFE_INTEGER;
    const rightRank = providerRank.get(right[0]) ?? Number.MAX_SAFE_INTEGER;
    return leftRank === rightRank
      ? left[1].providerLabel.localeCompare(right[1].providerLabel)
      : leftRank - rightRank;
  };
  const modelIndex = new Map<string, Map<string, ProviderBucket>>();
  for (const account of catalog.accounts) {
    // Dedupe per account: a credential may list the same model twice, which
    // would otherwise double count its accounts and its usage.
    for (const rawModel of new Set(account.supportedModels)) {
      const model = rawModel.trim();
      if (model.length === 0) {
        continue;
      }
      let byProvider = modelIndex.get(model);
      if (!byProvider) {
        byProvider = new Map<string, ProviderBucket>();
        modelIndex.set(model, byProvider);
      }
      const bucket = byProvider.get(account.providerId);
      if (bucket) {
        bucket.accounts.push(account);
      } else {
        byProvider.set(account.providerId, {
          providerLabel: account.providerLabel,
          accounts: [account],
        });
      }
    }
  }

  // A route naming one exact model is itself a declaration that the model
  // exists, so it earns a card even before an account backs it — that is how a
  // freshly added model shows up instead of vanishing until a credential lists
  // it. Wildcard patterns are skipped: they name a family, not a model.
  for (const chain of chains) {
    if (chain.pattern.length === 0 || chain.pattern.includes("*")) {
      continue;
    }
    if (!modelIndex.has(chain.pattern)) {
      modelIndex.set(chain.pattern, new Map<string, ProviderBucket>());
    }
  }

  return [...modelIndex.entries()]
    .map(([model, byProvider]) => {
      const order = resolveChainOrder(chains, model);
      const pinnedChain = chains.find((chain) => chain.pattern === model) ?? null;
      const linkContext: LinkContext = { model, metricsByAccountId, resolver };
      const placed = new Set<string>();
      const chainLinks: ModelPoolProviderLink[] = [];
      // 1. Providers the routes order, in the order the gateway would try them.
      for (const providerId of order.providerIds) {
        const bucket = byProvider.get(providerId);
        if (!bucket) {
          continue;
        }
        placed.add(providerId);
        chainLinks.push(buildLink(providerId, bucket, linkContext));
      }
      // 2. Providers no route mentions. The gateway would try these in document
      //    order, so the tail follows it rather than the alphabet.
      const unrouted = [...byProvider.entries()]
        .filter(([providerId]) => !placed.has(providerId))
        .sort(compareUnrouted);
      for (const [providerId, bucket] of unrouted) {
        placed.add(providerId);
        chainLinks.push(buildLink(providerId, bucket, linkContext));
      }
      // 3. Entries this model's own route pins that no account backs. Only the
      //    exact-pattern route contributes here: a wildcard route's unrelated
      //    providers are not this card's to keep.
      for (const providerId of pinnedChain?.providerIds ?? []) {
        if (placed.has(providerId)) {
          continue;
        }
        placed.add(providerId);
        chainLinks.push(buildLink(providerId, undefined, linkContext));
      }

      const accounts = [...byProvider.values()].flatMap((bucket) => bucket.accounts);
      // The card totals the chain it renders. Each link is already one provider
      // account's traffic for this model, so the links add up without the
      // double counting that summing account cards would introduce — and a
      // detached link still contributes, because a route pinning a provider for
      // this model means upstream may have served it there.
      const cardMetrics = resolver
        ? chainLinks
            .map((link) => link.metrics)
            .filter((metrics): metrics is ProviderAggregateMetrics => metrics !== null)
            .map(
              (metrics): ProviderMetricsAccountLike => ({
                concurrencyUsed: metrics.concurrency?.used ?? null,
                concurrencyTotal: metrics.concurrency?.total ?? null,
                requestCount: metrics.requests,
                upstreamCost: metrics.upstreamCost,
                userCost: metrics.platformRevenue,
                successWindows: metrics.successWindows,
              }),
            )
        : metricsByAccountId
          ? accounts
              .map((account) => metricsByAccountId.get(account.id))
              .filter((metrics): metrics is ProviderMetricsAccountLike => Boolean(metrics))
          : [];
      return {
        rowId: model,
        model,
        chain: chainLinks,
        accountCount: accounts.length,
        enabledAccountCount: accounts.filter((account) => account.enabled).length,
        pinned: pinnedChain != null,
        enabled: pinnedChain?.enabled ?? true,
        routedPatterns: order.patterns,
        metrics: cardMetrics.length > 0 ? aggregateProviderMetrics(cardMetrics) : null,
      };
    })
    .sort((left, right) => left.model.localeCompare(right.model));
}

/**
 * The chain with `providerId` moved one slot toward the front or the back.
 * Returns the input array when the move would fall off either end, so callers
 * can skip a no-op write.
 */
export function moveChainProvider(
  providerIds: readonly string[],
  providerId: string,
  direction: "up" | "down",
): string[] {
  const index = providerIds.indexOf(providerId);
  if (index < 0) {
    return [...providerIds];
  }
  const target = direction === "up" ? index - 1 : index + 1;
  if (target < 0 || target >= providerIds.length) {
    return [...providerIds];
  }
  const next = [...providerIds];
  next[index] = next[target];
  next[target] = providerId;
  return next;
}
