import type {
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialProbeResult,
  ConsoleCredentialProbeStatus,
} from "../../api/contracts";
import {
  hostLabelFromUrl,
  optionalString,
  providerVendorMetadata,
} from "./accountCatalogSupport";
import type {
  AccountEnabledFilter,
  AccountMembershipFilter,
  RouteAccountCatalog,
  RouteManagedAccount,
  RouteProviderBucket,
} from "./accountCatalogTypes";
import {
  providerVerificationFor,
  type ProviderVerificationStatus,
} from "./providerVerificationRegistry";
import { providerBrandLabel } from "./providerBrandLabel";

export { hostLabelFromUrl } from "./accountCatalogSupport";
export type {
  AccountEnabledFilter,
  AccountMembershipFilter,
  RouteAccountCatalog,
  RouteAccountGroup,
  RouteManagedAccount,
  RouteProviderBucket,
} from "./accountCatalogTypes";
export {
  buildRouteAccountCatalog,
  providerDefaultAccountId,
  routeAccountGroupsFromDocument,
} from "./routeDocumentAccountCatalog";
export {
  buildCredentialGroupDirectory,
  buildGroupMemberCandidates,
} from "./groupDirectoryViewModel";
export type {
  AccountGroupDraftLike,
  CredentialGroupDirectoryItem,
  CredentialGroupModelProviderMetrics,
  CredentialGroupModelScope,
  CredentialGroupProviderScope,
  CredentialGroupScopeMember,
  GroupMemberCandidate,
} from "./groupDirectoryViewModel";

export type GeminiLogicalChannel = {
  key: "logical:gemini" | "logical:gemini-business" | "logical:gemini-canvas";
  label: "Gemini" | "Gemini Business" | "Gemini Canvas";
  primaryProviderId: "gemini-canvas-chat" | "gemini-business" | "gemini-canvas";
  manualAddFamily: "gemini-canvas-chat" | "gemini-business" | "gemini-canvas";
  order: number;
};

export type CredentialProbeViewResult = ConsoleCredentialProbeResult | {
  credentialId: string;
  providerId: string;
  status: ConsoleCredentialProbeStatus | "error";
  message: string;
  checkedAt: string;
};

export type AccountLedgerRow = {
  accountId: string;
  displayName: string;
  providerId: string;
  providerLabel: string;
  providerPreset: string | null;
  vendorKey: string;
  vendorLabel: string;
  mode: "credential" | "provider-default";
  enabled: boolean;
  supportedModels: string[];
  groupIds: string[];
  groupLabels: string[];
  hostLabel: string | null;
  probeStatus: "not-tested" | "passed" | "failed" | "unsupported" | "error";
  probeMessage: string | null;
  probeCheckedAt: string | null;
  verificationStatus: ProviderVerificationStatus;
  verificationFamilies: string[];
  verificationCheckedAt: string | null;
  verificationEvidenceRef: string | null;
  verificationNote: string;
  searchText: string;
};

export function resolveGeminiLogicalChannel(
  providerId: string,
  providerPreset: string | null,
  providerLabel = "",
): GeminiLogicalChannel | null {
  const candidates = [providerId, providerPreset, providerLabel]
    .filter((value): value is string => Boolean(value))
    .map((value) => value.trim().toLowerCase());

  if (candidates.includes("gemini-business")) {
    return {
      key: "logical:gemini-business",
      label: "Gemini Business",
      primaryProviderId: "gemini-business",
      manualAddFamily: "gemini-business",
      order: 1,
    };
  }
  if (
    candidates.includes("gemini-canvas") ||
    candidates.includes("gemini-canvas-program-relay")
  ) {
    return {
      key: "logical:gemini-canvas",
      label: "Gemini Canvas",
      primaryProviderId: "gemini-canvas",
      manualAddFamily: "gemini-canvas",
      order: 2,
    };
  }
  if (
    candidates.includes("gemini-canvas-chat") ||
    candidates.includes("gemini-web") ||
    candidates.includes("gemini-web-chat") ||
    candidates.includes("gemini-web-chat-modular") ||
    candidates.includes("gemini-web-secondary")
  ) {
    return {
      key: "logical:gemini",
      label: "Gemini",
      primaryProviderId: "gemini-canvas-chat",
      manualAddFamily: "gemini-canvas-chat",
      order: 0,
    };
  }
  return null;
}

function normalizeSummaryAccountMode(mode: string): RouteManagedAccount["mode"] {
  return mode === "provider_default" ? "provider-default" : "credential";
}

