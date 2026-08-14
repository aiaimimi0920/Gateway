import type {
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialProbeResult,
  ConsoleCredentialProbeStatus,
  ConsoleRouteDocument,
} from "../../api/contracts";
import {
  providerVerificationFor,
  type ProviderVerificationStatus,
} from "./providerVerificationRegistry";

export type RouteAccountGroup = {
  id: string;
  name: string;
  description: string | null;
  billingMultiplier: number;
  enabled: boolean;
  notes: string | null;
  providerCredentialIds: string[];
};

export type RouteManagedAccount = {
  id: string;
  displayName: string;
  vendorKey: string;
  vendorName: string;
  providerId: string;
  providerLabel: string;
  providerPreset: string | null;
  baseUrl: string | null;
  hostLabel: string | null;
  mode: "credential" | "provider-default";
  enabled: boolean;
  supportedModels: string[];
  groupIds: string[];
  groupNames: string[];
};

export type RouteProviderBucket = {
  key: string;
  label: string;
  vendorKey: string;
  accountCount: number;
  providers: Array<{
    id: string;
    label: string;
    preset: string | null;
    baseUrl: string | null;
    accounts: RouteManagedAccount[];
  }>;
};

export type RouteAccountCatalog = {
  groups: RouteAccountGroup[];
  accounts: RouteManagedAccount[];
  providerBuckets: RouteProviderBucket[];
  ungroupedCount: number;
};

export type AccountMembershipFilter = "all" | "grouped" | "ungrouped";
export type AccountEnabledFilter = "all" | "enabled" | "disabled";

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

