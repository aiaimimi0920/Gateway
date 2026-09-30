import type { AccountsLedgerPilotAccount } from "./ProviderAccountCard";
import {
  aggregateProviderMetrics,
  aggregateProviderQuotaWindows,
  aggregateQuotaRemainingUsd,
  buildProviderAvailabilityCells,
  buildProviderPoolSegments,
  type AggregatedProviderQuotaWindow,
  type AggregatedSuccessWindow,
  type ProviderAggregateMetrics,
  type ProviderAvailabilityCell,
  type ProviderConcurrency,
  type ProviderCosts,
  type ProviderPoolSegments,
} from "./providerCardMetrics";

export type ProviderCardSnapshotSource = {
  supportsIdentityCategories: boolean;
  identityCategories: readonly { accounts: readonly AccountsLedgerPilotAccount[] }[];
  directAccounts: readonly AccountsLedgerPilotAccount[];
  poolTargetSize: number;
  telemetry?: ProviderAggregateMetrics | null;
};

export type ProviderCardSnapshot = {
  accounts: AccountsLedgerPilotAccount[];
  availablePoolCount: number;
  supportedModels: string[];
  poolSegments: ProviderPoolSegments;
  concurrency: ProviderConcurrency;
  costs: ProviderCosts;
  requestCount: number | null;
  successWindows: AggregatedSuccessWindow[];
  successTotals: { success: number; requests: number };
  successRate: number | null;
  availabilityCells: ProviderAvailabilityCell[];
  quotaWindows: AggregatedProviderQuotaWindow[];
  quotaRemainingUsd: number | null;
  actionableAccounts: AccountsLedgerPilotAccount[];
  hasProbeTargets: boolean;
};

/** Cacheable provider-card data; local flip/menu state must not rescan the account pool. */
export function buildProviderCardSnapshot(source: ProviderCardSnapshotSource): ProviderCardSnapshot {
  const rawAccounts = source.supportsIdentityCategories
    ? source.identityCategories.flatMap((category) => category.accounts)
    : source.directAccounts;
  const accounts = [
    ...new Map(
      rawAccounts.map((account) => [`${account.providerId}:${account.accountId}`, account]),
    ).values(),
  ];
  const poolSegments = buildProviderPoolSegments(accounts, source.poolTargetSize);
  const metrics = source.telemetry ?? aggregateProviderMetrics(accounts);
  const successTotals = metrics.successWindows.reduce(
    (totals, window) => ({
      success: totals.success + window.success,
      requests: totals.requests + window.requests,
    }),
    { success: 0, requests: 0 },
  );

  return {
    accounts,
    supportedModels: [...new Set(accounts.flatMap((account) => account.supportedModels ?? []))],
    availablePoolCount: poolSegments.available,
    poolSegments,
    concurrency: metrics.concurrency,
    costs: { upstream: metrics.upstreamCost, user: metrics.platformRevenue },
    requestCount: metrics.requests,
    successWindows: metrics.successWindows,
    successTotals,
    successRate:
      successTotals.requests > 0 ? successTotals.success / successTotals.requests : null,
    availabilityCells: buildProviderAvailabilityCells(metrics.successWindows),
    quotaWindows: aggregateProviderQuotaWindows(accounts).slice(0, 2),
    quotaRemainingUsd: aggregateQuotaRemainingUsd(accounts),
    actionableAccounts: accounts.filter(
      (account) =>
        account.mode === "credential" && account.dispatchEditable && !account.previewOnly,
    ),
    hasProbeTargets: accounts.some((account) => !account.previewOnly),
  };
}
