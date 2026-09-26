export type ProviderPoolState = "available" | "rate-limited" | "invalid" | "unknown";

export type ProviderPoolAccountLike = {
  statusLabel: string;
  dispatchEnabled: boolean;
  verificationStatus?: string;
};

export type ProviderPoolSegment = {
  state: Exclude<ProviderPoolState, "unknown"> | "remaining";
  count: number;
  ratio: number;
  percent: number;
};

export type ProviderPoolSegments = {
  target: number;
  available: number;
  rateLimited: number;
  invalid: number;
  remaining: number;
  unknown: number;
  segments: ProviderPoolSegment[];
};

export type ProviderConcurrencyLike = {
  concurrencyUsed?: number | null;
  concurrencyTotal?: number | null;
};

export type ProviderConcurrency = {
  used: number;
  /**
   * Null when the gateway reports live concurrency but no ceiling, which happens
   * while the adaptive limiter is still probing an account. Render `—` for the
   * denominator instead of inventing one.
   */
  total: number | null;
} | null;

export type ProviderCostLike = {
  upstreamCost?: number | null;
  userCost?: number | null;
  usageWindowBadges?: string[];
};

export type ProviderCosts = {
  upstream: number | null;
  user: number | null;
};

export type ProviderRequestLike = {
  /** Real request count from the gateway; preferred over the badge strings. */
  requestCount?: number | null;
  usageWindowBadges?: readonly string[];
};

export type ProviderSuccessWindow = {
  label: string;
  success: number;
  requests: number;
};

export type ProviderSuccessWindowLike = {
  successWindows?: ProviderSuccessWindow[] | null;
};

export type AggregatedSuccessWindow = ProviderSuccessWindow & {
  rate: number | null;
};

export type ProviderAvailabilityCellState = "success" | "mixed" | "failure" | "empty";

export type ProviderAvailabilityCell = {
  state: ProviderAvailabilityCellState;
  windowIndex: number;
  position: number;
  windowLabel: string;
  success: number;
  requests: number;
  rate: number | null;
};

export type ProviderQuotaWindowLike = {
  key: string;
  label: string;
  usedPercent?: number | null;
  remainingRatio?: number | null;
  limitWindowSeconds?: number | null;
  resetAt?: string | null;
};

export type ProviderQuotaLike = {
  quota?: {
    windows: ProviderQuotaWindowLike[];
  } | null;
  quotaRemainingUsd?: number | null;
};

export type AggregatedProviderQuotaWindow = {
  key: string;
  label: string;
  remainingRatio: number | null;
  accountCount: number;
  resetAt: string | null;
};

/** Everything the shared usage strip needs from one account. */
export type ProviderMetricsAccountLike = ProviderConcurrencyLike &
  ProviderCostLike &
  ProviderRequestLike &
  ProviderSuccessWindowLike;

export type ProviderMetricsResolveOptions = {
  /** Explicit rate, for a card that knows the one it bills at (a group card). */
  billingMultiplier?: number;
  /**
   * Credentials whose enabled groups set the ceiling rate, for a card that spans
   * several groups (a model card). Ignored when `billingMultiplier` is given.
   */
  credentialRefs?: readonly string[];
};

/**
 * Where a card gets the numbers the per-account map cannot answer.
 *
 * Requests, money and live concurrency are recorded per provider account and per
 * model, never per provider credential, so a card that covers several
 * credentials of one provider has to ask for the provider-account rollup instead
 * of summing its account cards — summing them would either multiply the totals
 * by the pool size or, once per-account attribution is suppressed to avoid that,
 * collapse the whole strip to `—`.
 */
export type ProviderMetricsResolver = {
  /** Rolled up over whole provider accounts. */
  providerAccounts: (
    providerAccountIds: readonly string[],
    options?: ProviderMetricsResolveOptions,
  ) => ProviderMetricsAccountLike | null;
  /**
   * One provider account's traffic for one logical model. Implementations
   * translate the logical name through that provider's `model_map` before
   * reading telemetry, because upstream is keyed by the model it was called
   * with.
   */
  providerAccountModel: (
    providerAccountId: string,
    model: string,
    options?: ProviderMetricsResolveOptions,
  ) => ProviderMetricsAccountLike | null;
};

