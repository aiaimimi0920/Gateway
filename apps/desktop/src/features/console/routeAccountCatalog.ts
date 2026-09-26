import type { ConsoleRouteDocument, ConsoleAccountGroupSummaryResponse } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { indexAccountsByProvider } from "./accountCatalogIndex";

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

export function providerDefaultAccountId(providerId: string): string {
  return `${providerId}::default`;
}

export function normalizeBucketKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

type ProviderVendorMetadata = {
  bucketKey: string;
  key: string;
  name: string;
};

export function optionalString(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
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

export function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

export function routeAccountGroupsFromDocument(document: ConsoleRouteDocument): RouteAccountGroup[] {
  const groupsValue = (document as Record<string, unknown>).account_groups;
  if (!Array.isArray(groupsValue)) {
    return [];
  }

  return groupsValue
    .map((entry) => {
      if (!isRecord(entry)) {
        return null;
      }
      const groupId = typeof entry.id === "string" ? entry.id.trim() : "";
      const name = typeof entry.name === "string" ? entry.name.trim() : "";
      if (groupId.length === 0 && name.length === 0) {
        return null;
      }
      return {
        id: groupId,
        name,
        description:
          typeof entry.description === "string" && entry.description.trim().length > 0
            ? entry.description.trim()
            : null,
        billingMultiplier:
          typeof entry.billing_multiplier === "number" && Number.isFinite(entry.billing_multiplier)
            ? entry.billing_multiplier
            : 1,
        enabled: typeof entry.enabled === "boolean" ? entry.enabled : true,
        notes:
          typeof entry.notes === "string" && entry.notes.trim().length > 0
            ? entry.notes.trim()
            : null,
        providerCredentialIds: readStringArray(entry.provider_credential_ids),
      } satisfies RouteAccountGroup;
    })
    .filter((entry): entry is RouteAccountGroup => entry !== null);
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
        providerPreset: account.providerPreset ?? provider?.preset ?? null,
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
  const accountsByProviderId = indexAccountsByProvider(accounts);

  for (const provider of summary.providers) {
    const providerAccounts = accountsByProviderId.get(provider.id) ?? [];
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
