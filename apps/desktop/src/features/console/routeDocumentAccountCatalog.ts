import type { ConsoleRouteDocument } from "../../api/contracts";
import {
  hostLabelFromUrl,
  providerVendorMetadata,
} from "./accountCatalogSupport";
import type {
  RouteAccountCatalog,
  RouteAccountGroup,
  RouteManagedAccount,
  RouteProviderBucket,
} from "./accountCatalogTypes";
import { providerBrandLabel } from "./providerBrandLabel";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
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
    const providerPreset =
      typeof provider.preset === "string" && provider.preset.trim().length > 0
        ? provider.preset.trim()
        : null;
    const providerConfigLabel =
      typeof provider.label === "string" && provider.label.trim().length > 0
        ? provider.label.trim()
        : null;
    const providerConfigVendorName =
      typeof provider.vendor_name === "string" && provider.vendor_name.trim().length > 0
        ? provider.vendor_name.trim()
        : typeof provider.vendorName === "string" && provider.vendorName.trim().length > 0
          ? provider.vendorName.trim()
          : null;
    // Every console surface reads the brand name, so a provider called
    // `jina-reader` or `Gemini Web /u/1/` in the config still shows up as the
    // site users recognize.
    const providerLabel = providerBrandLabel({
      providerId,
      providerPreset,
      providerLabel: providerConfigLabel,
      vendorName: providerConfigVendorName,
    });
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