export type ProviderAggregateMetrics = {
  concurrency: ProviderConcurrency;
  upstreamCost: number | null;
  platformRevenue: number | null;
  requests: number | null;
  successWindows: AggregatedSuccessWindow[];
  successSuccessCount: number;
  successRequestCount: number;
  successRate: number | null;
};

function successWindowSortKey(label: string): { kind: number; value: number; label: string } {
  const timeOnly = label.trim().match(/^(\d{1,2}):(\d{2})$/);
  if (timeOnly) {
    const hours = Number(timeOnly[1]);
    const minutes = Number(timeOnly[2]);
    if (hours >= 0 && hours < 24 && minutes >= 0 && minutes < 60) {
      return { kind: 0, value: hours * 60 + minutes, label };
    }
  }

  const timestamp = Date.parse(label);
  if (Number.isFinite(timestamp)) {
    return { kind: 0, value: timestamp, label };
  }

  return { kind: 1, value: 0, label: label.toLocaleLowerCase() };
}

function finiteNonNegative(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

/** Classify only states that are explicit in the account status contract. */
export function classifyAccountPoolState(account: ProviderPoolAccountLike): ProviderPoolState {
  const status = account.statusLabel.trim().toLowerCase();
  if (!status) {
    return "unknown";
  }
  if (
    /(限流|待恢复|恢复中|冷却|rate.?limit|throttl|cooldown|recover|retry.?after|waiting)/i.test(
      status,
    )
  ) {
    return "rate-limited";
  }
  if (
    /(失效|不可用|撤销|禁用|失败|错误|invalid|unavailable|revoked|disabled|failed|blocked|error)/i.test(
      status,
    )
  ) {
    return "invalid";
  }
  if (
    account.dispatchEnabled &&
    /(正常|可用|健康|启用|available|healthy|ready|enabled|active|ok)/i.test(status)
  ) {
    return "available";
  }
  return "unknown";
}

/** Build a four-colour pool bar without allowing invalid input to overflow. */
export function buildProviderPoolSegments(
  accounts: readonly ProviderPoolAccountLike[],
  targetValue: number,
): ProviderPoolSegments {
  const target = Number.isFinite(targetValue) && targetValue > 0 ? Math.floor(targetValue) : 0;
  let available = 0;
  let rateLimited = 0;
  let invalid = 0;
  let unknown = 0;

  for (const account of accounts) {
    switch (classifyAccountPoolState(account)) {
      case "available":
        available += 1;
        break;
      case "rate-limited":
        rateLimited += 1;
        break;
      case "invalid":
        invalid += 1;
        break;
      default:
        unknown += 1;
        break;
    }
  }

  const known = available + rateLimited + invalid;
  const remaining = Math.max(target - known, 0);
  const denominator = known + remaining;
  const ratio = (count: number) => (denominator > 0 ? count / denominator : 0);
  const segments: ProviderPoolSegment[] = [
    { state: "available", count: available, ratio: ratio(available), percent: ratio(available) * 100 },
    {
      state: "rate-limited",
      count: rateLimited,
      ratio: ratio(rateLimited),
      percent: ratio(rateLimited) * 100,
    },
    { state: "invalid", count: invalid, ratio: ratio(invalid), percent: ratio(invalid) * 100 },
    { state: "remaining", count: remaining, ratio: ratio(remaining), percent: ratio(remaining) * 100 },
  ];

  // When known states exceed the target, the grey segment is zero and the
  // three observed states are normalized against the observed total.
  return { target, available, rateLimited, invalid, remaining, unknown, segments };
}

export function aggregateProviderConcurrency(
  accounts: readonly (ProviderConcurrencyLike & { usageWindowBadges?: string[] })[],
): ProviderConcurrency {
  let used = 0;
  let total: number | null = null;
  let seen = false;
  for (const account of accounts) {
    const accountUsed = finiteNonNegative(account.concurrencyUsed);
    if (accountUsed === null) {
      continue;
    }
    seen = true;
    used += accountUsed;
    const accountTotal = finiteNonNegative(account.concurrencyTotal);
    if (accountTotal !== null) {
      total = (total ?? 0) + Math.max(accountTotal, accountUsed);
    }
  }
  return seen ? { used, total } : null;
}

function parseBadgeCost(badges: readonly string[] | undefined, prefix: "A" | "U"): number | null {
  if (!badges) {
    return null;
  }
  let value = 0;
  let seen = false;
  for (const badge of badges) {
    const match = badge.trim().match(new RegExp(`^${prefix}\\s+\\$([0-9]+(?:\\.[0-9]+)?)$`, "i"));
    if (!match) {
      continue;
    }
    const parsed = Number(match[1]);
    if (Number.isFinite(parsed) && parsed >= 0) {
      value += parsed;
      seen = true;
    }
  }
  return seen ? value : null;
}

export function aggregateProviderCosts(accounts: readonly ProviderCostLike[]): ProviderCosts {
  let upstream = 0;
  let user = 0;
  let upstreamSeen = false;
  let userSeen = false;
  for (const account of accounts) {
    const explicitUpstream = finiteNonNegative(account.upstreamCost);
    const explicitUser = finiteNonNegative(account.userCost);
    const accountUpstream = explicitUpstream ?? parseBadgeCost(account.usageWindowBadges, "A");
    const accountUser = explicitUser ?? parseBadgeCost(account.usageWindowBadges, "U");
    if (accountUpstream !== null) {
      upstream += accountUpstream;
      upstreamSeen = true;
    }
    if (accountUser !== null) {
      user += accountUser;
      userSeen = true;
    }
  }
  return { upstream: upstreamSeen ? upstream : null, user: userSeen ? user : null };
}

export function aggregateProviderRequests(
  accounts: readonly ProviderRequestLike[],
): number | null {
  let requests = 0;
  let seen = false;
  for (const account of accounts) {
    const explicit = finiteNonNegative(account.requestCount);
    if (explicit !== null) {
      requests += explicit;
      seen = true;
      continue;
    }
    // Legacy fallback: some fixtures only carry the rendered `"18 req"` badge.
    for (const badge of account.usageWindowBadges ?? []) {
      const match = badge.trim().match(/^([0-9]+(?:,[0-9]{3})*)\s*req$/i);
      if (!match) {
        continue;
      }
      const parsed = Number(match[1].replaceAll(",", ""));
      if (Number.isSafeInteger(parsed) && parsed >= 0) {
        requests += parsed;
        seen = true;
      }
    }
  }
  return seen ? requests : null;
}

export function aggregateSuccessWindows(
  accounts: readonly ProviderSuccessWindowLike[],
): AggregatedSuccessWindow[] {
  const aggregate = new Map<string, { success: number; requests: number }>();
  for (const account of accounts) {
    for (const window of account.successWindows ?? []) {
      if (!window.label || !Number.isFinite(window.success) || !Number.isFinite(window.requests)) {
        continue;
      }
      const success = Math.max(0, Math.floor(window.success));
      const requests = Math.max(success, Math.floor(window.requests));
      const current = aggregate.get(window.label) ?? { success: 0, requests: 0 };
      current.success += success;
      current.requests += requests;
      aggregate.set(window.label, current);
    }
  }
  return [...aggregate.entries()]
    .map(([label, values]) => ({
      label,
      ...values,
      rate: values.requests > 0 ? values.success / values.requests : null,
      sortKey: successWindowSortKey(label),
    }))
    .sort((left, right) =>
      left.sortKey.kind - right.sortKey.kind ||
      left.sortKey.value - right.sortKey.value ||
      left.sortKey.label.localeCompare(right.sortKey.label),
    )
    .map(({ sortKey: _sortKey, ...window }) => window);
}

export function buildProviderAvailabilityCells(
  windows: readonly AggregatedSuccessWindow[],
  cellsPerWindow = 12,
): ProviderAvailabilityCell[] {
  const cellCount = Number.isFinite(cellsPerWindow)
    ? Math.min(100, Math.max(1, Math.floor(cellsPerWindow)))
    : 12;

  return windows.flatMap((window, windowIndex) => {
    const rate = window.rate === null ? null : Math.min(1, Math.max(0, window.rate));
    const successfulShare = rate === null ? 0 : rate * cellCount;
    const successfulCells = Math.floor(successfulShare);
    const hasMixedCell =
      rate !== null &&
      successfulCells < cellCount &&
      successfulShare - successfulCells > Number.EPSILON;

    return Array.from({ length: cellCount }, (_, position): ProviderAvailabilityCell => {
      let state: ProviderAvailabilityCellState;
      if (rate === null || window.requests === 0) {
        state = "empty";
      } else if (position < successfulCells) {
        state = "success";
      } else if (hasMixedCell && position === successfulCells) {
        state = "mixed";
      } else {
        state = "failure";
      }
      return {
        state,
        windowIndex,
        position,
        windowLabel: window.label,
        success: window.success,
        requests: window.requests,
        rate: window.rate,
      };
    });
  });
}

export function aggregateProviderQuotaWindows(
  accounts: readonly ProviderQuotaLike[],
): AggregatedProviderQuotaWindow[] {
  const aggregate = new Map<
    string,
    {
      key: string;
      label: string;
      remainingTotal: number;
      remainingCount: number;
      accountCount: number;
      resetAt: string | null;
      sortSeconds: number;
    }
  >();

  for (const account of accounts) {
    for (const window of account.quota?.windows ?? []) {
      const limitWindowSeconds = finiteNonNegative(window.limitWindowSeconds);
      const aggregateKey = `${window.key}:${limitWindowSeconds ?? "unknown"}`;
      const current = aggregate.get(aggregateKey) ?? {
        key: window.key,
        label: window.label,
        remainingTotal: 0,
        remainingCount: 0,
        accountCount: 0,
        resetAt: null,
        sortSeconds: limitWindowSeconds ?? Number.POSITIVE_INFINITY,
      };
      const explicitRemaining =
        typeof window.remainingRatio === "number" && Number.isFinite(window.remainingRatio)
          ? window.remainingRatio
          : typeof window.usedPercent === "number" && Number.isFinite(window.usedPercent)
            ? 1 - window.usedPercent / 100
            : null;
      if (explicitRemaining !== null) {
        current.remainingTotal += Math.min(1, Math.max(0, explicitRemaining));
        current.remainingCount += 1;
      }
      current.accountCount += 1;
      if (window.resetAt) {
        const nextReset = Date.parse(window.resetAt);
        const currentReset = current.resetAt ? Date.parse(current.resetAt) : Number.POSITIVE_INFINITY;
        if (Number.isFinite(nextReset) && nextReset < currentReset) {
          current.resetAt = window.resetAt;
        }
      }
      aggregate.set(aggregateKey, current);
    }
  }

  return [...aggregate.values()]
    .sort((left, right) => left.sortSeconds - right.sortSeconds || left.label.localeCompare(right.label))
    .map((window) => ({
      key: window.key,
      label: window.label,
      remainingRatio:
        window.remainingCount > 0 ? window.remainingTotal / window.remainingCount : null,
      accountCount: window.accountCount,
      resetAt: window.resetAt,
    }));
}

export function aggregateQuotaRemainingUsd(accounts: readonly ProviderQuotaLike[]): number | null {
  let total = 0;
  let seen = false;
  for (const account of accounts) {
    const value = finiteNonNegative(account.quotaRemainingUsd);
    if (value === null) {
      continue;
    }
    total += value;
    seen = true;
  }
  return seen ? total : null;
}

/**
 * One call for the whole usage strip (concurrency / upstream cost / platform
 * revenue / requests / success rate). Delegates to the single-metric
 * aggregators above so provider cards, account cards and entitlement group
 * cards cannot drift apart in how they roll numbers up.
 */
export function aggregateProviderMetrics(
  accounts: readonly ProviderMetricsAccountLike[],
  windowLimit = 4,
): ProviderAggregateMetrics {
  const costs = aggregateProviderCosts(accounts);
  const successWindows = aggregateSuccessWindows(accounts).slice(-Math.max(1, windowLimit));
  const totals = successWindows.reduce(
    (accumulated, window) => ({
      success: accumulated.success + window.success,
      requests: accumulated.requests + window.requests,
    }),
    { success: 0, requests: 0 },
  );

  return {
    concurrency: aggregateProviderConcurrency(accounts),
    upstreamCost: costs.upstream,
    platformRevenue: costs.user,
    requests: aggregateProviderRequests(accounts),
    successWindows,
    successSuccessCount: totals.success,
    successRequestCount: totals.requests,
    successRate: totals.requests > 0 ? totals.success / totals.requests : null,
  };
}

export function formatAggregateMoney(value: number): string {
  return value.toFixed(2);
}

export function formatAggregateRate(rate: number | null): string {
  if (rate === null) {
    return "—";
  }
  return `${(rate * 100).toFixed(1).replace(/\.0$/, "")}%`;
}