export function buildRouteAccountCatalogFromSummary(
  summary: ConsoleAccountGroupSummaryResponse["summary"],
): RouteAccountCatalog {
  const groups = summary.accountGroups.map((group) => ({
    id: group.id,
    name: group.name,
    description: group.description ?? null,
    billingMultiplier: group.billingMultiplier,
    enabled: group.enabled,
    notes: group.notes ?? null,
    providerCredentialIds: [...group.providerCredentialIds],
  }));
  const groupsById = new Map(groups.map((group) => [group.id, group]));
  const providersById = new Map(summary.providers.map((provider) => [provider.id, provider]));
  const providerBucketsMap = new Map<string, RouteProviderBucket>();

  const accounts = summary.accounts
    .filter((account) => providersById.has(account.providerId))
    .map((account) => {
      const provider = providersById.get(account.providerId)!;
      const baseUrl = account.baseUrl ?? provider.baseUrl ?? null;
      const providerRecord = provider as unknown as Record<string, unknown>;
      const accountRecord = account as unknown as Record<string, unknown>;
      const vendor = providerVendorMetadata(
        {
          vendorKey:
            optionalString(accountRecord, "vendorKey") ??
            optionalString(providerRecord, "vendorKey"),
          vendorName:
            optionalString(accountRecord, "vendorName") ??
            optionalString(providerRecord, "vendorName"),
        },
        account.providerId,
        account.providerLabel,
        "vendorKey",
        "vendorName",
      );
      return {
        id: account.id,
        displayName: account.displayName,
        vendorKey: vendor.key,
        vendorName: vendor.name,
        providerId: account.providerId,
        providerLabel: account.providerLabel,
        providerPreset: account.providerPreset ?? provider.preset ?? null,
        baseUrl,
        hostLabel: hostLabelFromUrl(baseUrl),
        mode: normalizeSummaryAccountMode(account.mode),
        enabled: account.enabled,
        supportedModels: [...account.supportedModels],
        groupIds: [...account.groupIds],
        groupNames: account.groupIds
          .map((groupId) => groupsById.get(groupId)?.name)
          .filter((groupName): groupName is string => Boolean(groupName)),
      } satisfies RouteManagedAccount;
    })
    .sort((left, right) => left.displayName.localeCompare(right.displayName));

  for (const provider of summary.providers) {
    const providerAccounts = accounts
      .filter((account) => account.providerId === provider.id)
      .sort((left, right) => left.displayName.localeCompare(right.displayName));
    const providerLabel =
      providerBrandLabel({
        providerId: provider.id,
        providerPreset: provider.preset ?? null,
        providerLabel: provider.label ?? null,
        vendorName: provider.vendorName ?? null,
      }) || hostLabelFromUrl(provider.baseUrl ?? null) || provider.id;
    const vendor = providerVendorMetadata(
      provider as unknown as Record<string, unknown>,
      provider.id,
      providerLabel,
      "vendorKey",
      "vendorName",
    );
    const bucket = providerBucketsMap.get(vendor.bucketKey) ?? {
      key: vendor.bucketKey,
      label: vendor.name,
      vendorKey: vendor.key,
      accountCount: 0,
      providers: [],
    };
    bucket.providers.push({
      id: provider.id,
      label: provider.label,
      preset: provider.preset ?? null,
      baseUrl: provider.baseUrl ?? null,
      accounts: providerAccounts,
    });
    bucket.accountCount += providerAccounts.length;
    providerBucketsMap.set(vendor.bucketKey, bucket);
  }

  const providerBuckets = [...providerBucketsMap.values()]
    .map((bucket) => ({
      ...bucket,
      providers: [...bucket.providers].sort((left, right) => left.label.localeCompare(right.label)),
    }))
    .sort((left, right) => left.label.localeCompare(right.label));

  return {
    groups: groups.sort((left, right) => left.name.localeCompare(right.name)),
    accounts,
    providerBuckets,
    ungroupedCount: accounts.filter((account) => account.groupIds.length === 0).length,
  };
}

