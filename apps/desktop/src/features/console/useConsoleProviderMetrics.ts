import { useMemo } from "react";
import type { ProviderDraftRow } from "./providerCredentialDraft";
import type { ProviderModelMappingEntry } from "./ProviderModelMappingDialog";
import type { ProviderMetricsAccountLike, ProviderMetricsResolveOptions, ProviderMetricsResolver } from "./providerCardMetrics";
import type { ConsoleTelemetrySnapshot } from "./telemetry";
import { resolveBillingMultiplier } from "./telemetryPresentation";
import { rollupHasData, rollupProviderAccountModelTelemetry, rollupProviderAccountTelemetry, type ConsoleTelemetryRollup } from "./telemetryRollups";
import { isRecord } from "./routeDocument";

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
      const entries: ProviderModelMappingEntry[] = [];
      const rawMap = row.provider.model_map;
      if (isRecord(rawMap)) {
        for (const [model, upstreamModel] of Object.entries(rawMap)) {
          if (typeof upstreamModel === "string") {
            entries.push({ model, upstreamModel });
          }
        }
        entries.sort((left, right) => left.model.localeCompare(right.model));
      }
      byProvider.set(providerId, entries);
    });
    return byProvider;
  }, [providerDraftRows]);

  // Provider rollups avoid counting the same traffic once per pooled credential.
  const providerMetricsResolver = useMemo<ProviderMetricsResolver>(() => {
    const upstreamModelFor = (providerAccountId: string, model: string): string =>
      providerModelMapEntries.get(providerAccountId)?.find((entry) => entry.model === model)
        ?.upstreamModel ?? model;
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
          rollupProviderAccountModelTelemetry(consoleTelemetry, [
            { providerAccountId, model: upstreamModelFor(providerAccountId, model) },
          ]),
          options,
        ),
    };
  }, [accountGroupSummary, consoleTelemetry, providerModelMapEntries]);

  return { providerModelMapEntries, providerMetricsResolver };
}
