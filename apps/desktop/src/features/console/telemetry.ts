import type {
  ConsoleCostOverview,
  ConsoleProviderCredentialInventoryItem,
  ConsoleProviderCredentialModelState,
  ConsoleRequestAuditSummary,
  ConsoleRuntimePressure,
} from "../../api/contracts";
import type { ProviderSuccessWindow } from "./providerCardMetrics";

const MICROS_PER_USD = 1_000_000;

/** Health of one route-document credential, rolled up over the models it served. */
export type ConsoleTelemetryCredential = {
  credentialRef: string;
  providerAccountIds: string[];
  /** Worst status across the credential's models: active < degraded < cooling < blocked. */
  status: string | null;
  models: string[];
  failureCount: number;
  cooldownUntil: string | null;
  lastError: string | null;
  lastUpstreamStatus: number | null;
  lastSuccessAt: string | null;
  lastFailureAt: string | null;
  quotaRemainingUsd: number | null;
};

/**
 * One provider account's numbers for a single upstream model. Model cards and
 * entitlement scope rows are per model, and a provider account that serves
 * several models cannot answer them from its totals.
 */
export type ConsoleTelemetryProviderAccountModel = {
  model: string;
  requestCount: number | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  upstreamCostUsd: number | null;
  lastRequestAt: string | null;
  completedCount: number | null;
  failedCount: number | null;
  successWindows: ProviderSuccessWindow[];
};

/** Live and billed state of one provider account (a route-document provider). */
export type ConsoleTelemetryProviderAccount = {
  providerAccountId: string;
  activeConcurrency: number | null;
  concurrencyLimit: number | null;
  concurrencyAvailable: number | null;
  breakerOpen: boolean;
  runtimeStatus: string | null;
  requestCount: number | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  upstreamCostUsd: number | null;
  lastRequestAt: string | null;
  /** Models with recorded traffic, not the provider's declared catalogue. */
  models: string[];
  /** Keyed by upstream model name, for the models that carry traffic. */
  modelStats: ReadonlyMap<string, ConsoleTelemetryProviderAccountModel>;
  completedCount: number | null;
  failedCount: number | null;
  successWindows: ProviderSuccessWindow[];
};

export type ConsoleTelemetrySnapshot = {
  credentials: ReadonlyMap<string, ConsoleTelemetryCredential>;
  providerAccounts: ReadonlyMap<string, ConsoleTelemetryProviderAccount>;
  /** Credential refs observed for a provider account, from health states. */
  credentialRefsByProviderAccountId: ReadonlyMap<string, string[]>;
  hasPressure: boolean;
  hasCosts: boolean;
  hasStates: boolean;
  hasAudits: boolean;
};

export const EMPTY_CONSOLE_TELEMETRY: ConsoleTelemetrySnapshot = {
  credentials: new Map(),
  providerAccounts: new Map(),
  credentialRefsByProviderAccountId: new Map(),
  hasPressure: false,
  hasCosts: false,
  hasStates: false,
  hasAudits: false,
};

export type ConsoleTelemetryInput = {
  pressure: ConsoleRuntimePressure | null;
  costOverview: ConsoleCostOverview | null;
  requestAuditSummary: ConsoleRequestAuditSummary | null;
  credentialModelStates: ConsoleProviderCredentialModelState[] | null;
  credentialInventory: ConsoleProviderCredentialInventoryItem[] | null;
  /** How many hourly windows the availability strip should keep. */
  successWindowLimit?: number;
};

const STATUS_SEVERITY: Record<string, number> = {
  active: 0,
  degraded: 1,
  cooling: 2,
  blocked: 3,
};

function statusSeverity(status: string): number {
  return STATUS_SEVERITY[status.trim().toLowerCase()] ?? 1;
}

export function laterTimestamp(current: string | null, candidate: string | null): string | null {
  if (!candidate) {
    return current;
  }
  if (!current) {
    return candidate;
  }
  return candidate > current ? candidate : current;
}