export function filterRouteAccountCatalog(
  catalog: RouteAccountCatalog,
  query: string,
  membershipFilter: AccountMembershipFilter,
  enabledFilter: AccountEnabledFilter = "all",
): RouteAccountCatalog {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const matchesAccount = (account: RouteManagedAccount) => {
    if (membershipFilter === "grouped" && account.groupIds.length === 0) {
      return false;
    }
    if (membershipFilter === "ungrouped" && account.groupIds.length > 0) {
      return false;
    }
    if (enabledFilter === "enabled" && !account.enabled) {
      return false;
    }
    if (enabledFilter === "disabled" && account.enabled) {
      return false;
    }
    if (normalizedQuery.length === 0) {
      return true;
    }
    return [
      account.displayName,
      account.id,
      account.vendorKey,
      account.vendorName,
      account.providerId,
      account.providerLabel,
      account.providerPreset ?? "",
      account.hostLabel ?? "",
      ...account.supportedModels,
      ...account.groupNames,
    ]
      .join("\n")
      .toLocaleLowerCase()
      .includes(normalizedQuery);
  };

  const accounts = catalog.accounts.filter(matchesAccount);
  const visibleAccountIds = new Set(accounts.map((account) => account.id));
  const providerBuckets = catalog.providerBuckets
    .map((bucket) => {
      const providers = bucket.providers
        .map((provider) => ({
          ...provider,
          accounts: provider.accounts.filter((account) => visibleAccountIds.has(account.id)),
        }))
        .filter((provider) => provider.accounts.length > 0);
      return {
        ...bucket,
        accountCount: providers.reduce((total, provider) => total + provider.accounts.length, 0),
        providers,
      };
    })
    .filter((bucket) => bucket.accountCount > 0);

  return {
    groups: catalog.groups,
    accounts,
    providerBuckets,
    ungroupedCount: accounts.filter((account) => account.groupIds.length === 0).length,
  };
}

export function buildAccountLedgerRows(
  catalog: RouteAccountCatalog,
  probeResults: Record<string, CredentialProbeViewResult>,
): AccountLedgerRow[] {
  return catalog.accounts.map((account) => {
    const probe = probeResults[account.id];
    return {
      accountId: account.id,
      displayName: account.displayName,
      providerId: account.providerId,
      providerLabel: account.providerLabel,
      providerPreset: account.providerPreset,
      vendorKey: account.vendorKey,
      vendorLabel: account.vendorName,
      mode: account.mode,
      enabled: account.enabled,
      supportedModels: [...account.supportedModels],
      groupIds: [...account.groupIds],
      groupLabels: [...account.groupNames],
      hostLabel: account.hostLabel,
      probeStatus: probe?.status ?? "not-tested",
      probeMessage: probe?.message ?? null,
      probeCheckedAt: probe?.checkedAt ?? null,
      verificationStatus: providerVerificationFor(account.providerId, account.vendorKey).status,
      verificationFamilies: providerVerificationFor(account.providerId, account.vendorKey).families,
      verificationCheckedAt: providerVerificationFor(account.providerId, account.vendorKey).checkedAt,
      verificationEvidenceRef: providerVerificationFor(account.providerId, account.vendorKey).evidenceRef,
      verificationNote: providerVerificationFor(account.providerId, account.vendorKey).note,
      searchText: [
        account.displayName,
        account.id,
        account.providerId,
        account.providerLabel,
        account.vendorName,
        ...account.supportedModels,
        ...account.groupNames,
      ]
        .join(" ")
        .toLowerCase(),
    };
  });
}

export function filterAccountLedgerRows(
  rows: AccountLedgerRow[],
  filters: {
    query: string;
    membership: AccountMembershipFilter;
    enabled: AccountEnabledFilter;
    groupId: string;
    providerKey: string;
  },
): AccountLedgerRow[] {
  const query = filters.query.trim().toLowerCase();
  return rows.filter((row) => {
    if (query.length > 0 && !row.searchText.includes(query)) {
      return false;
    }
    if (filters.membership === "grouped" && row.groupIds.length === 0) {
      return false;
    }
    if (filters.membership === "ungrouped" && row.groupIds.length > 0) {
      return false;
    }
    if (filters.enabled === "enabled" && !row.enabled) {
      return false;
    }
    if (filters.enabled === "disabled" && row.enabled) {
      return false;
    }
    if (filters.groupId !== "all" && !row.groupIds.includes(filters.groupId)) {
      return false;
    }
    if (filters.providerKey !== "all") {
      const logicalChannel = resolveGeminiLogicalChannel(
        row.providerId,
        row.providerPreset,
        row.providerLabel,
      );
      const providerMatches = [
        row.vendorKey,
        row.providerId,
        row.providerPreset ?? "",
        logicalChannel?.key ?? "",
      ].includes(filters.providerKey);
      if (!providerMatches) {
        return false;
      }
    }
    return true;
  });
}
