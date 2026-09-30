import { useMemo } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { UiLocale } from "../../i18n/UiLocaleProvider";
import type { useConsoleRouteData } from "./useConsoleRouteData";
import type { CredentialProbeViewResult } from "./useConsoleProbeActions";
import type { PilotActionDialogState } from "./PilotActionDialog";
import type { AccountsLedgerPilotAccount } from "./AccountsLedgerWorkspace";
import type { ProviderMetricsAccountLike } from "./providerCardMetrics";
import { buildRouteAccountCatalog, buildRouteAccountCatalogFromSummary, filterRouteAccountCatalog, type RouteAccountCatalog } from "./routeAccountCatalog";
import { buildAccountLedgerRows, filterAccountLedgerRows, resolveGeminiLogicalChannel } from "./accountManagementViewModel";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildPilotStatsView } from "./pilotStatsView";

type RouteData = ReturnType<typeof useConsoleRouteData>;
type ConsoleAccountSelectorsOptions = {
  draftDocumentState: { document: ConsoleRouteDocument | null };
  routeConfig: RouteData["routeConfig"];
  accountGroupSummary: RouteData["accountGroupSummary"];
  draftMatchesActiveRevision: boolean;
  credentialProbeResults: Record<string, CredentialProbeViewResult>;
  runtimePressure: RouteData["runtimePressure"];
  costOverview: RouteData["costOverview"];
  requestAuditSummary: RouteData["requestAuditSummary"];
  credentialModelStates: RouteData["credentialModelStates"];
  providerCredentialInventory: RouteData["providerCredentialInventory"];
  accountSearch: string;
  accountMembershipFilter: Parameters<typeof filterRouteAccountCatalog>[2];
  accountEnabledFilter: NonNullable<Parameters<typeof filterRouteAccountCatalog>[3]>;
  selectedAccountGroupFilter: string;
  selectedProviderFilter: string;
  locale: UiLocale;
  pilotActionDialog: PilotActionDialogState | null;
  t: (zh: string, en: string) => string;
};