function nonNegative(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Providers only report a spendable balance when the upstream exposes one, and
 * the shape differs per vendor, so accept the handful of documented keys and
 * refuse to guess at anything else.
 */
export function readQuotaRemainingUsd(rawData: unknown, depth = 0): number | null {
  if (!isRecord(rawData) || depth > 2) {
    return null;
  }
  for (const key of ["remainingUsd", "remaining_usd", "balanceUsd", "balance_usd"]) {
    const value = nonNegative(rawData[key] as number | null | undefined);
    if (value !== null) {
      return value;
    }
  }
  const currency = typeof rawData.currency === "string" ? rawData.currency.toUpperCase() : null;
  if (currency === "USD") {
    for (const key of ["remaining", "available", "balance"]) {
      const value = nonNegative(rawData[key] as number | null | undefined);
      if (value !== null) {
        return value;
      }
    }
  }
  for (const key of ["data", "quota", "credits", "balance"]) {
    const nested = readQuotaRemainingUsd(rawData[key], depth + 1);
    if (nested !== null) {
      return nested;
    }
  }
  return null;
}

type CredentialAccumulator = {
  providerAccountIds: Set<string>;
  status: string | null;
  models: Set<string>;
  failureCount: number;
  cooldownUntil: string | null;
  lastError: string | null;
  lastUpstreamStatus: number | null;
  lastSuccessAt: string | null;
  lastFailureAt: string | null;
};

/** The published entry with its per-model map still writable. */
type ProviderAccountAccumulator = Omit<ConsoleTelemetryProviderAccount, "modelStats"> & {
  modelStats: Map<string, ConsoleTelemetryProviderAccountModel>;
};

/**
 * Fold every live endpoint the console polls into one lookup keyed the way the
 * cards are: provider accounts by route-document provider id, credentials by
 * route-document credential id.
 */
export function buildConsoleTelemetrySnapshot(
  input: ConsoleTelemetryInput,
): ConsoleTelemetrySnapshot {
  const windowLimit = Math.max(1, Math.floor(input.successWindowLimit ?? 4));
  const providerAccounts = new Map<string, ProviderAccountAccumulator>();

  const ensureProviderAccount = (providerAccountId: string): ProviderAccountAccumulator => {
    const existing = providerAccounts.get(providerAccountId);
    if (existing) {
      return existing;
    }
    const created: ProviderAccountAccumulator = {
      providerAccountId,
      activeConcurrency: null,
      concurrencyLimit: null,
      concurrencyAvailable: null,
      breakerOpen: false,
      runtimeStatus: null,
      requestCount: null,
      promptTokens: null,
      completionTokens: null,
      totalTokens: null,
      upstreamCostUsd: null,
      lastRequestAt: null,
      models: [],
      modelStats: new Map(),
      completedCount: null,
      failedCount: null,
      successWindows: [],
    };
    providerAccounts.set(providerAccountId, created);
    return created;
  };

  const ensureModelStats = (
    entry: ProviderAccountAccumulator,
    model: string,
  ): ConsoleTelemetryProviderAccountModel => {
    const existing = entry.modelStats.get(model);
    if (existing) {
      return existing;
    }
    const created: ConsoleTelemetryProviderAccountModel = {
      model,
      requestCount: null,
      promptTokens: null,
      completionTokens: null,
      totalTokens: null,
      upstreamCostUsd: null,
      lastRequestAt: null,
      completedCount: null,
      failedCount: null,
      successWindows: [],
    };
    entry.modelStats.set(model, created);
    return created;
  };

  for (const provider of input.pressure?.providers ?? []) {
    const entry = ensureProviderAccount(provider.providerAccountId);
    entry.activeConcurrency = nonNegative(provider.activeConcurrency) ?? 0;
    entry.concurrencyLimit = nonNegative(provider.concurrencyLimit);
    entry.concurrencyAvailable = nonNegative(provider.concurrencyAvailable);
    entry.breakerOpen = provider.breakerOpen;
    entry.runtimeStatus = provider.status;
  }

  for (const bucket of input.costOverview?.providerBuckets ?? []) {
    const entry = ensureProviderAccount(bucket.providerAccountId);
    entry.requestCount = nonNegative(bucket.requestCount) ?? 0;
    entry.promptTokens = nonNegative(bucket.promptTokens) ?? 0;
    entry.completionTokens = nonNegative(bucket.completionTokens) ?? 0;
    entry.totalTokens = nonNegative(bucket.totalTokens) ?? 0;
    // Null means "no price configured for these models", which must stay null
    // instead of collapsing to $0.00 and reading as a free provider.
    const costMicros = nonNegative(bucket.estimatedMarketCostMicros);
    entry.upstreamCostUsd = costMicros === null ? null : costMicros / MICROS_PER_USD;
    entry.lastRequestAt = laterTimestamp(entry.lastRequestAt, bucket.lastRequestAt);
    entry.models = [
      ...new Set(
        bucket.models
          .map((row) => row.model.trim())
          .filter((model) => model.length > 0),
      ),
    ].sort((left, right) => left.localeCompare(right));
    for (const row of bucket.models) {
      const model = row.model.trim();
      if (!model) {
        continue;
      }
      const modelEntry = ensureModelStats(entry, model);
      modelEntry.requestCount = nonNegative(row.requestCount) ?? 0;
      modelEntry.promptTokens = nonNegative(row.promptTokens) ?? 0;
      modelEntry.completionTokens = nonNegative(row.completionTokens) ?? 0;
      modelEntry.totalTokens = nonNegative(row.totalTokens) ?? 0;
      const modelCostMicros = nonNegative(row.estimatedMarketCostMicros);
      modelEntry.upstreamCostUsd =
        modelCostMicros === null ? null : modelCostMicros / MICROS_PER_USD;
      modelEntry.lastRequestAt = laterTimestamp(modelEntry.lastRequestAt, row.lastRequestAt);
    }
  }

  for (const stats of input.requestAuditSummary?.providerAccounts ?? []) {
    const entry = ensureProviderAccount(stats.providerAccountId);
    entry.completedCount = nonNegative(stats.completedCount) ?? 0;
    entry.failedCount = nonNegative(stats.failedCount) ?? 0;
    entry.lastRequestAt = laterTimestamp(entry.lastRequestAt, stats.lastRequestAt);
    entry.successWindows = stats.windows
      .map((window) => ({
        label: window.label,
        success: Math.max(0, Math.floor(window.successCount)),
        requests: Math.max(0, Math.floor(window.totalRequests)),
      }))
      .slice(-windowLimit);
    for (const modelStats of stats.models) {
      const model = modelStats.model.trim();
      if (!model) {
        continue;
      }
      const modelEntry = ensureModelStats(entry, model);
      modelEntry.completedCount = nonNegative(modelStats.completedCount) ?? 0;
      modelEntry.failedCount = nonNegative(modelStats.failedCount) ?? 0;
      modelEntry.lastRequestAt = laterTimestamp(modelEntry.lastRequestAt, modelStats.lastRequestAt);
      modelEntry.successWindows = modelStats.windows
        .map((window) => ({
          label: window.label,
          success: Math.max(0, Math.floor(window.successCount)),
          requests: Math.max(0, Math.floor(window.totalRequests)),
        }))
        .slice(-windowLimit);
    }
    // The audit summary can see models the cost overview has no price row for,
    // so re-derive the model list from both sources instead of only the costs.
    if (entry.modelStats.size > entry.models.length) {
      entry.models = [...entry.modelStats.keys()].sort((left, right) => left.localeCompare(right));
    }
  }

  const credentialAccumulators = new Map<string, CredentialAccumulator>();
  for (const state of input.credentialModelStates ?? []) {
    const credentialRef = (state.providerCredentialRef ?? state.providerCredentialId ?? "").trim();
    if (!credentialRef) {
      continue;
    }
    const accumulator = credentialAccumulators.get(credentialRef) ?? {
      providerAccountIds: new Set<string>(),
      status: null,
      models: new Set<string>(),
      failureCount: 0,
      cooldownUntil: null,
      lastError: null,
      lastUpstreamStatus: null,
      lastSuccessAt: null,
      lastFailureAt: null,
    };
    if (state.providerAccountId.trim().length > 0) {
      accumulator.providerAccountIds.add(state.providerAccountId.trim());
    }
    if (state.model.trim().length > 0) {
      accumulator.models.add(state.model.trim());
    }
    if (
      accumulator.status === null ||
      statusSeverity(state.status) > statusSeverity(accumulator.status)
    ) {
      accumulator.status = state.status;
    }
    accumulator.failureCount += nonNegative(state.failureCount) ?? 0;
    accumulator.cooldownUntil = laterTimestamp(accumulator.cooldownUntil, state.cooldownUntil);
    accumulator.lastSuccessAt = laterTimestamp(accumulator.lastSuccessAt, state.lastSuccessAt);
    // Keep the message that belongs to the newest failure, so the card never
    // shows an old error next to a fresher timestamp.
    if (
      state.lastFailureAt &&
      (accumulator.lastFailureAt === null || state.lastFailureAt > accumulator.lastFailureAt)
    ) {
      accumulator.lastFailureAt = state.lastFailureAt;
      accumulator.lastError = state.lastError;
      accumulator.lastUpstreamStatus = state.lastUpstreamStatus;
    }
    credentialAccumulators.set(credentialRef, accumulator);
  }

  const quotaRemainingByCredentialId = new Map<string, number>();
  for (const item of input.credentialInventory ?? []) {
    const remaining = readQuotaRemainingUsd(item.providerQuota?.rawData);
    if (remaining !== null) {
      quotaRemainingByCredentialId.set(item.id, remaining);
    }
  }

  const credentials = new Map<string, ConsoleTelemetryCredential>();
  const credentialRefs = new Set([
    ...credentialAccumulators.keys(),
    ...quotaRemainingByCredentialId.keys(),
  ]);
  for (const credentialRef of credentialRefs) {
    const accumulator = credentialAccumulators.get(credentialRef);
    credentials.set(credentialRef, {
      credentialRef,
      providerAccountIds: [...(accumulator?.providerAccountIds ?? [])].sort((left, right) =>
        left.localeCompare(right),
      ),
      status: accumulator?.status ?? null,
      models: [...(accumulator?.models ?? [])].sort((left, right) => left.localeCompare(right)),
      failureCount: accumulator?.failureCount ?? 0,
      cooldownUntil: accumulator?.cooldownUntil ?? null,
      lastError: accumulator?.lastError ?? null,
      lastUpstreamStatus: accumulator?.lastUpstreamStatus ?? null,
      lastSuccessAt: accumulator?.lastSuccessAt ?? null,
      lastFailureAt: accumulator?.lastFailureAt ?? null,
      quotaRemainingUsd: quotaRemainingByCredentialId.get(credentialRef) ?? null,
    });
  }

  const credentialRefsByProviderAccountId = new Map<string, string[]>();
  for (const credential of credentials.values()) {
    for (const providerAccountId of credential.providerAccountIds) {
      const refs = credentialRefsByProviderAccountId.get(providerAccountId) ?? [];
      refs.push(credential.credentialRef);
      credentialRefsByProviderAccountId.set(providerAccountId, refs);
    }
  }

  return {
    credentials,
    providerAccounts,
    credentialRefsByProviderAccountId,
    hasPressure: input.pressure !== null,
    hasCosts: input.costOverview !== null,
    hasStates: input.credentialModelStates !== null,
    hasAudits: input.requestAuditSummary !== null,
  };
}
