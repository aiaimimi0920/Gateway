import type { ProviderSuccessWindow } from "./providerCardMetrics";
import { laterTimestamp, type ConsoleTelemetrySnapshot } from "./telemetry";

export type ConsoleTelemetryRollup = {
  concurrencyUsed: number | null;
  concurrencyTotal: number | null;
  requestCount: number | null;
  upstreamCostUsd: number | null;
  lastRequestAt: string | null;
  successWindows: ProviderSuccessWindow[];
};

const EMPTY_ROLLUP: ConsoleTelemetryRollup = {
  concurrencyUsed: null,
  concurrencyTotal: null,
  requestCount: null,
  upstreamCostUsd: null,
  lastRequestAt: null,
  successWindows: [],
};

/**
 * Roll several provider accounts up into one card's worth of numbers. Provider
 * cards and entitlement group cards must read this instead of summing their
 * account cards: provider-account telemetry has no per-credential breakdown, so
 * copying it onto every credential would multiply the totals by the pool size.
 */
export function rollupProviderAccountTelemetry(
  snapshot: ConsoleTelemetrySnapshot,
  providerAccountIds: Iterable<string>,
): ConsoleTelemetryRollup {
  const seen = new Set<string>();
  let concurrencyUsed: number | null = null;
  let concurrencyTotal: number | null = null;
  let requestCount: number | null = null;
  let upstreamCostUsd: number | null = null;
  let lastRequestAt: string | null = null;
  const windowTotals = new Map<string, { success: number; requests: number }>();

  for (const providerAccountId of providerAccountIds) {
    if (seen.has(providerAccountId)) {
      continue;
    }
    seen.add(providerAccountId);
    const entry = snapshot.providerAccounts.get(providerAccountId);
    if (!entry) {
      continue;
    }
    if (entry.activeConcurrency !== null) {
      concurrencyUsed = (concurrencyUsed ?? 0) + entry.activeConcurrency;
    }
    if (entry.concurrencyLimit !== null) {
      concurrencyTotal = (concurrencyTotal ?? 0) + entry.concurrencyLimit;
    }
    if (entry.requestCount !== null) {
      requestCount = (requestCount ?? 0) + entry.requestCount;
    }
    if (entry.upstreamCostUsd !== null) {
      upstreamCostUsd = (upstreamCostUsd ?? 0) + entry.upstreamCostUsd;
    }
    lastRequestAt = laterTimestamp(lastRequestAt, entry.lastRequestAt);
    for (const window of entry.successWindows) {
      const current = windowTotals.get(window.label) ?? { success: 0, requests: 0 };
      current.success += window.success;
      current.requests += window.requests;
      windowTotals.set(window.label, current);
    }
  }

  if (seen.size === 0) {
    return EMPTY_ROLLUP;
  }

  return {
    concurrencyUsed,
    concurrencyTotal,
    requestCount,
    upstreamCostUsd,
    lastRequestAt,
    successWindows: [...windowTotals.entries()]
      .map(([label, totals]) => ({ label, ...totals }))
      .sort((left, right) => left.label.localeCompare(right.label)),
  };
}

/** One (provider account, upstream model) pair a model card or scope row covers. */
export type ConsoleTelemetryModelTarget = {
  providerAccountId: string;
  /** Upstream model name, already translated through the provider's model map. */
  model: string;
};

/**
 * Roll a model card's worth of numbers up over the provider accounts that serve
 * it. Money, tokens and traffic come from the per-model rows; live concurrency
 * has no model dimension upstream, so it is attributed only when the account
 * serves this one model and stays null otherwise instead of double counting a
 * shared limiter across every model on the account.
 */
export function rollupProviderAccountModelTelemetry(
  snapshot: ConsoleTelemetrySnapshot,
  targets: Iterable<ConsoleTelemetryModelTarget>,
): ConsoleTelemetryRollup {
  const seen = new Set<string>();
  let matched = false;
  let concurrencyUsed: number | null = null;
  let concurrencyTotal: number | null = null;
  let requestCount: number | null = null;
  let upstreamCostUsd: number | null = null;
  let lastRequestAt: string | null = null;
  const windowTotals = new Map<string, { success: number; requests: number }>();

  for (const target of targets) {
    // Provider account ids are uuids, so a space cannot collide with one even
    // though a model name may legitimately contain punctuation.
    const key = `${target.providerAccountId} ${target.model}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    const account = snapshot.providerAccounts.get(target.providerAccountId);
    const entry = account?.modelStats.get(target.model);
    if (!account || !entry) {
      continue;
    }
    matched = true;
    if (account.modelStats.size <= 1) {
      if (account.activeConcurrency !== null) {
        concurrencyUsed = (concurrencyUsed ?? 0) + account.activeConcurrency;
      }
      if (account.concurrencyLimit !== null) {
        concurrencyTotal = (concurrencyTotal ?? 0) + account.concurrencyLimit;
      }
    }
    if (entry.requestCount !== null) {
      requestCount = (requestCount ?? 0) + entry.requestCount;
    }
    if (entry.upstreamCostUsd !== null) {
      upstreamCostUsd = (upstreamCostUsd ?? 0) + entry.upstreamCostUsd;
    }
    lastRequestAt = laterTimestamp(lastRequestAt, entry.lastRequestAt);
    for (const window of entry.successWindows) {
      const current = windowTotals.get(window.label) ?? { success: 0, requests: 0 };
      current.success += window.success;
      current.requests += window.requests;
      windowTotals.set(window.label, current);
    }
  }

  if (!matched) {
    return EMPTY_ROLLUP;
  }

  return {
    concurrencyUsed,
    concurrencyTotal,
    requestCount,
    upstreamCostUsd,
    lastRequestAt,
    successWindows: [...windowTotals.entries()]
      .map(([label, totals]) => ({ label, ...totals }))
      .sort((left, right) => left.label.localeCompare(right.label)),
  };
}

/**
 * True when a rollup carries at least one observed number. A rollup over ids the
 * snapshot does not know reads as all-null rather than as zeros, and a card must
 * render `—` for that instead of claiming no traffic.
 */
export function rollupHasData(rollup: ConsoleTelemetryRollup): boolean {
  return (
    rollup.concurrencyUsed !== null ||
    rollup.concurrencyTotal !== null ||
    rollup.requestCount !== null ||
    rollup.upstreamCostUsd !== null ||
    rollup.successWindows.length > 0
  );
}