export function useConsoleAccountSelectors({
  draftDocumentState,
  routeConfig,
  accountGroupSummary,
  draftMatchesActiveRevision,
  credentialProbeResults,
  runtimePressure,
  costOverview,
  requestAuditSummary,
  credentialModelStates,
  providerCredentialInventory,
  accountSearch,
  accountMembershipFilter,
  accountEnabledFilter,
  selectedAccountGroupFilter,
  selectedProviderFilter,
  locale,
  pilotActionDialog,
  t,
}: ConsoleAccountSelectorsOptions) {
  const draftAccountCatalog = useMemo<RouteAccountCatalog>(() => {
    if (!draftDocumentState.document) {
      return {
        groups: [],
        accounts: [],
        providerBuckets: [],
        ungroupedCount: 0,
      };
    }
    return buildRouteAccountCatalog(draftDocumentState.document);
  }, [draftDocumentState.document]);

  const activeAccountCatalog = useMemo<RouteAccountCatalog | null>(() => {
    if (!routeConfig || !accountGroupSummary) {
      return null;
    }
    if (accountGroupSummary.routeConfigRevision !== routeConfig.routeConfig.revision.id) {
      return null;
    }
    return buildRouteAccountCatalogFromSummary(accountGroupSummary);
  }, [accountGroupSummary, routeConfig]);

  const displayedAccountCatalog = useMemo<RouteAccountCatalog>(
    () =>
      activeAccountCatalog && draftMatchesActiveRevision ? activeAccountCatalog : draftAccountCatalog,
    [activeAccountCatalog, draftAccountCatalog, draftMatchesActiveRevision],
  );

  const filteredAccountCatalog = useMemo(
    () =>
      filterRouteAccountCatalog(
        displayedAccountCatalog,
        accountSearch,
        accountMembershipFilter,
        accountEnabledFilter,
      ),
    [accountEnabledFilter, accountMembershipFilter, accountSearch, displayedAccountCatalog],
  );

  const accountLedgerRows = useMemo(
    () => buildAccountLedgerRows(displayedAccountCatalog, credentialProbeResults),
    [credentialProbeResults, displayedAccountCatalog],
  );

  const filteredAccountLedgerRows = useMemo(
    () =>
      filterAccountLedgerRows(accountLedgerRows, {
        query: accountSearch,
        membership: accountMembershipFilter,
        enabled: accountEnabledFilter,
        groupId: selectedAccountGroupFilter,
        providerKey: selectedProviderFilter,
      }),
    [
      accountEnabledFilter,
      accountLedgerRows,
      accountMembershipFilter,
      accountSearch,
      selectedAccountGroupFilter,
      selectedProviderFilter,
    ],
  );

  // One fold of polled telemetry, keyed by provider account and credential IDs.
  const consoleTelemetry = useMemo(
    () =>
      buildConsoleTelemetrySnapshot({
        pressure: runtimePressure,
        costOverview,
        requestAuditSummary,
        credentialModelStates,
        credentialInventory: providerCredentialInventory?.credentials ?? null,
      }),
    [
      costOverview,
      credentialModelStates,
      providerCredentialInventory,
      requestAuditSummary,
      runtimePressure,
    ],
  );

  const credentialInventoryById = useMemo(
    () =>
      new Map(
        providerCredentialInventory?.credentials.map((credential) => [
          credential.id,
          credential,
        ]) ?? [],
      ),
    [providerCredentialInventory],
  );

  const accountPilotSections = useMemo(
    () =>
      buildAccountLedgerSections(
        draftDocumentState.document,
        filteredAccountLedgerRows,
        credentialInventoryById,
        consoleTelemetry,
        locale,
      ),
    [
      consoleTelemetry,
      credentialInventoryById,
      draftDocumentState.document,
      filteredAccountLedgerRows,
      locale,
    ],
  );

  // Group totals must not change when the credential pool's search filters change.
  const unfilteredAccountPilotSections = useMemo(
    () =>
      buildAccountLedgerSections(
        draftDocumentState.document,
        accountLedgerRows,
        credentialInventoryById,
        consoleTelemetry,
        locale,
      ),
    [
      accountLedgerRows,
      consoleTelemetry,
      credentialInventoryById,
      draftDocumentState.document,
      locale,
    ],
  );

  const accountCardMetricsById = useMemo(() => {
    const metricsById = new Map<string, ProviderMetricsAccountLike>();
    for (const section of unfilteredAccountPilotSections) {
      const accounts = [
        ...section.directAccounts,
        ...section.identityCategories.flatMap((category) => category.accounts),
      ];
      for (const account of accounts) {
        metricsById.set(account.accountId, account);
      }
    }
    return metricsById;
  }, [unfilteredAccountPilotSections]);

  // Group members reuse the credential pool's account cards and metadata.
  const pilotAccountsById = useMemo(() => {
    const accountsById = new Map<string, AccountsLedgerPilotAccount>();
    for (const section of unfilteredAccountPilotSections) {
      const accounts = [
        ...section.directAccounts,
        ...section.identityCategories.flatMap((category) => category.accounts),
      ];
      for (const account of accounts) {
        accountsById.set(account.accountId, account);
      }
    }
    return accountsById;
  }, [unfilteredAccountPilotSections]);

  const accountLedgerGroupOptions = useMemo(
    () => [
      { value: "all", label: t("全部分组", "All groups") },
      ...displayedAccountCatalog.groups.map((group) => ({
        value: group.id,
        label: group.name,
      })),
    ],
    [displayedAccountCatalog.groups, t],
  );

  const accountLedgerProviderOptions = useMemo(
    () => {
      const options = new Map<string, string>();
      for (const bucket of displayedAccountCatalog.providerBuckets) {
        let hasLogicalGeminiProvider = false;
        for (const provider of bucket.providers) {
          const channel = resolveGeminiLogicalChannel(
            provider.id,
            provider.preset,
            provider.label,
          );
          if (!channel) {
            continue;
          }
          hasLogicalGeminiProvider = true;
          options.set(channel.key, channel.label);
        }
        if (!hasLogicalGeminiProvider) {
          options.set(bucket.providers[0]?.preset ?? bucket.vendorKey, bucket.label);
        }
      }
      return [
        { value: "all", label: t("全部服务商", "All providers") },
        ...[...options.entries()]
          .map(([value, label]) => ({ value, label }))
          .sort((left, right) => left.label.localeCompare(right.label)),
      ];
    },
    [displayedAccountCatalog.providerBuckets, t],
  );

  const displayedAccountsById = useMemo(
    () => new Map(displayedAccountCatalog.accounts.map((account) => [account.id, account])),
    [displayedAccountCatalog.accounts],
  );

  const activePilotManagedAccount = useMemo(
    () =>
      pilotActionDialog &&
      (pilotActionDialog.kind === "probe" || pilotActionDialog.kind === "stats")
        ? (displayedAccountsById.get(pilotActionDialog.account.accountId) ?? null)
        : null,
    [displayedAccountsById, pilotActionDialog],
  );

  const activePilotProbeResult = useMemo(
    () =>
      pilotActionDialog?.kind === "probe"
        ? (credentialProbeResults[pilotActionDialog.account.accountId] ?? null)
        : null,
    [credentialProbeResults, pilotActionDialog],
  );

  // Credential health and provider aggregates share the existing polled snapshot.
  const activePilotStatsView = useMemo(
    () =>
      pilotActionDialog?.kind === "stats"
        ? buildPilotStatsView(
            pilotActionDialog.account,
            pilotActionDialog.providerId,
            consoleTelemetry,
            locale,
          )
        : null,
    [consoleTelemetry, locale, pilotActionDialog],
  );

  return {
    draftAccountCatalog,
    activeAccountCatalog,
    displayedAccountCatalog,
    filteredAccountCatalog,
    accountLedgerRows,
    filteredAccountLedgerRows,
    consoleTelemetry,
    credentialInventoryById,
    accountPilotSections,
    unfilteredAccountPilotSections,
    accountCardMetricsById,
    pilotAccountsById,
    accountLedgerGroupOptions,
    accountLedgerProviderOptions,
    displayedAccountsById,
    activePilotManagedAccount,
    activePilotProbeResult,
    activePilotStatsView,
  };
}
