import { useMemo } from "react";
import { buildModelPoolDirectory, modelRouteChainsFromRows } from "./modelPoolViewModel";
import { parseSupportedModelsText, type ProviderDraftRow } from "./providerCredentialDraft";
import type { ModelRouteDraftRow } from "./modelRouteDraft";
import type { ProviderModelMappingEntry } from "./ProviderModelMappingDialog";
import type { AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import { buildProviderCardSnapshot } from "./providerCardSnapshot";
import { rankedMappingModels } from "./providerModelMappingDocument";
import type { ConsoleTelemetrySnapshot } from "./telemetry";

type ConsoleModelSelectorsOptions = {
  displayedAccountCatalog: Parameters<typeof buildModelPoolDirectory>[0];
  accountCardMetricsById: Parameters<typeof buildModelPoolDirectory>[2];
  providerMetricsResolver: Parameters<typeof buildModelPoolDirectory>[4];
  providerDraftRows: ProviderDraftRow[];
  modelRouteDraftRows: ModelRouteDraftRow[];
  providerModelMapEntries: ReadonlyMap<string, ProviderModelMappingEntry[]>;
  accountPilotSections: AccountsLedgerPilotSection[];
  providerModelMappingProviderId: string | null;
  consoleTelemetry: ConsoleTelemetrySnapshot;
};

export function useConsoleModelSelectors({
  displayedAccountCatalog,
  accountCardMetricsById,
  providerMetricsResolver,
  providerDraftRows,
  modelRouteDraftRows,
  providerModelMapEntries,
  accountPilotSections,
  providerModelMappingProviderId,
  consoleTelemetry,
}: ConsoleModelSelectorsOptions) {
  const modelPoolChains = useMemo(
    () => modelRouteChainsFromRows(modelRouteDraftRows),
    [modelRouteDraftRows],
  );

  const modelPoolProviderOrder = useMemo(
    () => providerDraftRows.map((row) => row.providerId.trim()).filter((id) => id.length > 0),
    [providerDraftRows],
  );

  const modelPoolDirectory = useMemo(
    () =>
      buildModelPoolDirectory(
        displayedAccountCatalog,
        modelPoolChains,
        accountCardMetricsById,
        modelPoolProviderOrder,
        providerMetricsResolver,
      ),
    [
      accountCardMetricsById,
      displayedAccountCatalog,
      modelPoolChains,
      modelPoolProviderOrder,
      providerMetricsResolver,
    ],
  );

  // The add/edit dialog offers providers in the same order as the model cards.
  const modelPoolDialogProviderOptions = useMemo(() => {
    const labels = new Map<string, string>();
    for (const bucket of displayedAccountCatalog.providerBuckets) {
      for (const provider of bucket.providers) {
        labels.set(provider.id, provider.label);
      }
    }
    const options = modelPoolProviderOrder.map((id) => ({ id, label: labels.get(id) ?? id }));
    const listed = new Set(options.map((option) => option.id));
    for (const [id, label] of labels) {
      if (!listed.has(id)) {
        options.push({ id, label });
      }
    }
    return options;
  }, [displayedAccountCatalog.providerBuckets, modelPoolProviderOrder]);

  const modelPoolModelNames = useMemo(
    () => modelPoolDirectory.map((card) => card.model),
    [modelPoolDirectory],
  );

  const modelMappingCountByProvider = useMemo(() => {
    const counts = new Map<string, number>();
    for (const [providerId, entries] of providerModelMapEntries) {
      counts.set(providerId, new Set(entries.map((entry) => entry.model)).size);
    }
    return counts;
  }, [providerModelMapEntries]);

  const providerModelMappingTarget = useMemo(() => {
    if (providerModelMappingProviderId === null) {
      return null;
    }
    const row = providerDraftRows.find(
      (candidate) => candidate.providerId.trim() === providerModelMappingProviderId,
    );
    const providerLabel =
      accountPilotSections.find(
        (section) => section.providerId === providerModelMappingProviderId,
      )?.providerLabel ??
      modelPoolDialogProviderOptions.find(
        (option) => option.id === providerModelMappingProviderId,
      )?.label ??
      providerModelMappingProviderId;
    // Suggest what this provider declares first, then everything the pool holds,
    // because a provider can serve a pool model without declaring it.
    const declared = row ? parseSupportedModelsText(row.supportedModelsText) : [];
    const section = accountPilotSections.find((section) => section.providerId === providerModelMappingProviderId);
    const upstreamModelOptions = [...new Set([...declared, ...(section ? buildProviderCardSnapshot(section).supportedModels : [])])];
    const modelOptions = rankedMappingModels([...upstreamModelOptions, ...modelPoolModelNames], consoleTelemetry);
    return {
      providerId: providerModelMappingProviderId,
      providerLabel,
      modelOptions,
      upstreamModelOptions,
      entries: providerModelMapEntries.get(providerModelMappingProviderId) ?? [],
    };
  }, [
    accountPilotSections,
    modelPoolDialogProviderOptions,
    modelPoolModelNames,
    providerDraftRows,
    providerModelMapEntries,
    providerModelMappingProviderId,
    consoleTelemetry,
  ]);

  return { modelPoolChains, modelPoolProviderOrder, modelPoolDirectory, modelPoolDialogProviderOptions, modelPoolModelNames, modelMappingCountByProvider, providerModelMappingTarget };
}
