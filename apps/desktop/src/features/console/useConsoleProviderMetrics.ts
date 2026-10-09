import { useMemo } from "react";
import type { ProviderDraftRow } from "./providerCredentialDraft";
import type { ProviderModelMappingEntry } from "./ProviderModelMappingDialog";
import type { ProviderMetricsAccountLike, ProviderMetricsResolveOptions, ProviderMetricsResolver } from "./providerCardMetrics";
import type { ConsoleTelemetrySnapshot } from "./telemetry";
import { resolveBillingMultiplier } from "./telemetryPresentation";
import { rollupHasData, rollupProviderAccountModelTelemetry, rollupProviderAccountTelemetry, type ConsoleTelemetryRollup } from "./telemetryRollups";
import { isRecord } from "./routeDocument";
import { readProviderModelMappings } from "./providerModelMappingDocument";

type ConsoleProviderMetricsOptions = {
  providerDraftRows: ProviderDraftRow[];
  consoleTelemetry: ConsoleTelemetrySnapshot;
  accountGroupSummary: Parameters<typeof resolveBillingMultiplier>[0];
};

export function useConsoleProviderMetrics({
  providerDraftRows,
  consoleTelemetry,
  accountGroupSummary,
}: ConsoleProviderMetricsOptions) {
  // model_map contains upstream overrides; unmapped pool model names stay unchanged.
  const providerModelMapEntries = useMemo(() => {
    const byProvider = new Map<string, ProviderModelMappingEntry[]>();
    providerDraftRows.forEach((row) => {
      const providerId = row.providerId.trim();
      if (providerId.length === 0) {
        return;
      }
      const entries = readProviderModelMappings(row.provider);
      byProvider.set(providerId, entries);
    });
    return byProvider;
  }, [providerDraftRows]);

  // Provider rollups avoid counting the same traffic once per pooled credential.
  const providerMetricsResolver = useMemo<ProviderMetricsResolver>(() => {
    const upstreamModelsFor = (providerAccountId: string, model: string): string[] => {
      const mapped = providerModelMapEntries.get(providerAccountId)?.filter((entry) => entry.model === model).map((entry) => entry.upstreamModel) ?? [];
      return mapped.length ? [...new Set(mapped)] : [model];
    };
    // A card that bills at one known rate passes it; a card spanning several
    // groups passes its credentials and takes the ceiling rate they imply.
    const multiplierFor = (options: ProviderMetricsResolveOptions | undefined): number =>
      typeof options?.billingMultiplier === "number" && Number.isFinite(options.billingMultiplier)
        ? options.billingMultiplier
        : resolveBillingMultiplier(accountGroupSummary, options?.credentialRefs ?? []);
    const toAccountLike = (
      rollup: ConsoleTelemetryRollup,
      options: ProviderMetricsResolveOptions | undefined,
    ): ProviderMetricsAccountLike | null => {
      // Ids the snapshot does not know roll up to all-null, which is not the
      // same claim as zero traffic — return null so the card keeps rendering `—`.
      if (!rollupHasData(rollup)) {
        return null;
      }
      const multiplier = multiplierFor(options);
      return {
        concurrencyUsed: rollup.concurrencyUsed,
        concurrencyTotal: rollup.concurrencyTotal,
        requestCount: rollup.requestCount,
        upstreamCost: rollup.upstreamCostUsd,
        userCost: rollup.upstreamCostUsd === null ? null : rollup.upstreamCostUsd * multiplier,
        successWindows: rollup.successWindows,
      };
    };
    return {
      providerAccounts: (providerAccountIds, options) =>
        toAccountLike(
          rollupProviderAccountTelemetry(consoleTelemetry, providerAccountIds),
          options,
        ),
      providerAccountModel: (providerAccountId, model, options) =>
        toAccountLike(
          rollupProviderAccountModelTelemetry(consoleTelemetry,
            upstreamModelsFor(providerAccountId, model).map((model) => ({ providerAccountId, model }))),
          options,
        ),
    };
  }, [accountGroupSummary, consoleTelemetry, providerModelMapEntries]);

  return { providerModelMapEntries, providerMetricsResolver };
}