export type AccountGroupDraftLike = {
  id: string;
  groupId: string;
  name: string;
  description: string;
  billingMultiplier: string;
  enabled: boolean;
  notes: string;
  providerCredentialIds: string[];
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

export type CredentialGroupDirectoryItem = {
  rowId: string;
  groupId: string;
  name: string;
  description: string;
  billingMultiplier: string;
  enabled: boolean;
  notes: string;
  memberCount: number;
  providerLabels: string[];
  modelLabels: string[];
};

export type GroupMemberCandidate = {
  accountId: string;
  displayName: string;
  providerId: string;
  providerLabel: string;
  vendorLabel: string;
  mode: "credential" | "provider-default";
  enabled: boolean;
  groupIds: string[];
  groupLabels: string[];
  selected: boolean;
  searchText: string;
};

type ProviderVendorMetadata = {
  bucketKey: string;
  key: string;
  name: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function optionalString(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

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

function normalizeBucketKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function providerVendorMetadata(
  provider: Record<string, unknown>,
  providerId: string,
  providerLabel: string,
  keyField: "vendor_key" | "vendorKey",
  nameField: "vendor_name" | "vendorName",
): ProviderVendorMetadata {
  const explicitKey = optionalString(provider, keyField);
  const explicitName = optionalString(provider, nameField);
  if (explicitKey) {
    return {
      bucketKey: `vendor:${normalizeBucketKey(explicitKey) || explicitKey.toLocaleLowerCase()}`,
      key: explicitKey,
      name: explicitName ?? explicitKey,
    };
  }
  if (explicitName) {
    const normalizedName = normalizeBucketKey(explicitName) || providerId;
    return {
      bucketKey: `vendor-name:${normalizedName}`,
      key: normalizedName,
      name: explicitName,
    };
  }
  return {
    bucketKey: `provider:${providerId}`,
    key: providerId,
    name: providerLabel,
  };
}

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

export function hostLabelFromUrl(url: string | null): string | null {
  if (!url) {
    return null;
  }
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

export function providerDefaultAccountId(providerId: string): string {
  return `${providerId}::default`;
}

export function routeAccountGroupsFromDocument(
  document: ConsoleRouteDocument,
): RouteAccountGroup[] {
  const groupsValue = (document as Record<string, unknown>).account_groups;
  if (!Array.isArray(groupsValue)) {
    return [];
  }
  return groupsValue
    .filter(isRecord)
    .map((entry, index) => ({
      id:
        typeof entry.id === "string" && entry.id.trim().length > 0
          ? entry.id.trim()
          : `group-${index}`,
      name:
        typeof entry.name === "string" && entry.name.trim().length > 0
          ? entry.name.trim()
          : `Group ${index + 1}`,
      description:
        typeof entry.description === "string" && entry.description.trim().length > 0
          ? entry.description.trim()
          : null,
      billingMultiplier:
        typeof entry.billing_multiplier === "number" &&
        Number.isFinite(entry.billing_multiplier) &&
        entry.billing_multiplier >= 0
          ? entry.billing_multiplier
          : 1,
      enabled: typeof entry.enabled === "boolean" ? entry.enabled : true,
      notes:
        typeof entry.notes === "string" && entry.notes.trim().length > 0
          ? entry.notes.trim()
          : null,
      providerCredentialIds: readStringArray(entry.provider_credential_ids),
    }));
}

export function buildRouteAccountCatalog(document: ConsoleRouteDocument): RouteAccountCatalog {
  const groups = routeAccountGroupsFromDocument(document);
  const memberships = new Map<string, RouteAccountGroup[]>();
  for (const group of groups) {
    for (const providerCredentialId of group.providerCredentialIds) {
      const existing = memberships.get(providerCredentialId) ?? [];
      existing.push(group);
      memberships.set(providerCredentialId, existing);
    }
  }

  const providerBucketsMap = new Map<string, RouteProviderBucket>();
  const accounts: RouteManagedAccount[] = [];

  for (const provider of document.providers) {
    if (!isRecord(provider)) {
      continue;
    }
    const providerId = typeof provider.id === "string" ? provider.id.trim() : "";
    if (providerId.length === 0) {
      continue;
    }
    const providerLabel =
      typeof provider.label === "string" && provider.label.trim().length > 0
        ? provider.label.trim()
        : providerId;
    const providerPreset =
      typeof provider.preset === "string" && provider.preset.trim().length > 0
        ? provider.preset.trim()
        : null;
    const providerBaseUrl =
      typeof provider.base_url === "string" && provider.base_url.trim().length > 0
        ? provider.base_url.trim()
        : null;
    const providerSupportedModels = readStringArray(provider.supported_models);
    const vendor = providerVendorMetadata(
      provider,
      providerId,
      providerLabel,
      "vendor_key",
      "vendor_name",
    );
    const providerAccounts: RouteManagedAccount[] = [];

    const credentials = Array.isArray(provider.credentials)
      ? provider.credentials.filter(isRecord)
      : [];
    if (credentials.length > 0) {
      credentials.forEach((credential, index) => {
        const accountId =
          typeof credential.id === "string" && credential.id.trim().length > 0
            ? credential.id.trim()
            : `${providerId}-cred-${index}`;
        const displayName =
          typeof credential.account_name === "string" && credential.account_name.trim().length > 0
            ? credential.account_name.trim()
            : accountId;
        const baseUrl =
          typeof credential.base_url === "string" && credential.base_url.trim().length > 0
            ? credential.base_url.trim()
            : providerBaseUrl;
        const supportedModels = readStringArray(credential.supported_models);
        const accountGroups = memberships.get(accountId) ?? [];
        const account: RouteManagedAccount = {
          id: accountId,
          displayName,
          vendorKey: vendor.key,
          vendorName: vendor.name,
          providerId,
          providerLabel,
          providerPreset,
          baseUrl,
          hostLabel: hostLabelFromUrl(baseUrl),
          mode: "credential",
          enabled: typeof credential.enabled === "boolean" ? credential.enabled : true,
          supportedModels: supportedModels.length > 0 ? supportedModels : providerSupportedModels,
          groupIds: accountGroups.map((group) => group.id),
          groupNames: accountGroups.map((group) => group.name),
        };
        providerAccounts.push(account);
        accounts.push(account);
      });
    } else {
      const accountId = providerDefaultAccountId(providerId);
      const accountGroups = memberships.get(accountId) ?? [];
      const account: RouteManagedAccount = {
        id: accountId,
        displayName:
          typeof provider.account_name === "string" && provider.account_name.trim().length > 0
            ? provider.account_name.trim()
            : providerLabel,
        vendorKey: vendor.key,
        vendorName: vendor.name,
        providerId,
        providerLabel,
        providerPreset,
        baseUrl: providerBaseUrl,
        hostLabel: hostLabelFromUrl(providerBaseUrl),
        mode: "provider-default",
        enabled: typeof provider.enabled === "boolean" ? provider.enabled : true,
        supportedModels: providerSupportedModels,
        groupIds: accountGroups.map((group) => group.id),
        groupNames: accountGroups.map((group) => group.name),
      };
      providerAccounts.push(account);
      accounts.push(account);
    }

    providerAccounts.sort((left, right) => left.displayName.localeCompare(right.displayName));
    const bucket = providerBucketsMap.get(vendor.bucketKey) ?? {
      key: vendor.bucketKey,
      label: vendor.name,
      vendorKey: vendor.key,
      accountCount: 0,
      providers: [],
    };
    bucket.providers.push({
      id: providerId,
      label: providerLabel,
      preset: providerPreset,
      baseUrl: providerBaseUrl,
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
    groups: [...groups].sort((left, right) => left.name.localeCompare(right.name)),
    accounts: [...accounts].sort((left, right) => left.displayName.localeCompare(right.displayName)),
    providerBuckets,
    ungroupedCount: accounts.filter((account) => account.groupIds.length === 0).length,
  };
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
      provider.label || provider.preset || hostLabelFromUrl(provider.baseUrl ?? null) || provider.id;
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

export function buildCredentialGroupDirectory(
  catalog: RouteAccountCatalog,
  rows: AccountGroupDraftLike[],
): CredentialGroupDirectoryItem[] {
  const accountsById = new Map(catalog.accounts.map((account) => [account.id, account]));
  return rows.map((row) => {
    const members = row.providerCredentialIds
      .map((credentialId) => accountsById.get(credentialId))
      .filter((account): account is RouteManagedAccount => Boolean(account));
    const providerLabels = [...new Set(members.map((member) => member.providerLabel))].sort(
      (left, right) => left.localeCompare(right),
    );
    const modelLabels = [...new Set(members.flatMap((member) => member.supportedModels))].sort(
      (left, right) => left.localeCompare(right),
    );
    return {
      rowId: row.id,
      groupId: row.groupId,
      name: row.name,
      description: row.description,
      billingMultiplier: row.billingMultiplier,
      enabled: row.enabled,
      notes: row.notes,
      memberCount: members.length,
      providerLabels,
      modelLabels,
    };
  });
}

export function buildGroupMemberCandidates(
  accounts: RouteManagedAccount[],
  filters: {
    selectedCredentialIds: string[];
    query: string;
    mode: "all" | "members" | "ungrouped";
  },
): GroupMemberCandidate[] {
  const selectedIds = new Set(filters.selectedCredentialIds);
  const normalizedQuery = filters.query.trim().toLowerCase();
  return accounts
    .map((account) => ({
      accountId: account.id,
      displayName: account.displayName,
      providerId: account.providerId,
      providerLabel: account.providerLabel,
      vendorLabel: account.vendorName,
      mode: account.mode,
      enabled: account.enabled,
      groupIds: [...account.groupIds],
      groupLabels: [...account.groupNames],
      selected: selectedIds.has(account.id),
      searchText: [
        account.displayName,
        account.id,
        account.providerLabel,
        account.vendorName,
        ...account.supportedModels,
        ...account.groupNames,
      ]
        .join(" ")
        .toLowerCase(),
    }))
    .filter((account) => {
      if (filters.mode === "members" && !account.selected) {
        return false;
      }
      if (filters.mode === "ungrouped" && account.groupIds.length > 0 && !account.selected) {
        return false;
      }
      if (normalizedQuery.length > 0 && !account.searchText.includes(normalizedQuery)) {
        return false;
      }
      return true;
    })
    .sort((left, right) => left.displayName.localeCompare(right.displayName));
}
