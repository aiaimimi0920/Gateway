import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createGatewayApiClient } from "../../api/client";
import { createConsoleApi, type ConsoleApi } from "../../api/console";
import { GatewayApiError } from "../../api/errors";
import type {
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialProbeResult,
  ConsoleCredentialProbeStatus,
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigValidationResponse,
  ConsoleRouteDocument,
  ConsoleSecretPatch,
  ConsoleRouteConfigResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
} from "../../api/contracts";
import { SecretConfirmDialog } from "../auth/SecretConfirmDialog";
import {
  CredentialDialog,
  type CredentialDialogMode,
  type CredentialDialogValue,
} from "./CredentialDialog";
import { DiscardDraftDialog } from "./DiscardDraftDialog";
import { RestoreRevisionDialog } from "./RestoreRevisionDialog";
import {
  addExplicitCredential,
  buildCredentialSecretPatches,
  deleteExplicitCredential,
  type CredentialSecretEdit,
  updateExplicitCredential,
} from "./credentialDocument";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type BrowserConsoleAppProps = {
  consoleApi?: ConsoleApi;
};

type SecretAccessRecovery = "not-required" | "recovered" | "stale";

type ConsoleActionRequest = {
  api: ConsoleApi;
  generation: number;
  managementToken: string;
  secretGrant: string | null;
  secretGrantEpoch: number;
};

function isSecretAccessRequiredError(error: unknown): error is GatewayApiError {
  return (
    error instanceof GatewayApiError &&
    error.status === 403 &&
    error.code === "console_secret_access_required"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return (
    isRecord(value) &&
    Object.values(value).every((entry) => typeof entry === "string")
  );
}

function isRouteDocument(value: unknown): value is ConsoleRouteDocument {
  return (
    isRecord(value) &&
    Array.isArray(value.providers) &&
    Array.isArray(value.model_routes) &&
    isStringRecord(value.aliases)
  );
}

function parseRouteDocument(text: string): ConsoleRouteDocument {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text) as unknown;
  } catch (error) {
    throw new Error(
      `Route document JSON is invalid: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (!isRouteDocument(parsed)) {
    throw new Error(
      "Route document JSON must be an object containing providers[], model_routes[], and aliases{}.",
    );
  }
  return parsed;
}

type SecretPatchDraft = {
  operation: ConsoleSecretPatch["operation"];
  value: string;
};

type AliasDraftRow = {
  id: string;
  alias: string;
  model: string;
};

type ModelRouteDraftRow = {
  id: string;
  pattern: string;
  route: Record<string, unknown>;
};

type ProviderDraftRow = {
  id: string;
  providerId: string;
  preset: string;
  vendorKey: string;
  vendorName: string;
  baseUrl: string;
  supportedModelsText: string;
  provider: Record<string, unknown>;
};

type CredentialDialogState = {
  mode: CredentialDialogMode;
  initialValue: CredentialDialogValue | null;
};

type RouteDocumentDiff = {
  activeAliasCount: number;
  selectedAliasCount: number;
  aliasChanges: Array<{
    alias: string;
    activeModel: string | null;
    selectedModel: string | null;
  }>;
  activeProviderCount: number;
  selectedProviderCount: number;
  addedProviders: string[];
  removedProviders: string[];
  activeModelRouteCount: number;
  selectedModelRouteCount: number;
  addedModelRoutes: string[];
  removedModelRoutes: string[];
  activeAccountGroupCount: number;
  selectedAccountGroupCount: number;
  addedAccountGroups: string[];
  removedAccountGroups: string[];
  changedAccountGroups: Array<{
    groupId: string;
    activeName: string;
    selectedName: string;
    activeDescription: string | null;
    selectedDescription: string | null;
    activeEnabled: boolean;
    selectedEnabled: boolean;
    activeNotes: string | null;
    selectedNotes: string | null;
    activeMembers: string[];
    selectedMembers: string[];
    activeBillingMultiplier: number;
    selectedBillingMultiplier: number;
  }>;
};

function createSecretPatchDrafts(
  routeConfig: ConsoleRouteConfigResponse | null,
): Record<string, SecretPatchDraft> {
  return Object.fromEntries(
    (routeConfig?.routeConfig.secrets ?? [])
      .filter((secret) => secret.path.trim().length > 0)
      .map((secret) => [
        secret.path,
        {
          operation: "keep" as const,
          value: "",
        },
      ]),
  );
}

function buildSecretPatches(
  routeConfig: ConsoleRouteConfigResponse | null,
  drafts: Record<string, SecretPatchDraft>,
): ConsoleSecretPatch[] {
  return (routeConfig?.routeConfig.secrets ?? [])
    .filter((secret) => secret.path.trim().length > 0)
    .map((secret) => {
      const draft = drafts[secret.path];
      if (draft?.operation === "replace") {
        return {
          path: secret.path,
          operation: "replace" as const,
          value: draft.value,
        };
      }
      if (draft?.operation === "clear") {
        return {
          path: secret.path,
          operation: "clear" as const,
        };
      }
      return {
        path: secret.path,
        operation: "keep" as const,
      };
    });
}

function filterSecretPatchesForDocument(
  patches: ConsoleSecretPatch[],
  document: ConsoleRouteDocument,
  allowedPaths?: Set<string>,
): ConsoleSecretPatch[] {
  return patches.filter((patch) => {
    if (allowedPaths && !allowedPaths.has(patch.path)) {
      return false;
    }
    const providerMatch = /^\/providers\/(\d+)(?:\/|$)/.exec(patch.path);
    if (!providerMatch) {
      return true;
    }
    return Number(providerMatch[1]) < document.providers.length;
  });
}

function summarizeSecretPatches(secretPatches: ConsoleSecretPatch[]): {
  keep: number;
  replace: number;
  clear: number;
} {
  return secretPatches.reduce(
    (summary, patch) => {
      summary[patch.operation] += 1;
      return summary;
    },
    { keep: 0, replace: 0, clear: 0 },
  );
}

function diagnosticsList(
  validation: ConsoleRouteConfigValidationResponse | null,
): Array<{ code: string; severity: string; path: string; message: string }> {
  return validation?.validation.diagnostics.diagnostics ?? [];
}

function routeProviderIds(document: ConsoleRouteDocument): string[] {
  return document.providers.map((provider, index) => {
    if (
      typeof provider === "object" &&
      provider !== null &&
      "id" in provider &&
      typeof provider.id === "string"
    ) {
      return provider.id;
    }
    return `provider-${index}`;
  });
}

function routeModelPatterns(document: ConsoleRouteDocument): string[] {
  return document.model_routes.map((route, index) => {
    if (
      typeof route === "object" &&
      route !== null &&
      "pattern" in route &&
      typeof route.pattern === "string"
    ) {
      return route.pattern;
    }
    return `route-${index}`;
  });
}

function normalizeAccountGroupMemberIds(memberIds: string[]): string[] {
  return [...new Set(memberIds.map((memberId) => memberId.trim()).filter(Boolean))].sort(
    (left, right) => left.localeCompare(right),
  );
}

function compareRouteDocuments(
  active: ConsoleRouteDocument,
  selected: ConsoleRouteDocument,
): RouteDocumentDiff {
  const aliasKeys = new Set([
    ...Object.keys(active.aliases),
    ...Object.keys(selected.aliases),
  ]);
  const activeProviders = routeProviderIds(active);
  const selectedProviders = routeProviderIds(selected);
  const activeRoutes = routeModelPatterns(active);
  const selectedRoutes = routeModelPatterns(selected);
  const activeGroups = routeAccountGroupsFromDocument(active);
  const selectedGroups = routeAccountGroupsFromDocument(selected);
  const activeGroupsById = new Map(activeGroups.map((group) => [group.id, group]));
  const selectedGroupsById = new Map(selectedGroups.map((group) => [group.id, group]));
  const groupIds = new Set([...activeGroupsById.keys(), ...selectedGroupsById.keys()]);

  return {
    activeAliasCount: Object.keys(active.aliases).length,
    selectedAliasCount: Object.keys(selected.aliases).length,
    aliasChanges: [...aliasKeys]
      .sort((left, right) => left.localeCompare(right))
      .map((alias) => ({
        alias,
        activeModel: active.aliases[alias] ?? null,
        selectedModel: selected.aliases[alias] ?? null,
      }))
      .filter((entry) => entry.activeModel !== entry.selectedModel),
    activeProviderCount: activeProviders.length,
    selectedProviderCount: selectedProviders.length,
    addedProviders: selectedProviders.filter((providerId) => !activeProviders.includes(providerId)),
    removedProviders: activeProviders.filter(
      (providerId) => !selectedProviders.includes(providerId),
    ),
    activeModelRouteCount: activeRoutes.length,
    selectedModelRouteCount: selectedRoutes.length,
    addedModelRoutes: selectedRoutes.filter((pattern) => !activeRoutes.includes(pattern)),
    removedModelRoutes: activeRoutes.filter((pattern) => !selectedRoutes.includes(pattern)),
    activeAccountGroupCount: activeGroups.length,
    selectedAccountGroupCount: selectedGroups.length,
    addedAccountGroups: selectedGroups
      .filter((group) => !activeGroupsById.has(group.id))
      .map((group) => group.name || group.id),
    removedAccountGroups: activeGroups
      .filter((group) => !selectedGroupsById.has(group.id))
      .map((group) => group.name || group.id),
    changedAccountGroups: [...groupIds]
      .sort((left, right) => left.localeCompare(right))
      .map((groupId) => {
        const activeGroup = activeGroupsById.get(groupId);
        const selectedGroup = selectedGroupsById.get(groupId);
        return {
          groupId,
          activeName: activeGroup?.name ?? "",
          selectedName: selectedGroup?.name ?? "",
          activeDescription: activeGroup?.description ?? null,
          selectedDescription: selectedGroup?.description ?? null,
          activeEnabled: activeGroup?.enabled ?? true,
          selectedEnabled: selectedGroup?.enabled ?? true,
          activeNotes: activeGroup?.notes ?? null,
          selectedNotes: selectedGroup?.notes ?? null,
          activeMembers: normalizeAccountGroupMemberIds(
            activeGroup?.providerCredentialIds ?? [],
          ),
          selectedMembers: normalizeAccountGroupMemberIds(
            selectedGroup?.providerCredentialIds ?? [],
          ),
          activeBillingMultiplier: activeGroup?.billingMultiplier ?? 1,
          selectedBillingMultiplier: selectedGroup?.billingMultiplier ?? 1,
        };
      })
      .filter(
        (entry) =>
          activeGroupsById.has(entry.groupId) &&
          selectedGroupsById.has(entry.groupId) &&
          (entry.activeName !== entry.selectedName ||
            entry.activeDescription !== entry.selectedDescription ||
            entry.activeEnabled !== entry.selectedEnabled ||
            entry.activeNotes !== entry.selectedNotes ||
            entry.activeBillingMultiplier !== entry.selectedBillingMultiplier ||
            entry.activeMembers.join(",") !== entry.selectedMembers.join(",")),
      ),
  };
}

function formatRouteDocument(document: ConsoleRouteDocument): string {
  return JSON.stringify(document, null, 2);
}

function createAliasDraftRow(alias = "", model = ""): AliasDraftRow {
  return {
    id: `alias-${Math.random().toString(36).slice(2, 10)}`,
    alias,
    model,
  };
}

function aliasDraftRowsFromDocument(document: ConsoleRouteDocument): AliasDraftRow[] {
  return Object.entries(document.aliases).map(([alias, model]) =>
    createAliasDraftRow(alias, model),
  );
}

function createModelRouteDraftRow(
  pattern = "",
  route: Record<string, unknown> = {},
): ModelRouteDraftRow {
  return {
    id: `route-${Math.random().toString(36).slice(2, 10)}`,
    pattern,
    route,
  };
}

function modelRouteDraftRowsFromDocument(document: ConsoleRouteDocument): ModelRouteDraftRow[] {
  return document.model_routes.map((route) => {
    if (isRecord(route)) {
      return createModelRouteDraftRow(
        typeof route.pattern === "string" ? route.pattern : "",
        route,
      );
    }
    return createModelRouteDraftRow();
  });
}

function createProviderDraftRow(
  providerId = "",
  preset = "",
  vendorKey = "",
  vendorName = "",
  baseUrl = "",
  supportedModelsText = "",
  provider: Record<string, unknown> = {},
): ProviderDraftRow {
  return {
    id: `provider-${Math.random().toString(36).slice(2, 10)}`,
    providerId,
    preset,
    vendorKey,
    vendorName,
    baseUrl,
    supportedModelsText,
    provider,
  };
}

function supportedModelsTextFromProvider(provider: Record<string, unknown>): string {
  const supportedModels = provider.supported_models;
  if (!Array.isArray(supportedModels)) {
    return "";
  }
  return supportedModels
    .filter((entry): entry is string => typeof entry === "string")
    .join("\n");
}

function parseSupportedModelsText(value: string): string[] {
  return value
    .split(/\r?\n|,/)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

function emptyCredentialDialogValue(providerId = ""): CredentialDialogValue {
  return {
    providerId,
    credentialId: "",
    accountName: "",
    enabled: true,
    baseUrl: "",
    supportedModelsText: "",
    apiKeyOperation: "replace",
    apiKeyValue: "",
  };
}

function credentialDialogValueFromDocument(
  document: ConsoleRouteDocument,
  providerId: string,
  credentialId: string,
): CredentialDialogValue | null {
  const provider = document.providers.find(
    (entry) => isRecord(entry) && entry.id === providerId,
  );
  if (!isRecord(provider) || !Array.isArray(provider.credentials)) {
    return null;
  }
  const credential = provider.credentials.find(
    (entry) => isRecord(entry) && entry.id === credentialId,
  );
  if (!isRecord(credential)) {
    return null;
  }
  return {
    providerId,
    credentialId,
    accountName: optionalString(credential, "account_name") ?? "",
    enabled: typeof credential.enabled === "boolean" ? credential.enabled : true,
    baseUrl: optionalString(credential, "base_url") ?? "",
    supportedModelsText: readStringArray(credential.supported_models).join("\n"),
    apiKeyOperation: "keep",
    apiKeyValue: "",
  };
}

function providerDraftRowsFromDocument(document: ConsoleRouteDocument): ProviderDraftRow[] {
  return document.providers.map((provider) => {
    if (isRecord(provider)) {
      return createProviderDraftRow(
        typeof provider.id === "string" ? provider.id : "",
        typeof provider.preset === "string" ? provider.preset : "",
        typeof provider.vendor_key === "string" ? provider.vendor_key : "",
        typeof provider.vendor_name === "string" ? provider.vendor_name : "",
        typeof provider.base_url === "string" ? provider.base_url : "",
        supportedModelsTextFromProvider(provider),
        provider,
      );
    }
    return createProviderDraftRow();
  });
}

type AccountGroupDraftRow = {
  id: string;
  groupId: string;
  name: string;
  description: string;
  billingMultiplier: string;
  enabled: boolean;
  notes: string;
  providerCredentialIds: string[];
};

function parseAccountGroupBillingMultiplier(value: string): number | null {
  const normalized = value.trim();
  if (normalized.length === 0) {
    return 1;
  }
  if (!/^(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(normalized)) {
    return null;
  }
  const parsed = Number(normalized);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function accountGroupDraftHasInput(row: AccountGroupDraftRow): boolean {
  return (
    row.groupId.trim().length > 0 ||
    row.name.trim().length > 0 ||
    row.description.trim().length > 0 ||
    row.notes.trim().length > 0 ||
    row.providerCredentialIds.length > 0
  );
}

function accountGroupDraftNeedsId(row: AccountGroupDraftRow): boolean {
  return accountGroupDraftHasInput(row) && row.groupId.trim().length === 0;
}

function documentHasIncompleteAccountGroup(document: ConsoleRouteDocument): boolean {
  const groupsValue = (document as Record<string, unknown>).account_groups;
  if (!Array.isArray(groupsValue)) {
    return false;
  }
  return groupsValue.some((entry) => {
    if (!isRecord(entry)) {
      return false;
    }
    const groupId = typeof entry.id === "string" ? entry.id.trim() : "";
    const hasInput =
      groupId.length > 0 ||
      (typeof entry.name === "string" && entry.name.trim().length > 0) ||
      (typeof entry.description === "string" && entry.description.trim().length > 0) ||
      (typeof entry.notes === "string" && entry.notes.trim().length > 0) ||
      (Array.isArray(entry.provider_credential_ids) && entry.provider_credential_ids.length > 0);
    return hasInput && groupId.length === 0;
  });
}

function documentHasInvalidAccountGroupBillingMultiplier(
  document: ConsoleRouteDocument,
): boolean {
  const groupsValue = (document as Record<string, unknown>).account_groups;
  if (!Array.isArray(groupsValue)) {
    return false;
  }
  return groupsValue.some(
    (entry) =>
      isRecord(entry) &&
      "billing_multiplier" in entry &&
      (typeof entry.billing_multiplier !== "number" ||
        !Number.isFinite(entry.billing_multiplier) ||
        entry.billing_multiplier < 0),
  );
}

type RouteAccountGroup = {
  id: string;
  name: string;
  description: string | null;
  billingMultiplier: number;
  enabled: boolean;
  notes: string | null;
  providerCredentialIds: string[];
};

type RouteManagedAccount = {
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

type RouteProviderBucket = {
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

type RouteAccountCatalog = {
  groups: RouteAccountGroup[];
  accounts: RouteManagedAccount[];
  providerBuckets: RouteProviderBucket[];
  ungroupedCount: number;
};

type AccountMembershipFilter = "all" | "grouped" | "ungrouped";
type AccountEnabledFilter = "all" | "enabled" | "disabled";

type CredentialProbeViewResult = ConsoleCredentialProbeResult | {
  credentialId: string;
  providerId: string;
  status: ConsoleCredentialProbeStatus | "error";
  message: string;
  checkedAt: string;
};

function providerDefaultAccountId(providerId: string): string {
  return `${providerId}::default`;
}

function normalizeBucketKey(value: string): string {
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

function optionalString(record: Record<string, unknown>, key: string): string | null {
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

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

function routeAccountGroupsFromDocument(document: ConsoleRouteDocument): RouteAccountGroup[] {
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

function createAccountGroupDraftRow(
  overrides: Partial<Omit<AccountGroupDraftRow, "id">> = {},
): AccountGroupDraftRow {
  return {
    id: `account-group-${Math.random().toString(36).slice(2, 10)}`,
    groupId: overrides.groupId ?? "",
    name: overrides.name ?? "",
    description: overrides.description ?? "",
    billingMultiplier: overrides.billingMultiplier ?? "1",
    enabled: overrides.enabled ?? true,
    notes: overrides.notes ?? "",
    providerCredentialIds: overrides.providerCredentialIds ?? [],
  };
}

function accountGroupDraftRowsFromDocument(
  document: ConsoleRouteDocument,
): AccountGroupDraftRow[] {
  return routeAccountGroupsFromDocument(document).map((group) =>
    createAccountGroupDraftRow({
      groupId: group.id,
      name: group.name,
      description: group.description ?? "",
      billingMultiplier: String(group.billingMultiplier),
      enabled: group.enabled,
      notes: group.notes ?? "",
      providerCredentialIds: group.providerCredentialIds,
    }),
  );
}

function buildRouteAccountCatalog(document: ConsoleRouteDocument): RouteAccountCatalog {
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

function buildRouteAccountCatalogFromSummary(
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
    .map((account) => {
      const provider = providersById.get(account.providerId);
      const baseUrl = account.baseUrl ?? provider?.baseUrl ?? null;
      const providerRecord = (provider ?? {}) as unknown as Record<string, unknown>;
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

function filterRouteAccountCatalog(
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

const ALIAS_EDITOR_JSON_ERROR = "Fix Route document JSON before using the structured alias editor.";
const MODEL_ROUTE_EDITOR_JSON_ERROR =
  "Fix Route document JSON before using the structured model route editor.";
const PROVIDER_EDITOR_JSON_ERROR =
  "Fix Route document JSON before using the structured provider editor.";
const ACCOUNT_GROUP_EDITOR_JSON_ERROR =
  "Fix Route document JSON before using the structured account-group editor.";
const ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH = "分组 ID 必须填写。";
const ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN = "Group ID is required.";
const ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH =
  "分组计费倍率必须是大于等于 0 的数字。";
const ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN =
  "Group billing multiplier must be a number greater than or equal to 0.";

type ConsoleWorkspaceId =
  | "overview"
  | "editor"
  | "providers"
  | "accounts"
  | "groups"
  | "secrets"
  | "revisions"
  | "advanced";

type ProviderSummaryCard = {
  id: string;
  preset: string | null;
  baseUrl: string | null;
  supportedModelsCount: number;
  credentialCount: number;
};

type ModelRouteSummaryCard = {
  pattern: string;
  priority: string | null;
  providerIds: string[];
};

function providerIdFromValue(provider: unknown, index: number): string {
  if (
    typeof provider === "object" &&
    provider !== null &&
    "id" in provider &&
    typeof provider.id === "string"
  ) {
    return provider.id;
  }
  return `provider-${index}`;
}

function hostLabelFromUrl(url: string | null): string | null {
  if (!url) {
    return null;
  }
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function reconcileSelectedRevision(
  current: ConsoleRouteRevisionDetailResponse | null,
  nextRouteConfig: ConsoleRouteConfigResponse,
  nextRevisions: ConsoleRouteRevisionListResponse,
): ConsoleRouteRevisionDetailResponse | null {
  if (!current) {
    return null;
  }

  const selectedRevisionId = current.routeConfig.revision.id;
  const nextListEntry = nextRevisions.revisions.find(
    (entry) => entry.revision.id === selectedRevisionId,
  );
  if (selectedRevisionId === nextRouteConfig.routeConfig.revision.id) {
    return {
      routeConfig: nextRouteConfig.routeConfig,
      active: nextListEntry?.active ?? true,
      hasArchive: nextListEntry?.hasArchive ?? current.hasArchive,
    };
  }

  return {
    ...current,
    routeConfig: nextListEntry
      ? { ...current.routeConfig, source: nextListEntry.source }
      : current.routeConfig,
    active: nextListEntry?.active ?? false,
    hasArchive: nextListEntry?.hasArchive ?? current.hasArchive,
  };
}

export function BrowserConsoleApp({ consoleApi }: BrowserConsoleAppProps) {
  const host = useGatewayHost();
  const session = useManagementSession();
  const { t } = useUiLocale();
  const translationRef = useRef(t);
  const refreshRequestIdRef = useRef(0);
  const consoleActionGenerationRef = useRef(0);
  const credentialProbeGenerationRef = useRef(0);
  translationRef.current = t;
  const client = useMemo(() => createGatewayApiClient({ host }), [host]);
  const api = useMemo(() => consoleApi ?? createConsoleApi(client), [client, consoleApi]);
  const managementToken = session.managementToken;
  const currentApiRef = useRef(api);
  const currentManagementTokenRef = useRef(managementToken);
  const currentSecretGrant = session.secretGrant?.grant ?? null;
  const observedSecretGrantRef = useRef(currentSecretGrant);
  const currentSecretGrantRef = useRef(currentSecretGrant);
  const currentSecretGrantEpochRef = useRef(0);
  currentApiRef.current = api;
  currentManagementTokenRef.current = managementToken;
  useLayoutEffect(() => {
    if (observedSecretGrantRef.current !== currentSecretGrant) {
      observedSecretGrantRef.current = currentSecretGrant;
      currentSecretGrantEpochRef.current += 1;
    }
    currentSecretGrantRef.current = currentSecretGrant;
  }, [currentSecretGrant]);
  const [routeConfig, setRouteConfig] = useState<ConsoleRouteConfigResponse | null>(null);
  const [accountGroupSummary, setAccountGroupSummary] =
    useState<ConsoleAccountGroupSummaryResponse["summary"] | null>(null);
  const [accountGroupSummaryError, setAccountGroupSummaryError] = useState<string | null>(null);
  const [revisions, setRevisions] = useState<ConsoleRouteRevisionListResponse | null>(null);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editorText, setEditorText] = useState("");
  const [commitMessage, setCommitMessage] = useState("");
  const [validation, setValidation] = useState<ConsoleRouteConfigValidationResponse | null>(null);
  const [actionBusy, setActionBusy] = useState<"validate" | "save" | null>(null);
  const [selectedRevision, setSelectedRevision] = useState<ConsoleRouteRevisionDetailResponse | null>(
    null,
  );
  const [revisionBusy, setRevisionBusy] = useState(false);
  const [secretDrafts, setSecretDrafts] = useState<Record<string, SecretPatchDraft>>({});
  const [credentialSecretEdits, setCredentialSecretEdits] = useState<CredentialSecretEdit[]>([]);
  const [secretDialogOpen, setSecretDialogOpen] = useState(false);
  const [credentialDialogState, setCredentialDialogState] =
    useState<CredentialDialogState | null>(null);
  const [restoreDialogOpen, setRestoreDialogOpen] = useState(false);
  const [discardRefreshDialogOpen, setDiscardRefreshDialogOpen] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [aliasDraftRows, setAliasDraftRows] = useState<AliasDraftRow[]>([]);
  const [modelRouteDraftRows, setModelRouteDraftRows] = useState<ModelRouteDraftRow[]>([]);
  const [providerDraftRows, setProviderDraftRows] = useState<ProviderDraftRow[]>([]);
  const [accountGroupDraftRows, setAccountGroupDraftRows] = useState<AccountGroupDraftRow[]>([]);
  const [accountGroupSearches, setAccountGroupSearches] = useState<Record<string, string>>({});
  const [activeWorkspace, setActiveWorkspace] = useState<ConsoleWorkspaceId>("overview");
  const [accountSearch, setAccountSearch] = useState("");
  const [accountMembershipFilter, setAccountMembershipFilter] =
    useState<AccountMembershipFilter>("all");
  const [accountEnabledFilter, setAccountEnabledFilter] = useState<AccountEnabledFilter>("all");
  const [credentialProbeBusy, setCredentialProbeBusy] = useState<string | null>(null);
  const [credentialProbeResults, setCredentialProbeResults] = useState<
    Record<string, CredentialProbeViewResult>
  >({});

  const beginConsoleActionRequest = useCallback(
    (requestManagementToken: string): ConsoleActionRequest => {
      const generation = consoleActionGenerationRef.current + 1;
      consoleActionGenerationRef.current = generation;
      return {
        api,
        generation,
        managementToken: requestManagementToken,
        secretGrant: session.secretGrant?.grant ?? null,
        secretGrantEpoch: currentSecretGrantEpochRef.current,
      };
    },
    [api, session.secretGrant?.grant],
  );

  const isConsoleActionIdentityCurrent = useCallback(
    (request: ConsoleActionRequest) =>
      request.generation === consoleActionGenerationRef.current &&
      request.api === currentApiRef.current &&
      request.managementToken === currentManagementTokenRef.current,
    [],
  );

  const isConsoleActionRequestCurrent = useCallback(
    (request: ConsoleActionRequest) =>
      isConsoleActionIdentityCurrent(request) &&
      request.secretGrantEpoch === currentSecretGrantEpochRef.current &&
      request.secretGrant === currentSecretGrantRef.current,
    [isConsoleActionIdentityCurrent],
  );

  const isConsoleActionRecoveryCurrent = useCallback(
    (request: ConsoleActionRequest) => {
      if (!isConsoleActionIdentityCurrent(request)) {
        return false;
      }
      const currentGrant = currentSecretGrantRef.current;
      const currentEpoch = currentSecretGrantEpochRef.current;
      return (
        (currentEpoch === request.secretGrantEpoch && currentGrant === request.secretGrant) ||
        (request.secretGrant !== null &&
          currentGrant === null &&
          currentEpoch === request.secretGrantEpoch + 1)
      );
    },
    [isConsoleActionIdentityCurrent],
  );

  const handleSecretAccessRequiredError = useCallback(
    (cause: unknown, isCurrentRequest: () => boolean): SecretAccessRecovery => {
      if (!isSecretAccessRequiredError(cause)) {
        return "not-required";
      }
      if (!isCurrentRequest()) {
        return "stale";
      }
      session.clearSecretGrant();
      credentialProbeGenerationRef.current += 1;
      setCredentialProbeBusy(null);
      setCredentialProbeResults((current) =>
        Object.keys(current).length === 0 ? current : {},
      );
      setSecretDialogOpen(true);
      return "recovered";
    },
    [session.clearSecretGrant],
  );

  const invalidateCredentialProbes = useCallback(() => {
    credentialProbeGenerationRef.current += 1;
    setCredentialProbeBusy(null);
    setCredentialProbeResults((current) =>
      Object.keys(current).length === 0 ? current : {},
    );
  }, []);

  const refresh = useCallback(async () => {
    if (!managementToken) {
      setError(
        translationRef.current(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        ),
      );
      setBusy(false);
      return;
    }
    const requestId = refreshRequestIdRef.current + 1;
    refreshRequestIdRef.current = requestId;
    setBusy(true);
    invalidateCredentialProbes();
    setError(null);
    try {
      const accountGroupSummaryPromise = api
        .getAccountGroupSummary(managementToken)
        .then((response) => ({ summary: response.summary, error: null as string | null }))
        .catch((cause: unknown) => ({
          summary: null,
          error: cause instanceof Error ? cause.message : String(cause),
        }));
      const [nextRouteConfig, nextRevisions, nextAccountGroupSummary] = await Promise.all([
        api.getRouteConfig(managementToken),
        api.listRouteConfigRevisions(managementToken),
        accountGroupSummaryPromise,
      ]);
      if (requestId !== refreshRequestIdRef.current) {
        return;
      }
      setValidation(null);
      setRouteConfig({ ...nextRouteConfig });
      setRevisions(nextRevisions);
      setAccountGroupSummary(nextAccountGroupSummary.summary);
      setAccountGroupSummaryError(nextAccountGroupSummary.error);
      setSelectedRevision((current) =>
        reconcileSelectedRevision(current, nextRouteConfig, nextRevisions),
      );
    } catch (cause) {
      if (requestId === refreshRequestIdRef.current) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally {
      if (requestId === refreshRequestIdRef.current) {
        setBusy(false);
      }
    }
  }, [api, invalidateCredentialProbes, managementToken]);

  useEffect(() => {
    if (!routeConfig) {
      return;
    }
    setEditorText(JSON.stringify(routeConfig.routeConfig.document, null, 2));
    setCommitMessage(routeConfig.routeConfig.revision.message ?? "");
    setValidation(null);
    setSecretDrafts(createSecretPatchDrafts(routeConfig));
    setCredentialSecretEdits([]);
    setCredentialDialogState(null);
    setCredentialProbeBusy(null);
    setCredentialProbeResults({});
    setAliasDraftRows(aliasDraftRowsFromDocument(routeConfig.routeConfig.document));
    setModelRouteDraftRows(modelRouteDraftRowsFromDocument(routeConfig.routeConfig.document));
    setProviderDraftRows(providerDraftRowsFromDocument(routeConfig.routeConfig.document));
    setAccountGroupDraftRows(accountGroupDraftRowsFromDocument(routeConfig.routeConfig.document));
  }, [routeConfig]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activeSecretPatches = useMemo(
    () => buildSecretPatches(routeConfig, secretDrafts),
    [routeConfig, secretDrafts],
  );
  const secretPatchDraftDocument = useMemo(() => {
    if (!routeConfig) {
      return null;
    }
    try {
      return editorText.trim().length > 0
        ? parseRouteDocument(editorText)
        : routeConfig.routeConfig.document;
    } catch {
      return routeConfig.routeConfig.document;
    }
  }, [editorText, routeConfig]);
  const secretPatches = useMemo(() => {
    if (!routeConfig || !secretPatchDraftDocument) {
      return activeSecretPatches;
    }
    return buildCredentialSecretPatches({
      activeDocument: routeConfig.routeConfig.document,
      draftDocument: secretPatchDraftDocument,
      activeSecrets: routeConfig.routeConfig.secrets,
      activeSecretPatches,
      credentialSecretEdits,
    });
  }, [activeSecretPatches, credentialSecretEdits, routeConfig, secretPatchDraftDocument]);
  const hasSecretAccess = Boolean(session.session?.secretAccessGranted);
  const selectedRevisionDiff = useMemo(() => {
    if (!routeConfig || !selectedRevision) {
      return null;
    }
    return compareRouteDocuments(
      routeConfig.routeConfig.document,
      selectedRevision.routeConfig.document,
    );
  }, [routeConfig, selectedRevision]);
  const activeRouteDocumentText = useMemo(
    () => (routeConfig ? formatRouteDocument(routeConfig.routeConfig.document) : ""),
    [routeConfig],
  );
  const selectedRouteDocumentText = useMemo(
    () =>
      selectedRevision ? formatRouteDocument(selectedRevision.routeConfig.document) : "",
    [selectedRevision],
  );
  const restoreCommitMessage = useMemo(
    () => (selectedRevision ? t(
      `恢复修订 ${selectedRevision.routeConfig.revision.id}`,
      `restore revision ${selectedRevision.routeConfig.revision.id}`,
    ) : ""),
    [selectedRevision, t],
  );
  const restoreSecretPatches = useMemo(() => {
    if (!selectedRevision) {
      return secretPatches;
    }
    const selectedSecretPaths = new Set(
      selectedRevision.routeConfig.secrets
        .map((secret) => secret.path.trim())
        .filter((path) => path.length > 0),
    );
    return filterSecretPatchesForDocument(
      secretPatches,
      selectedRevision.routeConfig.document,
      selectedSecretPaths,
    );
  }, [secretPatches, selectedRevision]);
  const secretPatchSummary = useMemo(
    () => summarizeSecretPatches(restoreDialogOpen ? restoreSecretPatches : secretPatches),
    [restoreDialogOpen, restoreSecretPatches, secretPatches],
  );
  const aliasChangeLines = useMemo(
    () =>
      (selectedRevisionDiff?.aliasChanges ?? []).map(
        (entry) =>
          `${entry.alias}: ${entry.activeModel ?? t("无", "<none>")} -> ${entry.selectedModel ?? t("无", "<none>")}`,
      ),
    [selectedRevisionDiff, t],
  );
  const providerChangeLines = useMemo(
    () => [
      ...(selectedRevisionDiff?.addedProviders ?? []).map((providerId) =>
        t(`新增 Provider：${providerId}`, `Added provider: ${providerId}`),
      ),
      ...(selectedRevisionDiff?.removedProviders ?? []).map(
        (providerId) => t(`移除 Provider：${providerId}`, `Removed provider: ${providerId}`),
      ),
    ],
    [selectedRevisionDiff, t],
  );
  const modelRouteChangeLines = useMemo(
    () => [
      ...(selectedRevisionDiff?.addedModelRoutes ?? []).map((pattern) =>
        t(`新增路由：${pattern}`, `Added route: ${pattern}`),
      ),
      ...(selectedRevisionDiff?.removedModelRoutes ?? []).map((pattern) =>
        t(`移除路由：${pattern}`, `Removed route: ${pattern}`),
      ),
    ],
    [selectedRevisionDiff, t],
  );
  const accountGroupChangeLines = useMemo(
    () => [
      ...(selectedRevisionDiff?.addedAccountGroups ?? []).map((groupName) =>
        t(`新增分组：${groupName}`, `Added group: ${groupName}`),
      ),
      ...(selectedRevisionDiff?.removedAccountGroups ?? []).map((groupName) =>
        t(`移除分组：${groupName}`, `Removed group: ${groupName}`),
      ),
      ...((selectedRevisionDiff?.changedAccountGroups ?? []).map((entry) =>
        t(
          `更新分组：${entry.groupId}，名称 ${entry.activeName || "无"} -> ${entry.selectedName || "无"}，描述 ${entry.activeDescription ?? "无"} -> ${entry.selectedDescription ?? "无"}，状态 ${entry.activeEnabled ? "启用" : "停用"} -> ${entry.selectedEnabled ? "启用" : "停用"}，备注 ${entry.activeNotes ?? "无"} -> ${entry.selectedNotes ?? "无"}，倍率 ${entry.activeBillingMultiplier} -> ${entry.selectedBillingMultiplier}，成员 ${entry.activeMembers.join(", ") || "无"} -> ${entry.selectedMembers.join(", ") || "无"}`,
          `Updated group: ${entry.groupId}, name ${entry.activeName || "<none>"} -> ${entry.selectedName || "<none>"}, description ${entry.activeDescription ?? "<none>"} -> ${entry.selectedDescription ?? "<none>"}, status ${entry.activeEnabled ? "enabled" : "disabled"} -> ${entry.selectedEnabled ? "enabled" : "disabled"}, notes ${entry.activeNotes ?? "<none>"} -> ${entry.selectedNotes ?? "<none>"}, multiplier ${entry.activeBillingMultiplier} -> ${entry.selectedBillingMultiplier}, members ${entry.activeMembers.join(", ") || "<none>"} -> ${entry.selectedMembers.join(", ") || "<none>"}`,
        ),
      )),
    ],
    [selectedRevisionDiff, t],
  );
  const activeAliasEntries = useMemo(
    () => Object.entries(routeConfig?.routeConfig.document.aliases ?? {}),
    [routeConfig],
  );
  const activeProviderSummaries = useMemo<ProviderSummaryCard[]>(
    () =>
      routeConfig?.routeConfig.document.providers.map((provider, index) => {
        const record = isRecord(provider) ? provider : {};
        return {
          id: providerIdFromValue(provider, index),
          preset: typeof record.preset === "string" ? record.preset : null,
          baseUrl: typeof record.base_url === "string" ? record.base_url : null,
          supportedModelsCount: Array.isArray(record.supported_models)
            ? record.supported_models.filter((entry) => typeof entry === "string").length
            : 0,
          credentialCount: Array.isArray(record.credentials) ? record.credentials.length : 0,
        };
      }) ?? [],
    [routeConfig],
  );
  const activeModelRouteSummaries = useMemo<ModelRouteSummaryCard[]>(
    () =>
      routeConfig?.routeConfig.document.model_routes.map((route, index) => {
        const record = isRecord(route) ? route : {};
        const providerIds = Array.isArray(record.provider_ids)
          ? record.provider_ids.filter((entry): entry is string => typeof entry === "string")
          : [];
        return {
          pattern:
            typeof record.pattern === "string" && record.pattern.trim().length > 0
              ? record.pattern
              : `route-${index}`,
          priority:
            typeof record.priority === "number"
              ? String(record.priority)
              : typeof record.priority === "string"
                ? record.priority
                : null,
          providerIds,
        };
      }) ?? [],
    [routeConfig],
  );
  const draftDocumentState = useMemo(() => {
    if (editorText.trim().length === 0) {
      return {
        document: routeConfig?.routeConfig.document ?? null,
        invalid: false,
      };
    }
    try {
      return {
        document: parseRouteDocument(editorText),
        invalid: false,
      };
    } catch {
      return {
        document: routeConfig?.routeConfig.document ?? null,
        invalid: true,
      };
    }
  }, [editorText, routeConfig]);
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
  const draftMatchesActiveRevision = useMemo(() => {
    if (!routeConfig || draftDocumentState.invalid) {
      return false;
    }
    if (editorText.trim().length === 0) {
      return true;
    }
    return editorText === formatRouteDocument(routeConfig.routeConfig.document);
  }, [draftDocumentState.invalid, editorText, routeConfig]);
  const incompleteAccountGroupDraftCount = useMemo(
    () => accountGroupDraftRows.filter(accountGroupDraftNeedsId).length,
    [accountGroupDraftRows],
  );
  const draftDirty = useMemo(() => {
    if (!routeConfig) {
      return false;
    }
    const activeDocumentText = formatRouteDocument(routeConfig.routeConfig.document);
    const documentChanged = editorText.trim().length > 0 && editorText !== activeDocumentText;
    const messageChanged =
      commitMessage.trim() !== (routeConfig.routeConfig.revision.message ?? "").trim();
    const secretChanged = secretPatches.some((patch) => patch.operation !== "keep");
    return (
      documentChanged ||
      messageChanged ||
      secretChanged ||
      incompleteAccountGroupDraftCount > 0
    );
  }, [commitMessage, editorText, incompleteAccountGroupDraftCount, routeConfig, secretPatches]);

  useEffect(() => {
    if (!draftDirty) {
      return;
    }
    const preventAccidentalNavigation = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", preventAccidentalNavigation);
    return () => window.removeEventListener("beforeunload", preventAccidentalNavigation);
  }, [draftDirty]);

  const handleManualRefresh = useCallback(() => {
    if (draftDirty) {
      setDiscardRefreshDialogOpen(true);
      return;
    }
    void refresh();
  }, [draftDirty, refresh]);

  const handleConfirmDiscardRefresh = useCallback(async () => {
    await refresh();
  }, [refresh]);

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
  const credentialProviderOptions = useMemo(
    () =>
      (draftDocumentState.document?.providers ?? []).flatMap((provider, index) => {
        if (!isRecord(provider)) {
          return [];
        }
        const id = providerIdFromValue(provider, index);
        return [
          {
            id,
            label: optionalString(provider, "label") ?? id,
          },
        ];
      }),
    [draftDocumentState.document],
  );
  const explicitCredentialIds = useMemo(
    () =>
      draftAccountCatalog.accounts
        .filter((account) => account.mode === "credential")
        .map((account) => account.id),
    [draftAccountCatalog.accounts],
  );
  const selectedRevisionProviderSummaries = useMemo<ProviderSummaryCard[]>(
    () =>
      selectedRevision?.routeConfig.document.providers.map((provider, index) => {
        const record = isRecord(provider) ? provider : {};
        return {
          id: providerIdFromValue(provider, index),
          preset: typeof record.preset === "string" ? record.preset : null,
          baseUrl: typeof record.base_url === "string" ? record.base_url : null,
          supportedModelsCount: Array.isArray(record.supported_models)
            ? record.supported_models.filter((entry) => typeof entry === "string").length
            : 0,
          credentialCount: Array.isArray(record.credentials) ? record.credentials.length : 0,
        };
      }) ?? [],
    [selectedRevision],
  );

  const buildCommitRequest = useCallback(
    (
      document: ConsoleRouteDocument,
      messageOverride?: string,
      secretPatchOverride?: ConsoleSecretPatch[],
    ): ConsoleRouteConfigCommitRequest => {
      if (!routeConfig) {
        throw new Error("Route configuration is not loaded yet.");
      }
      const message = messageOverride?.trim() || commitMessage.trim();
      const commitSecretPatches = secretPatchOverride ?? secretPatches;
      for (const patch of commitSecretPatches) {
        if (patch.operation !== "keep" && !hasSecretAccess) {
          throw new Error(
            t(
              "执行敏感字段替换或清空前，需要先确认敏感信息访问权限。",
              "Secret replacement or clearing requires confirmed secret access.",
            ),
          );
        }
        if (patch.operation === "replace" && (!patch.value || patch.value.trim().length === 0)) {
          throw new Error(
            t(
              `${patch.path} 的替换值不能为空。`,
              `Replacement secret for ${patch.path} cannot be empty.`,
            ),
          );
        }
      }
      return {
        expectedRevision: routeConfig.routeConfig.revision.id,
        document,
        secretPatches: commitSecretPatches,
        ...(message ? { message } : {}),
      };
    },
    [commitMessage, hasSecretAccess, routeConfig, secretPatches, t],
  );

  const parseDraft = useCallback((): ConsoleRouteConfigCommitRequest => {
    const document = parseRouteDocument(editorText);
    if (
      accountGroupDraftRows.some(accountGroupDraftNeedsId)
    ) {
      throw new Error(
        t(
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH,
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN,
        ),
      );
    }
    if (
      accountGroupDraftRows.some(
        (row) => parseAccountGroupBillingMultiplier(row.billingMultiplier) === null,
      ) ||
      documentHasInvalidAccountGroupBillingMultiplier(document)
    ) {
      throw new Error(
        t(
          ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH,
          ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN,
        ),
      );
    }
    if (documentHasIncompleteAccountGroup(document)) {
      throw new Error(
        t(
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH,
          ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN,
        ),
      );
    }
    return buildCommitRequest(document);
  }, [accountGroupDraftRows, buildCommitRequest, editorText, t]);

  const replaceEditorDocument = useCallback(
    (document: ConsoleRouteDocument, syncStructuredEditors = false) => {
      invalidateCredentialProbes();
      setEditorText(formatRouteDocument(document));
      setValidation(null);
      if (syncStructuredEditors) {
        setAliasDraftRows(aliasDraftRowsFromDocument(document));
        setModelRouteDraftRows(modelRouteDraftRowsFromDocument(document));
        setProviderDraftRows(providerDraftRowsFromDocument(document));
        setAccountGroupDraftRows(accountGroupDraftRowsFromDocument(document));
      }
    },
    [invalidateCredentialProbes],
  );

  const openAddCredentialDialog = useCallback((providerId = "") => {
    setCredentialDialogState({
      mode: "add",
      initialValue: emptyCredentialDialogValue(providerId),
    });
  }, []);

  const openEditCredentialDialog = useCallback(
    (providerId: string, credentialId: string) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      const initialValue = credentialDialogValueFromDocument(
        document,
        providerId,
        credentialId,
      );
      if (!initialValue) {
        setError(
          t(
            `找不到账号 ${credentialId}。`,
            `Account ${credentialId} could not be found.`,
          ),
        );
        return;
      }
      setCredentialDialogState({ mode: "edit", initialValue });
    },
    [editorText, t],
  );

  const updateCredentialSecretEdit = useCallback((value: CredentialDialogValue) => {
    setCredentialSecretEdits((current) => {
      const next = current.filter(
        (entry) =>
          !(
            entry.providerId === value.providerId &&
            entry.credentialId === value.credentialId &&
            entry.field === "api_key"
          ),
      );
      if (value.apiKeyOperation === "keep") {
        return next;
      }
      if (value.apiKeyOperation === "replace") {
        return [
          ...next,
          {
            providerId: value.providerId,
            credentialId: value.credentialId,
            field: "api_key",
            operation: "replace",
            value: value.apiKeyValue,
          },
        ];
      }
      return [
        ...next,
        {
          providerId: value.providerId,
          credentialId: value.credentialId,
          field: "api_key",
          operation: "clear",
        },
      ];
    });
  }, []);

  const applyCredentialDialogValue = useCallback(
    (value: CredentialDialogValue) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const supportedModels = parseSupportedModelsText(value.supportedModelsText);
      const fields: Record<string, unknown> = {
        account_name: value.accountName || undefined,
        enabled: value.enabled,
        base_url: value.baseUrl || undefined,
        supported_models: supportedModels.length > 0 ? supportedModels : undefined,
      };
      try {
        const nextDocument =
          credentialDialogState?.mode === "edit"
            ? updateExplicitCredential(
                document,
                {
                  providerId: value.providerId,
                  credentialId: value.credentialId,
                },
                fields,
              )
            : addExplicitCredential(document, {
                providerId: value.providerId,
                credential: {
                  id: value.credentialId,
                  ...fields,
                },
              });
        updateCredentialSecretEdit(value);
        replaceEditorDocument(nextDocument, true);
        setError(null);
        setSuccessMessage(
          t(
            `账号 ${value.accountName || value.credentialId} 已写入草稿，保存路由配置后生效。`,
            `Account ${value.accountName || value.credentialId} was added to the draft and will take effect after saving the route config.`,
          ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [
      credentialDialogState?.mode,
      editorText,
      replaceEditorDocument,
      t,
      updateCredentialSecretEdit,
    ],
  );

  const removeCredential = useCallback(
    (providerId: string, credentialId: string, displayName: string) => {
      if (
        !window.confirm(
          t(
            `确认从草稿中删除账号 ${displayName}（${credentialId}）？`,
            `Delete account ${displayName} (${credentialId}) from the draft?`,
          ),
        )
      ) {
        return;
      }
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      try {
        const nextDocument = deleteExplicitCredential(document, {
          providerId,
          credentialId,
        });
        setCredentialSecretEdits((current) =>
          current.filter(
            (entry) =>
              !(entry.providerId === providerId && entry.credentialId === credentialId),
          ),
        );
        replaceEditorDocument(nextDocument, true);
        setError(null);
        setSuccessMessage(
          t(
            `账号 ${displayName} 已从草稿删除，保存路由配置后生效。`,
            `Account ${displayName} was removed from the draft and will take effect after saving the route config.`,
          ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [editorText, replaceEditorDocument, t],
  );

  const handleCredentialProbe = useCallback(
    async (account: RouteManagedAccount) => {
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
      }
      if (!session.secretGrant?.grant) {
        setError(
          t(
            "测试账号前需要先授权敏感信息访问权限。",
            "Confirm secret access before probing an account.",
          ),
        );
        setSecretDialogOpen(true);
        return;
      }
      if (
        !account.enabled ||
        draftDirty ||
        !draftMatchesActiveRevision
      ) {
        return;
      }

      const probeGeneration = credentialProbeGenerationRef.current + 1;
      credentialProbeGenerationRef.current = probeGeneration;
      const probeApi = api;
      const probeManagementToken = managementToken;
      const probeSecretGrant = session.secretGrant.grant;
      const probeSecretGrantEpoch = currentSecretGrantEpochRef.current;
      const isCurrentProbeIdentity = () =>
        probeGeneration === credentialProbeGenerationRef.current &&
        probeApi === currentApiRef.current &&
        probeManagementToken === currentManagementTokenRef.current;
      const isCurrentProbe = () =>
        isCurrentProbeIdentity() &&
        probeSecretGrantEpoch === currentSecretGrantEpochRef.current &&
        probeSecretGrant === currentSecretGrantRef.current;
      const isCurrentProbeRecovery = () => {
        if (!isCurrentProbeIdentity()) {
          return false;
        }
        const currentGrant = currentSecretGrantRef.current;
        const currentEpoch = currentSecretGrantEpochRef.current;
        return (
          (currentEpoch === probeSecretGrantEpoch && currentGrant === probeSecretGrant) ||
          (currentGrant === null && currentEpoch === probeSecretGrantEpoch + 1)
        );
      };
      setCredentialProbeBusy(account.id);
      setError(null);
      try {
        const response = await probeApi.probeCredential(
          probeManagementToken,
          probeSecretGrant,
          account.id,
        );
        if (!isCurrentProbe()) {
          return;
        }
        setCredentialProbeResults((current) => ({
          ...current,
          [account.id]: response.result,
        }));
      } catch (cause) {
        const secretRecovery = handleSecretAccessRequiredError(cause, isCurrentProbeRecovery);
        if (secretRecovery === "stale") {
          return;
        }
        if (secretRecovery === "recovered") {
          return;
        }
        if (!isCurrentProbe()) {
          return;
        }
        setCredentialProbeResults((current) => ({
          ...current,
          [account.id]: {
            credentialId: account.id,
            providerId: account.providerId,
            status: "error",
            message: cause instanceof Error ? cause.message : String(cause),
            checkedAt: new Date().toISOString(),
          },
        }));
      } finally {
        if (probeGeneration === credentialProbeGenerationRef.current) {
          setCredentialProbeBusy(null);
        }
      }
    },
    [
      api,
      draftDirty,
      draftMatchesActiveRevision,
      handleSecretAccessRequiredError,
      managementToken,
      session.secretGrant,
      t,
    ],
  );

  const applyAliasDraftRows = useCallback(
    (nextRows: AliasDraftRow[]) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(ALIAS_EDITOR_JSON_ERROR);
        return;
      }
      setError((current) =>
        current === ALIAS_EDITOR_JSON_ERROR ? null : current,
      );
      const aliases = Object.fromEntries(
        nextRows
          .filter((row) => row.alias.trim().length > 0)
          .map((row) => [row.alias.trim(), row.model.trim()]),
      );
      document.aliases = aliases;
      setAliasDraftRows(nextRows);
      replaceEditorDocument(document);
    },
    [editorText, replaceEditorDocument],
  );

  const addAliasRow = useCallback(() => {
    applyAliasDraftRows([...aliasDraftRows, createAliasDraftRow()]);
  }, [aliasDraftRows, applyAliasDraftRows]);

  const updateAliasRow = useCallback(
    (rowId: string, field: "alias" | "model", value: string) => {
      applyAliasDraftRows(
        aliasDraftRows.map((row) => (row.id === rowId ? { ...row, [field]: value } : row)),
      );
    },
    [aliasDraftRows, applyAliasDraftRows],
  );

  const applyModelRouteDraftRows = useCallback(
    (nextRows: ModelRouteDraftRow[]) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(MODEL_ROUTE_EDITOR_JSON_ERROR);
        return;
      }
      setError((current) => (current === MODEL_ROUTE_EDITOR_JSON_ERROR ? null : current));
      document.model_routes = nextRows
        .filter((row) => row.pattern.trim().length > 0)
        .map((row) => ({
          ...row.route,
          pattern: row.pattern.trim(),
        }));
      setModelRouteDraftRows(nextRows);
      replaceEditorDocument(document);
    },
    [editorText, replaceEditorDocument],
  );

  const addModelRouteRow = useCallback(() => {
    applyModelRouteDraftRows([...modelRouteDraftRows, createModelRouteDraftRow()]);
  }, [applyModelRouteDraftRows, modelRouteDraftRows]);

  const applyProviderDraftRows = useCallback(
    (nextRows: ProviderDraftRow[]) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(PROVIDER_EDITOR_JSON_ERROR);
        return;
      }
      setError((current) => (current === PROVIDER_EDITOR_JSON_ERROR ? null : current));
      document.providers = nextRows
        .filter((row) => row.providerId.trim().length > 0)
        .map((row) => {
          const nextProvider: Record<string, unknown> = {
            ...row.provider,
            id: row.providerId.trim(),
          };
          const trimmedPreset = row.preset.trim();
          if (trimmedPreset.length > 0) {
            nextProvider.preset = trimmedPreset;
          } else {
            delete nextProvider.preset;
          }
          const trimmedVendorKey = row.vendorKey.trim();
          if (trimmedVendorKey.length > 0) {
            nextProvider.vendor_key = trimmedVendorKey;
          } else {
            delete nextProvider.vendor_key;
          }
          const trimmedVendorName = row.vendorName.trim();
          if (trimmedVendorName.length > 0) {
            nextProvider.vendor_name = trimmedVendorName;
          } else {
            delete nextProvider.vendor_name;
          }
          const trimmedBaseUrl = row.baseUrl.trim();
          if (trimmedBaseUrl.length > 0) {
            nextProvider.base_url = trimmedBaseUrl;
          } else {
            delete nextProvider.base_url;
          }
          const supportedModels = parseSupportedModelsText(row.supportedModelsText);
          if (supportedModels.length > 0) {
            nextProvider.supported_models = supportedModels;
          } else {
            delete nextProvider.supported_models;
          }
          return nextProvider;
        });
      setProviderDraftRows(nextRows);
      replaceEditorDocument(document);
    },
    [editorText, replaceEditorDocument],
  );

  const addProviderRow = useCallback(() => {
    applyProviderDraftRows([...providerDraftRows, createProviderDraftRow()]);
  }, [applyProviderDraftRows, providerDraftRows]);

  const updateProviderRow = useCallback(
    (
      rowId: string,
      field:
        | "providerId"
        | "preset"
        | "vendorKey"
        | "vendorName"
        | "baseUrl"
        | "supportedModelsText",
      value: string,
    ) => {
      applyProviderDraftRows(
        providerDraftRows.map((row) => (row.id === rowId ? { ...row, [field]: value } : row)),
      );
    },
    [applyProviderDraftRows, providerDraftRows],
  );

  const removeProviderRow = useCallback(
    (rowId: string) => {
      applyProviderDraftRows(providerDraftRows.filter((row) => row.id !== rowId));
    },
    [applyProviderDraftRows, providerDraftRows],
  );

  const updateModelRouteRow = useCallback(
    (rowId: string, value: string) => {
      applyModelRouteDraftRows(
        modelRouteDraftRows.map((row) => (row.id === rowId ? { ...row, pattern: value } : row)),
      );
    },
    [applyModelRouteDraftRows, modelRouteDraftRows],
  );

  const removeModelRouteRow = useCallback(
    (rowId: string) => {
      applyModelRouteDraftRows(modelRouteDraftRows.filter((row) => row.id !== rowId));
    },
    [applyModelRouteDraftRows, modelRouteDraftRows],
  );

  const removeAliasRow = useCallback(
    (rowId: string) => {
      applyAliasDraftRows(aliasDraftRows.filter((row) => row.id !== rowId));
    },
    [aliasDraftRows, applyAliasDraftRows],
  );

  const applyAccountGroupDraftRows = useCallback(
    (nextRows: AccountGroupDraftRow[]) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(ACCOUNT_GROUP_EDITOR_JSON_ERROR);
        return;
      }
      setError((current) =>
        current === ACCOUNT_GROUP_EDITOR_JSON_ERROR ||
        current === ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH ||
        current === ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN ||
        current === ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH ||
        current === ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN
          ? null
          : current,
      );

      const nextGroups = [];
      for (const row of nextRows) {
        if (!accountGroupDraftHasInput(row)) {
          continue;
        }
        if (accountGroupDraftNeedsId(row)) {
          setAccountGroupDraftRows(nextRows);
          setError(t(ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH, ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN));
          return;
        }
        const billingMultiplier = parseAccountGroupBillingMultiplier(row.billingMultiplier);
        if (billingMultiplier === null) {
          setAccountGroupDraftRows(nextRows);
          setError(
            t(
              ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH,
              ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN,
            ),
          );
          return;
        }

        nextGroups.push({
          id: row.groupId.trim(),
          name: row.name.trim(),
          billing_multiplier: billingMultiplier,
          enabled: row.enabled,
          ...(row.description.trim().length > 0 ? { description: row.description.trim() } : {}),
          ...(row.notes.trim().length > 0 ? { notes: row.notes.trim() } : {}),
          provider_credential_ids: [...new Set(row.providerCredentialIds)],
        });
      }

      if (nextGroups.length > 0) {
        (document as Record<string, unknown>).account_groups = nextGroups;
      } else {
        delete (document as Record<string, unknown>).account_groups;
      }
      setAccountGroupDraftRows(nextRows);
      replaceEditorDocument(document);
    },
    [editorText, replaceEditorDocument, t],
  );

  const addAccountGroupRow = useCallback(() => {
    applyAccountGroupDraftRows([...accountGroupDraftRows, createAccountGroupDraftRow()]);
  }, [accountGroupDraftRows, applyAccountGroupDraftRows]);

  const updateAccountGroupRow = useCallback(
    (
      rowId: string,
      field: "groupId" | "name" | "description" | "billingMultiplier" | "notes",
      value: string,
    ) => {
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => (row.id === rowId ? { ...row, [field]: value } : row)),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const updateAccountGroupEnabled = useCallback(
    (rowId: string, enabled: boolean) => {
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => (row.id === rowId ? { ...row, enabled } : row)),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const toggleAccountGroupMember = useCallback(
    (rowId: string, providerCredentialId: string) => {
      applyAccountGroupDraftRows(
        accountGroupDraftRows.map((row) => {
          if (row.id !== rowId) {
            return row;
          }
          const exists = row.providerCredentialIds.includes(providerCredentialId);
          return {
            ...row,
            providerCredentialIds: exists
              ? row.providerCredentialIds.filter((value) => value !== providerCredentialId)
              : [...row.providerCredentialIds, providerCredentialId],
          };
        }),
      );
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const removeAccountGroupRow = useCallback(
    (rowId: string) => {
      applyAccountGroupDraftRows(accountGroupDraftRows.filter((row) => row.id !== rowId));
    },
    [accountGroupDraftRows, applyAccountGroupDraftRows],
  );

  const handleValidate = useCallback(async () => {
    if (!managementToken) {
      setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
      return;
    }
    const actionRequest = beginConsoleActionRequest(managementToken);
    setActionBusy("validate");
    setError(null);
    setSuccessMessage(null);
    try {
      const draft = parseDraft();
      const validationRequest = {
        document: draft.document,
        secretPatches: draft.secretPatches,
      };
      const result = actionRequest.secretGrant
        ? await actionRequest.api.validateRouteConfig(
            actionRequest.managementToken,
            validationRequest,
            actionRequest.secretGrant,
          )
        : await actionRequest.api.validateRouteConfig(
            actionRequest.managementToken,
            validationRequest,
          );
      if (!isConsoleActionRequestCurrent(actionRequest)) {
        return;
      }
      setValidation(result);
    } catch (cause) {
      const secretRecovery = handleSecretAccessRequiredError(
        cause,
        () => isConsoleActionRecoveryCurrent(actionRequest),
      );
      if (secretRecovery === "stale") {
        return;
      }
      if (secretRecovery === "recovered") {
        setValidation(null);
        return;
      }
      if (!isConsoleActionRequestCurrent(actionRequest)) {
        return;
      }
      setValidation(null);
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (actionRequest.generation === consoleActionGenerationRef.current) {
        setActionBusy(null);
      }
    }
  }, [
    beginConsoleActionRequest,
    handleSecretAccessRequiredError,
    isConsoleActionRecoveryCurrent,
    isConsoleActionRequestCurrent,
    managementToken,
    parseDraft,
    t,
  ]);

  const handleSave = useCallback(async () => {
    if (!managementToken) {
      setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
      return;
    }
    const actionRequest = beginConsoleActionRequest(managementToken);
    setActionBusy("save");
    setError(null);
    setSuccessMessage(null);
    try {
      const draft = parseDraft();
      const result = actionRequest.secretGrant
        ? await actionRequest.api.commitRouteConfig(
            actionRequest.managementToken,
            draft,
            actionRequest.secretGrant,
          )
        : await actionRequest.api.commitRouteConfig(actionRequest.managementToken, draft);
      if (!isConsoleActionRequestCurrent(actionRequest)) {
        return;
      }
      setRouteConfig({ routeConfig: result.routeConfig });
      setValidation(null);
      setSuccessMessage(
        t(
          `已将路由配置保存为激活修订 ${result.routeConfig.revision.id}。`,
          `Saved route config as active revision ${result.routeConfig.revision.id}.`,
        ),
      );
      await refresh();
    } catch (cause) {
      const secretRecovery = handleSecretAccessRequiredError(
        cause,
        () => isConsoleActionRecoveryCurrent(actionRequest),
      );
      if (secretRecovery !== "not-required") {
        return;
      }
      if (!isConsoleActionRequestCurrent(actionRequest)) {
        return;
      }
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (actionRequest.generation === consoleActionGenerationRef.current) {
        setActionBusy(null);
      }
    }
  }, [
    beginConsoleActionRequest,
    handleSecretAccessRequiredError,
    isConsoleActionRecoveryCurrent,
    isConsoleActionRequestCurrent,
    managementToken,
    parseDraft,
    refresh,
    t,
  ]);

  const handleInspectRevision = useCallback(
    async (revisionId: string) => {
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
      }
      setRevisionBusy(true);
      setError(null);
      setSuccessMessage(null);
      try {
        const detail = await api.getRouteConfigRevision(managementToken, revisionId);
        setSelectedRevision(detail);
        setActiveWorkspace("revisions");
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setRevisionBusy(false);
      }
    },
    [api, managementToken, t],
  );

  const loadSelectedRevisionIntoEditor = useCallback(() => {
    if (!selectedRevision) {
      return;
    }
    replaceEditorDocument(selectedRevision.routeConfig.document, true);
    setCommitMessage(selectedRevision.routeConfig.revision.message ?? "");
  }, [replaceEditorDocument, selectedRevision]);

  const handleRestoreSelectedRevision = useCallback(async () => {
    if (!managementToken) {
      setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
      return;
    }
    if (!selectedRevision) {
      setError(t("请先选择一个修订，再执行恢复。", "Select a revision before restoring it."));
      return;
    }
    const actionRequest = beginConsoleActionRequest(managementToken);
    setActionBusy("save");
    setError(null);
    setSuccessMessage(null);
    try {
      const draft = buildCommitRequest(
        selectedRevision.routeConfig.document,
        restoreCommitMessage,
        restoreSecretPatches,
      );
      const result = actionRequest.secretGrant
        ? await actionRequest.api.commitRouteConfig(
            actionRequest.managementToken,
            draft,
            actionRequest.secretGrant,
          )
        : await actionRequest.api.commitRouteConfig(actionRequest.managementToken, draft);
      if (!isConsoleActionRequestCurrent(actionRequest)) {
        return;
      }
      setRouteConfig({ routeConfig: result.routeConfig });
      setValidation(null);
      replaceEditorDocument(selectedRevision.routeConfig.document, true);
      setCommitMessage(restoreCommitMessage);
      setSuccessMessage(
        t(
          `已将修订 ${selectedRevision.routeConfig.revision.id} 恢复为激活修订 ${result.routeConfig.revision.id}。`,
          `Restored revision ${selectedRevision.routeConfig.revision.id} as active revision ${result.routeConfig.revision.id}.`,
        ),
      );
      setRestoreDialogOpen(false);
      await refresh();
    } catch (cause) {
      const secretRecovery = handleSecretAccessRequiredError(
        cause,
        () => isConsoleActionRecoveryCurrent(actionRequest),
      );
      if (secretRecovery !== "not-required") {
        return;
      }
      if (!isConsoleActionRequestCurrent(actionRequest)) {
        return;
      }
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (actionRequest.generation === consoleActionGenerationRef.current) {
        setActionBusy(null);
      }
    }
  }, [
    beginConsoleActionRequest,
    buildCommitRequest,
    handleSecretAccessRequiredError,
    isConsoleActionRecoveryCurrent,
    isConsoleActionRequestCurrent,
    managementToken,
    refresh,
    replaceEditorDocument,
    restoreCommitMessage,
    restoreSecretPatches,
    selectedRevision,
    t,
  ]);

  const validationDiagnostics = diagnosticsList(validation);
  const activeRouteDiagnostics = routeConfig?.routeConfig.diagnostics?.diagnostics ?? [];
  const mutationSupported = routeConfig?.routeConfig.mutationSupported ?? false;
  const editorLocked = busy || actionBusy !== null || !mutationSupported;
  const countText = (
    count: number,
    chineseUnit: string,
    englishSingular: string,
    englishPlural: string,
  ) => `${count} ${t(chineseUnit, count === 1 ? englishSingular : englishPlural)}`;
  const workspaceItems = [
    {
      id: "overview" as const,
      label: t("总览", "Overview"),
      eyebrow: t("控制中心", "Control center"),
      meta: countText(activeProviderSummaries.length, "Provider", "provider", "providers"),
    },
    {
      id: "editor" as const,
      label: t("路由编辑", "Route editor"),
      eyebrow: t("结构化编辑", "Structured editing"),
      meta: countText(aliasDraftRows.length, "Alias", "alias", "aliases"),
    },
    {
      id: "providers" as const,
      label: t("Provider 资源", "Provider hub"),
      eyebrow: t("上游资源", "Upstream resources"),
      meta: countText(activeProviderSummaries.length, "项", "item", "items"),
    },
    {
      id: "accounts" as const,
      label: t("账号台账", "Accounts"),
      eyebrow: t("凭据池", "Account pool"),
      meta: countText(displayedAccountCatalog.accounts.length, "账号", "account", "accounts"),
    },
    {
      id: "groups" as const,
      label: t("分组策略", "Groups"),
      eyebrow: t("费率与隔离", "Billing and isolation"),
      meta: countText(displayedAccountCatalog.groups.length, "组", "group", "groups"),
    },
    {
      id: "secrets" as const,
      label: t("敏感信息", "Secrets"),
      eyebrow: t("密钥与凭据", "Credentials"),
      meta: countText(secretPatches.length, "项", "item", "items"),
    },
    {
      id: "revisions" as const,
      label: t("修订历史", "Revisions"),
      eyebrow: t("版本回滚", "History"),
      meta: countText(revisions?.revisions.length ?? 0, "条", "revision", "revisions"),
    },
    {
      id: "advanced" as const,
      label: t("高级 JSON", "Advanced JSON"),
      eyebrow: t("原始文档", "Raw document"),
      meta: t("手动修订", "Manual patching"),
    },
  ];
  const validationFeedback = validation ? (
    validationDiagnostics.length > 0 ? (
      <div className="nt-validation-list" role="alert">
        <strong>{t("草稿校验返回诊断结果", "Draft validation reported diagnostics")}</strong>
        <ul>
          {validationDiagnostics.map((item) => (
            <li key={`${item.code}:${item.path}:${item.message}`}>
              [{item.severity}] {item.path} · {item.message}
            </li>
          ))}
        </ul>
      </div>
    ) : (
      <div className="nt-validation-list nt-validation-list--warning">
        <strong>{t("草稿校验通过", "Draft validation passed")}</strong>
        <ul>
          <li>
            {t(
              "候选修订在结构上有效，并会通过 keep patch 保留已配置的敏感字段。",
              "Candidate revision is structurally valid and preserves configured secrets via keep patches.",
            )}
          </li>
        </ul>
      </div>
    )
  ) : null;
  const activeDiagnosticsFeedback = activeRouteDiagnostics.length > 0 ? (
    <div
      className="nt-alert nt-alert--danger nt-diagnostics-report"
      role="alert"
      aria-label={t("Active route diagnostics", "Active route diagnostics")}
    >
      <div>
        <strong>
          {t(
            `当前激活路由有 ${activeRouteDiagnostics.length} 条需要处理的诊断`,
            `${activeRouteDiagnostics.length} active route diagnostic(s) require attention`,
          )}
        </strong>
        <ul>
          {activeRouteDiagnostics.map((diagnostic) => (
            <li key={`${diagnostic.code}:${diagnostic.path}:${diagnostic.message}`}>
              <code>{diagnostic.code}</code> · <code>{diagnostic.path}</code> · {diagnostic.message}
            </li>
          ))}
        </ul>
        <p className="nt-copy">
          {t(
            "请先在高级 JSON 中修复这些问题，再保存新的路由修订。",
            "Fix these issues in Advanced JSON before saving a new route revision.",
          )}
        </p>
      </div>
      <button className="nt-btn nt-btn--outline" type="button" onClick={() => setActiveWorkspace("advanced")}>
        {t("打开高级 JSON", "Open Advanced JSON")}
      </button>
    </div>
  ) : null;
  const accountSummaryFeedback = accountGroupSummaryError ? (
    <div
      className="nt-alert nt-alert--warning"
      role="status"
      aria-label={t("Account summary status", "Account summary status")}
    >
      <span>
        {t(
          `后端账号汇总暂不可用，当前显示本地可解析快照：${accountGroupSummaryError}`,
          `Backend account summary unavailable; showing the locally parseable snapshot: ${accountGroupSummaryError}`,
        )}
      </span>
    </div>
  ) : null;
  const draftStructureNotice = draftDocumentState.invalid ? (
    <div className="nt-validation-list nt-validation-list--warning">
      <strong>{t("当前 JSON 草稿不可解析", "Current JSON draft is invalid")}</strong>
      <ul>
        <li>
          {t(
            "账号与分组视图当前回退到上一份可解析快照；修复高级 JSON 后，这里的结构化视图会自动重新同步。",
            "Accounts and groups currently fall back to the last parseable snapshot. Once Advanced JSON is fixed, the structured views here will resync automatically.",
          )}
        </li>
      </ul>
    </div>
  ) : null;
  const secretsWorkspace = (
    <article className="nt-card nt-card--panel">
      <div className="nt-section__head">
        <div>
          <p className="nt-kicker">// Secrets</p>
          <h2>{t("敏感信息与保留策略", "Secrets and preservation")}</h2>
        </div>
      </div>
      <div className="nt-copy" aria-label="Route editor secret handling">
        {t(
          "当前会自动为已存在的敏感字段生成 keep patch，避免浏览器编辑时覆盖现有密钥。",
          "Existing secret fields automatically receive keep patches so browser edits do not overwrite current credentials.",
        )}
      </div>
      {secretPatches.length > 0 ? (
        <div className="nt-scroll-panel">
          <ul className="nt-simple-list">
            {routeConfig?.routeConfig.secrets.map((secret) => {
              const draft = secretDrafts[secret.path] ?? {
                operation: "keep" as const,
                value: "",
              };
              return (
                <li key={secret.path}>
                  <div className="nt-stack">
                    <div>
                      <strong>{secret.path}</strong>
                      <span>
                        {secret.preview ?? (secret.configured ? "configured" : "not configured")}
                      </span>
                    </div>
                    <div className="nt-actions">
                      <button
                        className={`nt-btn${draft.operation === "keep" ? " nt-btn--primary" : " nt-btn--outline"}`}
                        type="button"
                        aria-pressed={draft.operation === "keep"}
                        disabled={editorLocked}
                        onClick={() =>
                          setSecretDrafts((current) => ({
                            ...current,
                            [secret.path]: { operation: "keep", value: "" },
                          }))
                        }
                      >
                        {t(`保留 ${secret.path}`, `Keep ${secret.path}`)}
                      </button>
                      <button
                        className={`nt-btn${draft.operation === "replace" ? " nt-btn--primary" : " nt-btn--outline"}`}
                        type="button"
                        aria-pressed={draft.operation === "replace"}
                        disabled={!hasSecretAccess || editorLocked}
                        onClick={() =>
                          setSecretDrafts((current) => ({
                            ...current,
                            [secret.path]: {
                              operation: "replace",
                              value: current[secret.path]?.value ?? "",
                            },
                          }))
                        }
                      >
                        {t(`替换 ${secret.path}`, `Replace ${secret.path}`)}
                      </button>
                      <button
                        className={`nt-btn${draft.operation === "clear" ? " nt-btn--primary" : " nt-btn--outline"}`}
                        type="button"
                        aria-pressed={draft.operation === "clear"}
                        disabled={!hasSecretAccess || editorLocked}
                        onClick={() =>
                          setSecretDrafts((current) => ({
                            ...current,
                            [secret.path]: { operation: "clear", value: "" },
                          }))
                        }
                      >
                        {t(`清空 ${secret.path}`, `Clear ${secret.path}`)}
                      </button>
                    </div>
                    {draft.operation === "replace" ? (
                      <label className="nt-field nt-field--wide">
                        <span>{t(`${secret.path} 的替换值`, `Replacement for ${secret.path}`)}</span>
                        <input
                          className="nt-input"
                          type="password"
                          autoComplete="off"
                          disabled={editorLocked}
                          value={draft.value}
                          onChange={(event) => {
                            const { value } = event.currentTarget;
                            setSecretDrafts((current) => ({
                              ...current,
                              [secret.path]: {
                                operation: "replace",
                                value,
                              },
                            }));
                          }}
                        />
                      </label>
                    ) : null}
                    <span>
                      {t(
                        draft.operation === "keep"
                          ? "保留"
                          : draft.operation === "replace"
                            ? "替换"
                            : "清空",
                        draft.operation,
                      )}
                    </span>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      ) : (
        <p className="nt-empty">
          {t(
            "当前配置没有需要保留的已登记敏感字段。",
            "There are no registered secret fields that need to be preserved.",
          )}
        </p>
      )}
      {secretPatches.length > 0 ? (
        hasSecretAccess ? (
          <div className="nt-validation-list nt-validation-list--warning">
            <strong>{t("敏感信息访问已激活", "Secret access active")}</strong>
            <ul>
              <li>
                {t(
                  "当前会话已获得短时 secret grant，可执行替换 / 清空操作。",
                  "This session has a short-lived secret grant and can perform replace / clear operations.",
                )}
              </li>
              <li>
                {t("授权过期时间", "Grant expires at")}:{" "}
                {session.secretGrant?.expiresAt ?? t("未知", "unknown")}
              </li>
            </ul>
          </div>
        ) : (
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setSecretDialogOpen(true)}
            >
              {t("确认敏感信息访问权限", "Confirm secret access")}
            </button>
          </div>
        )
      ) : null}
    </article>
  );
  const revisionsWorkspace = (
    <div className="nt-grid nt-grid--2">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Revision history</p>
            <h2>{t("修订历史", "Revision history")}</h2>
          </div>
        </div>
        <div className="nt-scroll-panel">
          <ul className="nt-simple-list">
            {revisions?.revisions.map((entry) => (
              <li key={entry.revision.id}>
                <div>
                  <strong>{entry.revision.id}</strong>
                  <span>{entry.revision.message ?? t("无说明", "no message")}</span>
                </div>
                <div className="nt-actions nt-actions--right">
                  <span>{entry.active ? t("当前激活", "ACTIVE") : entry.source}</span>
                  <button
                    className="nt-btn nt-btn--outline"
                    type="button"
                    disabled={revisionBusy}
                    onClick={() => void handleInspectRevision(entry.revision.id)}
                  >
                    {t(`查看修订 ${entry.revision.id}`, `Inspect revision ${entry.revision.id}`)}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </article>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Revision detail</p>
            <h2>{t("修订详情", "Revision detail")}</h2>
          </div>
        </div>
        {selectedRevision ? (
          <div className="nt-stack" aria-label="Selected revision detail">
            <dl className="nt-meta-list nt-meta-list--inline">
              <div>
                <dt>{t("修订号", "Revision")}</dt>
                <dd>{selectedRevision.routeConfig.revision.id}</dd>
              </div>
              <div>
                <dt>{t("来源", "Source")}</dt>
                <dd>{selectedRevision.routeConfig.source}</dd>
              </div>
              <div>
                <dt>{t("状态", "Status")}</dt>
                <dd>{selectedRevision.active ? t("激活中", "active") : t("已归档", "archived")}</dd>
              </div>
              <div>
                <dt>{t("归档", "Archive")}</dt>
                <dd>{selectedRevision.hasArchive ? t("可用", "available") : t("未存储", "not stored")}</dd>
              </div>
            </dl>

            <h3>{t("Providers", "Providers")}</h3>
            <div className="nt-console-preview-grid">
              {selectedRevisionProviderSummaries.map((provider) => (
                <article className="nt-card nt-card--stat nt-console-mini-card" key={provider.id}>
                  <span>{provider.preset ?? t("未设置 preset", "No preset")}</span>
                  <strong>{provider.id}</strong>
                  <small>
                    {hostLabelFromUrl(provider.baseUrl) ?? t("未配置地址", "No base URL")}
                  </small>
                </article>
              ))}
            </div>

            {selectedRevisionDiff ? (
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>{t("修订差异摘要", "Revision diff summary")}</strong>
                <ul>
                  <li>
                    {t("当前 Alias 数", "Active aliases")}: {selectedRevisionDiff.activeAliasCount}
                  </li>
                  <li>
                    {t("选中 Alias 数", "Selected aliases")}: {selectedRevisionDiff.selectedAliasCount}
                  </li>
                  <li>
                    {t("当前 Provider 数", "Active providers")}:{" "}
                    {selectedRevisionDiff.activeProviderCount}
                  </li>
                  <li>
                    {t("选中 Provider 数", "Selected providers")}:{" "}
                    {selectedRevisionDiff.selectedProviderCount}
                  </li>
                  <li>
                    {t("当前模型路由数", "Active model routes")}:{" "}
                    {selectedRevisionDiff.activeModelRouteCount}
                  </li>
                  <li>
                    {t("选中模型路由数", "Selected model routes")}:{" "}
                    {selectedRevisionDiff.selectedModelRouteCount}
                  </li>
                  <li>
                    {t("当前账号分组数", "Active account groups")}:{" "}
                    {selectedRevisionDiff.activeAccountGroupCount}
                  </li>
                  <li>
                    {t("选中账号分组数", "Selected account groups")}:{" "}
                    {selectedRevisionDiff.selectedAccountGroupCount}
                  </li>
                  {selectedRevisionDiff.aliasChanges.map((entry) => (
                    <li key={`alias-change:${entry.alias}`}>
                      {entry.alias}: {entry.activeModel ?? t("无", "<none>")} -&gt;{" "}
                      {entry.selectedModel ?? t("无", "<none>")}
                    </li>
                  ))}
                  {selectedRevisionDiff.addedProviders.map((providerId) => (
                    <li key={`provider-added:${providerId}`}>
                      {t(`新增 Provider：${providerId}`, `Added provider: ${providerId}`)}
                    </li>
                  ))}
                  {selectedRevisionDiff.removedProviders.map((providerId) => (
                    <li key={`provider-removed:${providerId}`}>
                      {t(`移除 Provider：${providerId}`, `Removed provider: ${providerId}`)}
                    </li>
                  ))}
                  {selectedRevisionDiff.addedModelRoutes.map((pattern) => (
                    <li key={`route-added:${pattern}`}>
                      {t(`新增路由：${pattern}`, `Added route: ${pattern}`)}
                    </li>
                  ))}
                  {selectedRevisionDiff.removedModelRoutes.map((pattern) => (
                    <li key={`route-removed:${pattern}`}>
                      {t(`移除路由：${pattern}`, `Removed route: ${pattern}`)}
                    </li>
                  ))}
                  {selectedRevisionDiff.addedAccountGroups.map((groupName) => (
                    <li key={`group-added:${groupName}`}>
                      {t(`新增分组：${groupName}`, `Added group: ${groupName}`)}
                    </li>
                  ))}
                  {selectedRevisionDiff.removedAccountGroups.map((groupName) => (
                    <li key={`group-removed:${groupName}`}>
                      {t(`移除分组：${groupName}`, `Removed group: ${groupName}`)}
                    </li>
                  ))}
                  {selectedRevisionDiff.changedAccountGroups.map((entry) => (
                    <li key={`group-changed:${entry.groupId}`}>
                      {t(
                        `更新分组：${entry.groupId}，名称 ${entry.activeName || "无"} -> ${entry.selectedName || "无"}，描述 ${entry.activeDescription ?? "无"} -> ${entry.selectedDescription ?? "无"}，状态 ${entry.activeEnabled ? "启用" : "停用"} -> ${entry.selectedEnabled ? "启用" : "停用"}，备注 ${entry.activeNotes ?? "无"} -> ${entry.selectedNotes ?? "无"}，倍率 ${entry.activeBillingMultiplier} -> ${entry.selectedBillingMultiplier}，成员 ${entry.activeMembers.join(", ") || "无"} -> ${entry.selectedMembers.join(", ") || "无"}`,
                        `Updated group: ${entry.groupId}, name ${entry.activeName || "<none>"} -> ${entry.selectedName || "<none>"}, description ${entry.activeDescription ?? "<none>"} -> ${entry.selectedDescription ?? "<none>"}, status ${entry.activeEnabled ? "enabled" : "disabled"} -> ${entry.selectedEnabled ? "enabled" : "disabled"}, notes ${entry.activeNotes ?? "<none>"} -> ${entry.selectedNotes ?? "<none>"}, multiplier ${entry.activeBillingMultiplier} -> ${entry.selectedBillingMultiplier}, members ${entry.activeMembers.join(", ") || "<none>"} -> ${entry.selectedMembers.join(", ") || "<none>"}`,
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            <div className="nt-grid nt-grid--2" aria-label="Revision document snapshots">
              <div>
                <h3>{t("当前激活路由文档", "Active route document")}</h3>
                <pre className="nt-code" aria-label="Active route document snapshot">
                  {activeRouteDocumentText}
                </pre>
              </div>
              <div>
                <h3>{t("选中修订路由文档", "Selected revision route document")}</h3>
                <pre className="nt-code" aria-label="Selected revision route document snapshot">
                  {selectedRouteDocumentText}
                </pre>
              </div>
            </div>
            <div className="nt-actions nt-actions--right">
              <button
                className="nt-btn nt-btn--secondary"
                type="button"
                disabled={editorLocked}
                onClick={() => {
                  loadSelectedRevisionIntoEditor();
                  setActiveWorkspace("editor");
                }}
              >
                {t("将修订装载到编辑器", "Load revision into editor")}
              </button>
              <button
                className="nt-btn nt-btn--primary"
                type="button"
                disabled={
                  selectedRevision.active || busy || actionBusy !== null || !mutationSupported
                }
                onClick={() => setRestoreDialogOpen(true)}
              >
                {t("恢复为激活配置", "Restore revision as active config")}
              </button>
            </div>
          </div>
        ) : (
          <p className="nt-empty">
            {t(
              "选择任意修订可查看归档详情，并将历史配置直接装载到编辑器中。",
              "Select any revision to inspect archived detail and load its historical configuration into the editor.",
            )}
          </p>
        )}
      </article>
    </div>
  );
  const editorWorkspace = (
    <div className="nt-stack">
      <div className="nt-grid nt-grid--2">
        <article className="nt-card nt-card--panel" aria-label="Structured alias editor">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Alias editor</p>
              <h2>{t("结构化 Alias 编辑", "Structured alias editor")}</h2>
            </div>
            <div className="nt-actions">
              <button
                className="nt-btn nt-btn--secondary"
                type="button"
                disabled={editorLocked}
                onClick={addAliasRow}
              >
                {t("添加 Alias 行", "Add alias row")}
              </button>
            </div>
          </div>
          {aliasDraftRows.length > 0 ? (
            <div className="nt-stack">
              {aliasDraftRows.map((row, index) => (
                <div className="nt-form-grid" key={row.id}>
                  <label className="nt-field">
                    <span>{t(`Alias 名称 ${index + 1}`, `Alias name ${index + 1}`)}</span>
                    <input
                      className="nt-input"
                      value={row.alias}
                      disabled={editorLocked}
                      onChange={(event) =>
                        updateAliasRow(row.id, "alias", event.currentTarget.value)
                      }
                    />
                  </label>
                  <label className="nt-field nt-field--wide">
                    <span>{t(`Alias 目标 ${index + 1}`, `Alias target ${index + 1}`)}</span>
                    <input
                      className="nt-input"
                      value={row.model}
                      disabled={editorLocked}
                      onChange={(event) =>
                        updateAliasRow(row.id, "model", event.currentTarget.value)
                      }
                    />
                  </label>
                  <div className="nt-actions nt-actions--right">
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={editorLocked}
                      onClick={() => removeAliasRow(row.id)}
                    >
                      {t(`移除 Alias 行 ${index + 1}`, `Remove alias row ${index + 1}`)}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="nt-empty">
              {t(
                "当前还没有 Alias，可以直接添加结构化 Alias 行。",
                "No aliases yet. You can add a structured alias row directly.",
              )}
            </p>
          )}
        </article>

        <article className="nt-card nt-card--panel" aria-label="Structured model route editor">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Model routes</p>
              <h2>{t("结构化模型路由编辑", "Structured model route editor")}</h2>
            </div>
            <div className="nt-actions">
              <button
                className="nt-btn nt-btn--secondary"
                type="button"
                disabled={editorLocked}
                onClick={addModelRouteRow}
              >
                {t("添加模型路由行", "Add model route row")}
              </button>
            </div>
          </div>
          {modelRouteDraftRows.length > 0 ? (
            <div className="nt-stack">
              {modelRouteDraftRows.map((row, index) => (
                <div className="nt-form-grid" key={row.id}>
                  <label className="nt-field nt-field--wide">
                    <span>{t(`模型路由模式 ${index + 1}`, `Model route pattern ${index + 1}`)}</span>
                    <input
                      className="nt-input"
                      value={row.pattern}
                      disabled={editorLocked}
                      onChange={(event) =>
                        updateModelRouteRow(row.id, event.currentTarget.value)
                      }
                    />
                  </label>
                  <div className="nt-actions nt-actions--right">
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={editorLocked}
                      onClick={() => removeModelRouteRow(row.id)}
                    >
                      {t(`移除模型路由行 ${index + 1}`, `Remove model route row ${index + 1}`)}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="nt-empty">
              {t(
                "当前还没有模型路由，可以直接添加结构化路由行。",
                "No model routes yet. You can add a structured route row directly.",
              )}
            </p>
          )}
        </article>
      </div>

      <article className="nt-card nt-card--panel" aria-label="Structured provider editor">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Providers</p>
            <h2>{t("结构化 Provider 编辑", "Structured provider editor")}</h2>
          </div>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={editorLocked}
              onClick={addProviderRow}
            >
              {t("添加 Provider 行", "Add provider row")}
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setActiveWorkspace("advanced")}
            >
              {t("打开 JSON 文档", "Open JSON document")}
            </button>
          </div>
        </div>
        {providerDraftRows.length > 0 ? (
          <div className="nt-stack">
            {providerDraftRows.map((row, index) => (
              <div className="nt-form-grid" key={row.id}>
                <label className="nt-field">
                  <span>{t(`Provider ID ${index + 1}`, `Provider id ${index + 1}`)}</span>
                    <input
                      className="nt-input"
                      value={row.providerId}
                      disabled={editorLocked}
                    onChange={(event) =>
                      updateProviderRow(row.id, "providerId", event.currentTarget.value)
                    }
                  />
                </label>
                <label className="nt-field">
                  <span>{t(`Provider 预设 ${index + 1}`, `Provider preset ${index + 1}`)}</span>
                    <input
                      className="nt-input"
                      value={row.preset}
                      disabled={editorLocked}
                    onChange={(event) =>
                      updateProviderRow(row.id, "preset", event.currentTarget.value)
                    }
                    />
                </label>
                <label className="nt-field">
                  <span>
                    {t(
                      `Provider 服务商标识 ${index + 1}`,
                      `Provider vendor key ${index + 1}`,
                    )}
                  </span>
                  <input
                    className="nt-input"
                    value={row.vendorKey}
                    disabled={editorLocked}
                    onChange={(event) =>
                      updateProviderRow(row.id, "vendorKey", event.currentTarget.value)
                    }
                  />
                </label>
                <label className="nt-field">
                  <span>
                    {t(
                      `Provider 服务商名称 ${index + 1}`,
                      `Provider vendor name ${index + 1}`,
                    )}
                  </span>
                  <input
                    className="nt-input"
                    value={row.vendorName}
                    disabled={editorLocked}
                    onChange={(event) =>
                      updateProviderRow(row.id, "vendorName", event.currentTarget.value)
                    }
                  />
                </label>
                <label className="nt-field nt-field--wide">
                  <span>{t(`Provider 基础 URL ${index + 1}`, `Provider base URL ${index + 1}`)}</span>
                    <input
                      className="nt-input"
                      value={row.baseUrl}
                      disabled={editorLocked}
                    onChange={(event) =>
                      updateProviderRow(row.id, "baseUrl", event.currentTarget.value)
                    }
                  />
                </label>
                <label className="nt-field nt-field--wide">
                  <span>
                    {t(
                      `Provider 支持模型 ${index + 1}`,
                      `Provider supported models ${index + 1}`,
                    )}
                  </span>
                    <textarea
                      className="nt-input nt-textarea"
                      value={row.supportedModelsText}
                      spellCheck={false}
                      disabled={editorLocked}
                    onChange={(event) =>
                      updateProviderRow(row.id, "supportedModelsText", event.currentTarget.value)
                    }
                  />
                </label>
                <div className="nt-actions nt-actions--right">
                  <button
                    className="nt-btn nt-btn--outline"
                    type="button"
                    disabled={editorLocked}
                    onClick={() => removeProviderRow(row.id)}
                  >
                    {t(`移除 Provider 行 ${index + 1}`, `Remove provider row ${index + 1}`)}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="nt-empty">
            {t(
              "当前还没有 Provider，可以直接添加结构化 Provider 行。",
              "No providers yet. You can add a structured provider row directly.",
            )}
          </p>
        )}
      </article>
    </div>
  );
  const providersWorkspace = (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Provider Hub</p>
            <h2>{t("Provider 概览", "Provider overview")}</h2>
          </div>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              onClick={() => setActiveWorkspace("editor")}
            >
              {t("进入结构化编辑", "Open structured editor")}
            </button>
          </div>
        </div>
        <div className="nt-console-preview-grid">
          {activeProviderSummaries.map((provider) => (
            <article className="nt-card nt-card--stat nt-console-mini-card" key={provider.id}>
              <span>{provider.preset ?? t("未设置 preset", "No preset")}</span>
              <strong>{provider.id}</strong>
              <small>{hostLabelFromUrl(provider.baseUrl) ?? t("未配置地址", "No base URL")}</small>
              <small>
                {provider.supportedModelsCount} {t("模型", "models")} · {provider.credentialCount}{" "}
                {t("凭据", "credentials")}
              </small>
            </article>
          ))}
        </div>
      </article>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Route mapping</p>
            <h2>{t("模型路由概览", "Model route overview")}</h2>
          </div>
        </div>
        <div className="nt-scroll-panel">
          <ul className="nt-simple-list">
            {activeModelRouteSummaries.map((route) => (
              <li key={route.pattern}>
                <div>
                  <strong>{route.pattern}</strong>
                  <span>
                    {route.providerIds.length > 0
                      ? route.providerIds.join(", ")
                      : t("未绑定 Provider", "No provider binding")}
                  </span>
                </div>
                <span>
                  {route.priority
                    ? t(`优先级 ${route.priority}`, `Priority ${route.priority}`)
                    : t("默认优先级", "Default priority")}
                </span>
              </li>
            ))}
          </ul>
        </div>
      </article>
    </div>
  );
  const accountsWorkspace = (
    <div className="nt-stack">
      {draftStructureNotice}
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Accounts</p>
            <h2>{t("账号台账", "Accounts")}</h2>
            <p className="nt-copy">
              {t(
                "把 route-config 中的 provider credentials 视为可复用账号单元，并按照服务商/Provider 进行分组浏览。",
                "Treat provider credentials in the route config as reusable account units and browse them grouped by service provider / provider.",
              )}
            </p>
          </div>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--primary"
              type="button"
              disabled={editorLocked || credentialProviderOptions.length === 0}
              onClick={() => openAddCredentialDialog()}
            >
              {t("添加账号", "Add account")}
            </button>
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              onClick={() => setActiveWorkspace("groups")}
            >
              {t("进入分组策略", "Open groups workspace")}
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setActiveWorkspace("editor")}
            >
              {t("返回路由编辑", "Back to route editor")}
            </button>
          </div>
        </div>
        <dl className="nt-meta-list nt-meta-list--inline">
          <div>
            <dt>{t("账号总数", "Accounts")}</dt>
            <dd>{displayedAccountCatalog.accounts.length}</dd>
          </div>
          <div>
            <dt>{t("分组数量", "Groups")}</dt>
            <dd>{displayedAccountCatalog.groups.length}</dd>
          </div>
          <div>
            <dt>{t("未分组账号", "Ungrouped")}</dt>
            <dd>{displayedAccountCatalog.ungroupedCount}</dd>
          </div>
          <div>
            <dt>{t("服务商", "Vendors")}</dt>
            <dd>{displayedAccountCatalog.providerBuckets.length}</dd>
          </div>
        </dl>
        <div className="nt-account-toolbar">
          <label className="nt-field">
            <span>{t("筛选账号", "Filter accounts")}</span>
            <input
              className="nt-input"
              type="search"
              value={accountSearch}
              placeholder={t("账号、Provider、模型或分组", "Account, provider, model, or group")}
              onChange={(event) => setAccountSearch(event.currentTarget.value)}
            />
          </label>
          <label className="nt-field">
            <span>{t("分组状态", "Grouping status")}</span>
            <select
              className="nt-select"
              value={accountMembershipFilter}
              onChange={(event) =>
                setAccountMembershipFilter(event.currentTarget.value as AccountMembershipFilter)
              }
            >
              <option value="all">{t("全部账号", "All accounts")}</option>
              <option value="grouped">{t("已分组账号", "Grouped accounts")}</option>
              <option value="ungrouped">{t("未分组账号", "Ungrouped accounts")}</option>
            </select>
          </label>
          <label className="nt-field">
            <span>{t("启用状态", "Enabled status")}</span>
            <select
              className="nt-select"
              value={accountEnabledFilter}
              onChange={(event) =>
                setAccountEnabledFilter(event.currentTarget.value as AccountEnabledFilter)
              }
            >
              <option value="all">{t("全部状态", "All statuses")}</option>
              <option value="enabled">{t("已启用", "Enabled")}</option>
              <option value="disabled">{t("已停用", "Disabled")}</option>
            </select>
          </label>
          <div className="nt-filter-count" role="status">
            <span>{t("当前显示", "Showing")}</span>
            <strong>
              {filteredAccountCatalog.accounts.length} / {displayedAccountCatalog.accounts.length}
            </strong>
          </div>
        </div>
      </article>

      {filteredAccountCatalog.providerBuckets.length > 0 ? (
        filteredAccountCatalog.providerBuckets.map((bucket) => (
          <details
            className="nt-account-bucket"
            open={filteredAccountCatalog.providerBuckets.length <= 4 ? true : undefined}
            key={`${bucket.key}:${accountSearch}:${accountMembershipFilter}:${accountEnabledFilter}`}
          >
            <summary className="nt-account-bucket__summary">
              <span className="nt-account-bucket__title">
                <small className="nt-kicker">// {bucket.vendorKey}</small>
                <span role="heading" aria-level={3}>
                  {bucket.label} · {countText(bucket.accountCount, "个账号", "account", "accounts")}
                </span>
              </span>
              <span className="nt-account-bucket__hint">
                {t("查看账号", "View accounts")}
              </span>
            </summary>
            <div className="nt-stack nt-account-bucket__body">
              {bucket.providers.map((provider) => (
                <article className="nt-card nt-card--panel nt-console-mini-card" key={provider.id}>
                  <div className="nt-section__head">
                    <div>
                      <strong>{provider.label}</strong>
                      <p className="nt-copy">
                        {provider.preset ?? t("未设置 preset", "No preset")} ·{" "}
                        {hostLabelFromUrl(provider.baseUrl) ?? t("未配置地址", "No base URL")}
                      </p>
                    </div>
                  </div>
                  <div className="nt-scroll-panel">
                    <ul className="nt-simple-list">
                      {provider.accounts.map((account) => {
                        const probeResult = credentialProbeResults[account.id];
                        const probeDisabled =
                          busy ||
                          actionBusy !== null ||
                          credentialProbeBusy !== null ||
                          !account.enabled ||
                          draftDirty ||
                          !draftMatchesActiveRevision;
                        return (
                          <li key={account.id}>
                            <div>
                              <strong>{account.displayName}</strong>
                              <span
                                className={`nt-badge ${
                                  account.enabled ? "nt-badge--success" : "nt-badge--warning"
                                }`}
                              >
                                {account.enabled
                                  ? t("已启用", "Enabled")
                                  : t("已停用", "Disabled")}
                              </span>
                              <span>
                                {account.id} ·{" "}
                                {account.mode === "provider-default"
                                  ? t("Provider 默认凭据", "Provider default credential")
                                  : t("显式凭据", "Explicit credential")}
                              </span>
                              <span>
                                {account.supportedModels.length > 0
                                  ? account.supportedModels.join(", ")
                                  : t("未声明模型", "No declared models")}
                              </span>
                              {probeResult ? (
                                <span
                                  className={`nt-badge ${
                                    probeResult.status === "passed"
                                      ? "nt-badge--success"
                                      : probeResult.status === "unsupported"
                                        ? "nt-badge--warning"
                                        : "nt-badge--danger"
                                  }`}
                                >
                                  {probeResult.status === "passed"
                                    ? t("连通正常", "Connectivity passed")
                                    : probeResult.status === "failed"
                                      ? t("连接失败", "Connectivity failed")
                                      : probeResult.status === "unsupported"
                                        ? t("暂不支持测试", "Probe unsupported")
                                        : t("测试异常", "Probe error")}
                                </span>
                              ) : (
                                <span>{t("尚未测试", "Not tested")}</span>
                              )}
                            </div>
                            <div className="nt-stack">
                              {account.groupNames.length > 0 ? (
                                <div className="nt-actions nt-actions--right">
                                  {account.groupNames.map((groupName) => (
                                    <span className="nt-chip" key={`${account.id}:${groupName}`}>
                                      {groupName}
                                    </span>
                                  ))}
                                </div>
                              ) : (
                                <span>{t("未加入分组", "Ungrouped")}</span>
                              )}
                              {probeResult ? (
                                <div className="nt-probe-result" role="status">
                                  <span>{probeResult.message}</span>
                                  <time dateTime={probeResult.checkedAt}>
                                    {probeResult.checkedAt}
                                  </time>
                                </div>
                              ) : null}
                              <div className="nt-actions nt-actions--right">
                                <button
                                  className="nt-btn nt-btn--outline"
                                  type="button"
                                  disabled={probeDisabled}
                                  aria-label={t(
                                    `测试账号 ${account.displayName}`,
                                    `Test account ${account.displayName}`,
                                  )}
                                  onClick={() => void handleCredentialProbe(account)}
                                >
                                  {credentialProbeBusy === account.id
                                    ? t("测试中...", "Probing...")
                                    : t("测试", "Probe")}
                                </button>
                                {account.mode === "credential" ? (
                                  <>
                                    <button
                                      className="nt-btn nt-btn--outline"
                                      type="button"
                                      disabled={editorLocked}
                                      aria-label={t(
                                        `编辑账号 ${account.displayName}`,
                                        `Edit account ${account.displayName}`,
                                      )}
                                      onClick={() =>
                                        openEditCredentialDialog(account.providerId, account.id)
                                      }
                                    >
                                      {t("编辑", "Edit")}
                                    </button>
                                    <button
                                      className="nt-btn nt-btn--danger"
                                      type="button"
                                      disabled={editorLocked}
                                      aria-label={t(
                                        `删除账号 ${account.displayName}`,
                                        `Delete account ${account.displayName}`,
                                      )}
                                      onClick={() =>
                                        removeCredential(
                                          account.providerId,
                                          account.id,
                                          account.displayName,
                                        )
                                      }
                                    >
                                      {t("删除", "Delete")}
                                    </button>
                                  </>
                                ) : (
                                  <button
                                    className="nt-btn nt-btn--outline"
                                    type="button"
                                    disabled={editorLocked}
                                    aria-label={t(
                                      `为 ${account.providerLabel} 添加显式账号`,
                                      `Add explicit account for ${account.providerLabel}`,
                                    )}
                                    onClick={() => openAddCredentialDialog(account.providerId)}
                                  >
                                    {t("添加显式账号", "Add explicit account")}
                                  </button>
                                )}
                              </div>
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                </article>
              ))}
            </div>
          </details>
        ))
      ) : displayedAccountCatalog.accounts.length > 0 ? (
        <article className="nt-card nt-card--panel">
          <h2>{t("没有匹配账号", "No matching accounts")}</h2>
          <p className="nt-copy">
            {t(
              "当前筛选条件没有匹配任何账号，请调整关键词或账号状态。",
              "No accounts match the current filters. Adjust the query or account status.",
            )}
          </p>
        </article>
      ) : (
        <article className="nt-card nt-card--panel">
          <h2>{t("暂无账号", "No accounts yet")}</h2>
          <p className="nt-copy">
            {t(
              "当前 route-config 里还没有解析出 provider credential。当前结构化编辑器不直接编辑 credentials，请在高级 JSON 中添加 credentials，再回到这里管理账号池。",
              "No provider credentials were found in the current route config. The structured editor does not edit credentials directly; add them in Advanced JSON, then come back here to manage the account pool.",
            )}
          </p>
        </article>
      )}
    </div>
  );
  const groupsWorkspace = (
    <div className="nt-stack">
      {draftStructureNotice}
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Groups</p>
            <h2>{t("分组策略", "Groups")}</h2>
            <p className="nt-copy">
              {t(
                "为多个账号建立隔离池，并记录后续 Platform 可消费的计费倍率元数据。",
                "Create isolated pools across multiple accounts and store billing-multiplier metadata for later Platform consumption.",
              )}
            </p>
          </div>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={editorLocked}
              onClick={addAccountGroupRow}
            >
              {t("添加分组", "Add group")}
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setActiveWorkspace("accounts")}
            >
              {t("返回账号台账", "Back to accounts")}
            </button>
          </div>
        </div>
        <dl className="nt-meta-list nt-meta-list--inline">
          <div>
            <dt>{t("已定义分组", "Groups")}</dt>
            <dd>{displayedAccountCatalog.groups.length}</dd>
          </div>
          <div>
            <dt>{t("可选账号", "Accounts available")}</dt>
            <dd>{displayedAccountCatalog.accounts.length}</dd>
          </div>
          <div>
            <dt>{t("未分组账号", "Ungrouped")}</dt>
            <dd>{displayedAccountCatalog.ungroupedCount}</dd>
          </div>
          <div>
            <dt>{t("跨 Provider 组织", "Cross-provider")}</dt>
            <dd>{t("支持", "enabled")}</dd>
          </div>
        </dl>
      </article>

      {accountGroupDraftRows.length > 0 ? (
        accountGroupDraftRows.map((row, index) => {
          const accountSearchValue = accountGroupSearches[row.id] ?? "";
          const groupIdIncomplete = accountGroupDraftNeedsId(row);
          const groupIdErrorId = `${row.id}-group-id-error`;
          const billingMultiplierInvalid =
            parseAccountGroupBillingMultiplier(row.billingMultiplier) === null;
          const billingMultiplierErrorId = `${row.id}-billing-multiplier-error`;
          const matchingAccountCatalog = filterRouteAccountCatalog(
            displayedAccountCatalog,
            accountSearchValue,
            "all",
          );
          return (
          <article className="nt-card nt-card--panel" key={row.id}>
            <div className="nt-section__head">
              <div>
                <p className="nt-kicker">// {row.groupId || row.name || t("新分组", "New group")}</p>
                <h3>{row.name || t("未命名分组", "Unnamed group")}</h3>
              </div>
              <div className="nt-actions">
                <label className="nt-chip">
                  <input
                    checked={row.enabled}
                    disabled={editorLocked}
                    aria-label={t(
                      `${row.name || row.groupId || `分组 ${index + 1}`} 启用状态`,
                      `${row.name || row.groupId || `Group ${index + 1}`} enabled state`,
                    )}
                    onChange={(event) =>
                      updateAccountGroupEnabled(row.id, event.currentTarget.checked)
                    }
                    type="checkbox"
                  />
                  <span>{row.enabled ? t("启用", "Enabled") : t("停用", "Disabled")}</span>
                </label>
                <button
                  className="nt-btn nt-btn--outline"
                  type="button"
                  disabled={editorLocked}
                  onClick={() => removeAccountGroupRow(row.id)}
                >
                  {t(`移除分组 ${index + 1}`, `Remove group ${index + 1}`)}
                </button>
              </div>
            </div>
            <div className="nt-form-grid">
              <label className="nt-field">
                <span>{t(`分组 ID ${index + 1}`, `Group ID ${index + 1}`)}</span>
                <input
                  className="nt-input"
                  value={row.groupId}
                  disabled={editorLocked}
                  aria-invalid={groupIdIncomplete}
                  aria-describedby={groupIdIncomplete ? groupIdErrorId : undefined}
                  onChange={(event) =>
                    updateAccountGroupRow(row.id, "groupId", event.currentTarget.value)
                  }
                />
                {groupIdIncomplete ? (
                  <small id={groupIdErrorId}>
                    {t(ACCOUNT_GROUP_ID_REQUIRED_ERROR_ZH, ACCOUNT_GROUP_ID_REQUIRED_ERROR_EN)}
                  </small>
                ) : null}
              </label>
              <label className="nt-field">
                <span>{t(`分组名称 ${index + 1}`, `Group name ${index + 1}`)}</span>
                <input
                  className="nt-input"
                  value={row.name}
                  disabled={editorLocked}
                  onChange={(event) =>
                    updateAccountGroupRow(row.id, "name", event.currentTarget.value)
                  }
                />
              </label>
              <label className="nt-field">
                <span>{t(`计费倍率 ${index + 1}`, `Billing multiplier ${index + 1}`)}</span>
                <input
                  className="nt-input"
                  inputMode="decimal"
                  value={row.billingMultiplier}
                  disabled={editorLocked}
                  aria-invalid={billingMultiplierInvalid}
                  aria-describedby={
                    billingMultiplierInvalid ? billingMultiplierErrorId : undefined
                  }
                  onChange={(event) =>
                    updateAccountGroupRow(row.id, "billingMultiplier", event.currentTarget.value)
                  }
                />
                {billingMultiplierInvalid ? (
                  <small id={billingMultiplierErrorId}>
                    {t(
                      ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_ZH,
                      ACCOUNT_GROUP_BILLING_MULTIPLIER_ERROR_EN,
                    )}
                  </small>
                ) : null}
              </label>
              <label className="nt-field nt-field--wide">
                <span>{t(`描述 ${index + 1}`, `Description ${index + 1}`)}</span>
                <input
                  className="nt-input"
                  value={row.description}
                  disabled={editorLocked}
                  onChange={(event) =>
                    updateAccountGroupRow(row.id, "description", event.currentTarget.value)
                  }
                />
              </label>
              <label className="nt-field nt-field--wide">
                <span>{t(`备注 ${index + 1}`, `Notes ${index + 1}`)}</span>
                <textarea
                  className="nt-input nt-textarea"
                  value={row.notes}
                  disabled={editorLocked}
                  onChange={(event) =>
                    updateAccountGroupRow(row.id, "notes", event.currentTarget.value)
                  }
                />
              </label>
            </div>
            <fieldset className="nt-field nt-field--wide">
              <legend>{t(`关联账号 ${index + 1}`, `Accounts ${index + 1}`)}</legend>
              <div className="nt-account-toolbar nt-account-toolbar--picker">
                <label className="nt-field">
                  <span>{t(`筛选关联账号 ${index + 1}`, `Filter accounts ${index + 1}`)}</span>
                  <input
                    className="nt-input"
                    type="search"
                    value={accountSearchValue}
                    placeholder={t("账号、Provider 或模型", "Account, provider, or model")}
                    onChange={(event) => {
                      const nextValue = event.currentTarget.value;
                      setAccountGroupSearches((current) => ({
                        ...current,
                        [row.id]: nextValue,
                      }));
                    }}
                  />
                </label>
                <div className="nt-filter-count" role="status">
                  <span>{t("已选 / 匹配", "Selected / matching")}</span>
                  <strong>
                    {row.providerCredentialIds.length} / {matchingAccountCatalog.accounts.length}
                  </strong>
                </div>
              </div>
              <div
                className="nt-stack nt-account-picker"
                role="region"
                tabIndex={0}
                aria-label={t(
                  `${row.name || row.groupId || `分组 ${index + 1}`} 可选账号`,
                  `${row.name || row.groupId || `Group ${index + 1}`} selectable accounts`,
                )}
              >
                {matchingAccountCatalog.providerBuckets.map((bucket) => (
                  <article className="nt-card nt-card--panel nt-console-mini-card" key={bucket.key}>
                    <strong>{bucket.label}</strong>
                    <div className="nt-stack">
                      {bucket.providers.map((provider) => (
                        <div className="nt-stack" key={provider.id}>
                          <span>
                            {provider.label} ·{" "}
                            {hostLabelFromUrl(provider.baseUrl) ?? t("未配置地址", "No base URL")}
                          </span>
                          <div className="nt-actions">
                            {provider.accounts.map((account) => (
                              <label className="nt-chip" key={`${row.id}:${account.id}`}>
                                <input
                                  type="checkbox"
                                  checked={row.providerCredentialIds.includes(account.id)}
                                  disabled={editorLocked}
                                  onChange={() => toggleAccountGroupMember(row.id, account.id)}
                                  aria-label={t(
                                    `${row.name || row.groupId || `分组 ${index + 1}`} · ${provider.label} · ${account.displayName}`,
                                    `${row.name || row.groupId || `Group ${index + 1}`} · ${provider.label} · ${account.displayName}`,
                                  )}
                                />
                                <span>{account.displayName}</span>
                              </label>
                            ))}
                          </div>
                        </div>
                      ))}
                    </div>
                  </article>
                ))}
                {matchingAccountCatalog.accounts.length === 0 ? (
                  <p className="nt-empty">
                    {t("没有匹配的可选账号。", "No selectable accounts match this filter.")}
                  </p>
                ) : null}
              </div>
            </fieldset>
          </article>
          );
        })
      ) : (
        <article className="nt-card nt-card--panel">
          <h2>{t("暂无分组", "No groups yet")}</h2>
          <p className="nt-copy">
            {t(
              "可以先创建分组，把不同服务商的账号拉进不同池中，并写入计费倍率元数据。",
              "Create a group first, pull accounts from different providers into separate pools, and store billing multiplier metadata.",
            )}
          </p>
        </article>
      )}
    </div>
  );
  const advancedWorkspace = (
    <article className="nt-card nt-card--panel">
      <div className="nt-section__head">
        <div>
          <p className="nt-kicker">// Advanced JSON</p>
          <h2>{t("高级 JSON 修订", "Advanced JSON patching")}</h2>
        </div>
      </div>
      <label className="nt-field nt-field--wide">
        <span>{t("路由配置 JSON", "Route document JSON")}</span>
        <textarea
          className="nt-input nt-textarea nt-textarea--json"
          value={editorText}
          spellCheck={false}
          disabled={editorLocked}
          onChange={(event) => {
            const nextText = event.currentTarget.value;
            invalidateCredentialProbes();
            setEditorText(nextText);
            setValidation(null);
            if (error?.startsWith("Route document JSON")) {
              setError(null);
            }
            try {
              const document = parseRouteDocument(nextText);
              setAliasDraftRows(aliasDraftRowsFromDocument(document));
              setModelRouteDraftRows(modelRouteDraftRowsFromDocument(document));
              setProviderDraftRows(providerDraftRowsFromDocument(document));
              setAccountGroupDraftRows(accountGroupDraftRowsFromDocument(document));
            } catch {
              // Keep the last structured snapshots until the JSON becomes valid again.
            }
          }}
        />
      </label>
      <div className="nt-grid nt-grid--2">
        <article className="nt-card nt-card--panel nt-console-mini-card">
          <h3>{t("Alias 概览", "Alias overview")}</h3>
          <div className="nt-scroll-panel">
            <ul className="nt-simple-list">
              {activeAliasEntries.map(([alias, model]) => (
                <li key={alias}>
                  <strong>{alias}</strong>
                  <span>{model}</span>
                </li>
              ))}
            </ul>
          </div>
        </article>
        <article className="nt-card nt-card--panel nt-console-mini-card">
          <h3>{t("Provider 概览", "Provider overview")}</h3>
          <div className="nt-scroll-panel">
            <ul className="nt-simple-list">
              {activeProviderSummaries.map((provider) => (
                <li key={provider.id}>
                  <strong>{provider.id}</strong>
                  <span>{hostLabelFromUrl(provider.baseUrl) ?? t("未配置地址", "No base URL")}</span>
                </li>
              ))}
            </ul>
          </div>
        </article>
      </div>
    </article>
  );
  const overviewWorkspace = (
    <div className="nt-stack">
      <div className="nt-grid nt-grid--2">
        <article className="nt-card nt-card--panel">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Active config</p>
              <h2>{t("当前路由配置", "Active route configuration")}</h2>
            </div>
            <div className="nt-actions">
              <button
                className="nt-btn nt-btn--secondary"
                type="button"
                onClick={() => setActiveWorkspace("editor")}
              >
                {t("进入路由编辑", "Open route editor workspace")}
              </button>
            </div>
          </div>
          <dl className="nt-meta-list nt-meta-list--inline">
            <div>
              <dt>{t("修订号", "Revision")}</dt>
              <dd>{routeConfig?.routeConfig.revision.id}</dd>
            </div>
            <div>
              <dt>{t("序列", "Sequence")}</dt>
              <dd>{routeConfig?.routeConfig.revision.sequence}</dd>
            </div>
            <div>
              <dt>{t("写入能力", "Mutation")}</dt>
              <dd>{mutationSupported ? t("支持", "supported") : t("只读", "read-only")}</dd>
            </div>
            <div>
              <dt>{t("修复状态", "Repair")}</dt>
              <dd>
                {routeConfig?.routeConfig.requiresRepair
                  ? t("需要修复", "required")
                  : t("无需修复", "not required")}
              </dd>
            </div>
            <div>
              <dt>{t("Alias 数量", "Aliases")}</dt>
              <dd>{activeAliasEntries.length}</dd>
            </div>
            <div>
              <dt>{t("敏感字段", "Secret fields")}</dt>
              <dd>{secretPatches.length}</dd>
            </div>
          </dl>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setActiveWorkspace("providers")}
            >
              {t("查看 Providers", "View providers")}
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setActiveWorkspace("secrets")}
            >
              {t("查看敏感信息", "View secrets")}
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              onClick={() => setActiveWorkspace("revisions")}
            >
              {t("查看修订历史", "View revisions")}
            </button>
          </div>
        </article>

        <article className="nt-card nt-card--panel">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Provider Hub</p>
              <h2>{t("Provider 概览", "Provider overview")}</h2>
            </div>
          </div>
          <div className="nt-console-preview-grid">
            {activeProviderSummaries.slice(0, 6).map((provider) => (
              <article className="nt-card nt-card--stat nt-console-mini-card" key={provider.id}>
                <span>{provider.preset ?? t("未设置 preset", "No preset")}</span>
                <strong>{provider.id}</strong>
                <small>
                  {hostLabelFromUrl(provider.baseUrl) ?? t("未配置地址", "No base URL")}
                </small>
              </article>
            ))}
          </div>
          {activeProviderSummaries.length > 6 ? (
            <p className="nt-copy">
              {t(
                `还有 ${activeProviderSummaries.length - 6} 个 Provider，进入 Providers 工作区查看完整清单。`,
                `${activeProviderSummaries.length - 6} more providers are available in the Providers workspace.`,
              )}
            </p>
          ) : null}
        </article>
      </div>

      <div className="nt-grid nt-grid--2">
        <article className="nt-card nt-card--panel">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Alias overview</p>
              <h2>{t("Alias 概览", "Alias overview")}</h2>
            </div>
          </div>
          <div className="nt-scroll-panel">
            <ul className="nt-simple-list">
              {activeAliasEntries.map(([alias, model]) => (
                <li key={alias}>
                  <strong>{alias}</strong>
                  <span>{model}</span>
                </li>
              ))}
            </ul>
          </div>
        </article>

        <article className="nt-card nt-card--panel">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Route overview</p>
              <h2>{t("模型路由概览", "Model route overview")}</h2>
            </div>
          </div>
          <div className="nt-scroll-panel">
            <ul className="nt-simple-list">
              {activeModelRouteSummaries.map((route) => (
                <li key={route.pattern}>
                  <div>
                    <strong>{route.pattern}</strong>
                    <span>
                      {route.providerIds.length > 0
                        ? route.providerIds.join(", ")
                        : t("未绑定 Provider", "No provider binding")}
                    </span>
                  </div>
                  <span>
                    {route.priority
                      ? t(`优先级 ${route.priority}`, `Priority ${route.priority}`)
                      : t("默认优先级", "Default priority")}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        </article>
      </div>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Revision snapshot</p>
            <h2>{t("修订历史快照", "Revision snapshot")}</h2>
          </div>
        </div>
        <div className="nt-scroll-panel">
          <ul className="nt-simple-list">
            {revisions?.revisions.map((entry) => (
              <li key={entry.revision.id}>
                <div>
                  <strong>{entry.revision.id}</strong>
                  <span>{entry.revision.message ?? t("无说明", "no message")}</span>
                </div>
                <div className="nt-actions nt-actions--right">
                  <span>{entry.active ? t("当前激活", "ACTIVE") : entry.source}</span>
                  <button
                    className="nt-btn nt-btn--outline"
                    type="button"
                    disabled={revisionBusy}
                    onClick={() => void handleInspectRevision(entry.revision.id)}
                  >
                    {t(`查看修订 ${entry.revision.id}`, `Inspect revision ${entry.revision.id}`)}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </article>
    </div>
  );
  const workspaceContent =
    activeWorkspace === "overview"
      ? overviewWorkspace
      : activeWorkspace === "editor"
        ? editorWorkspace
        : activeWorkspace === "providers"
          ? providersWorkspace
          : activeWorkspace === "accounts"
            ? accountsWorkspace
            : activeWorkspace === "groups"
              ? groupsWorkspace
          : activeWorkspace === "secrets"
            ? secretsWorkspace
            : activeWorkspace === "revisions"
              ? revisionsWorkspace
              : advancedWorkspace;

  if (busy && !routeConfig) {
    return <main role="status">{t("正在加载 Gateway 控制台...", "Loading Gateway console...")}</main>;
  }

  return (
    <>
      <main className="nt-shell nt-shell--console">
        <aside className="nt-rail nt-rail--console">
          <div className="nt-brand">
            <div className="nt-brand__orb">GW</div>
            <div className="nt-brand__copy">
              <small className="nt-kicker">{t("Gateway", "Gateway")}</small>
              <strong>{t("网页控制台", "Web console")}</strong>
            </div>
          </div>

          <nav className="nt-rail__nav nt-console-nav" aria-label="Gateway console navigation">
            {workspaceItems.map((item) => {
              const active = item.id === activeWorkspace;
              return (
                <button
                  key={item.id}
                  className={`nt-rail__item${active ? " nt-rail__item--active" : ""}`}
                  type="button"
                  aria-pressed={active}
                  onClick={() => setActiveWorkspace(item.id)}
                >
                  <small>{item.eyebrow}</small>
                  <span>{item.label}</span>
                  <em className="nt-rail__meta">{item.meta}</em>
                </button>
              );
            })}
          </nav>

          <div className="nt-rail__footer">
            <div className={mutationSupported ? "nt-chip nt-chip--online" : "nt-chip"}>
              {mutationSupported ? t("可写", "WRITE") : t("只读", "READ ONLY")}
            </div>
            <p>
              {t(
                "本地开发模式 · 配置修改会在保存后生成新的路由修订。",
                "Local development mode · saved changes create a new route revision.",
              )}
            </p>
            <dl className="nt-mini-list">
              <div>
                <dt>{t("激活修订", "Active revision")}</dt>
                <dd>{routeConfig?.routeConfig.revision.id ?? "-"}</dd>
              </div>
              <div>
                <dt>{t("Provider", "Provider")}</dt>
                <dd>{activeProviderSummaries.length}</dd>
              </div>
              <div>
                <dt>{t("Secret grant", "Secret grant")}</dt>
                <dd>{hasSecretAccess ? t("已授权", "granted") : t("未授权", "locked")}</dd>
              </div>
            </dl>
          </div>
        </aside>

        <section className="nt-board nt-board--browser-console">
          <header className="nt-board__header">
            <div>
              <p className="nt-kicker">// {t("Gateway 控制台", "Gateway Console")}</p>
              <h1>{t("Gateway 网页控制台", "Gateway Web Console")}</h1>
              <p className="nt-board__copy">
                {t(
                  "直接通过 Gateway 本体托管的浏览器控制台，当前已接入 route-config 与 revision 历史能力。",
                  "Browser control panel hosted directly by the Gateway runtime, currently wired to route-config and revision history capabilities.",
                )}
              </p>
            </div>
            <div className="nt-actions nt-actions--right">
              <LanguageToggleButton className="nt-btn nt-btn--outline" />
              <button
                className="nt-btn nt-btn--secondary"
                type="button"
                disabled={busy || actionBusy !== null}
                onClick={handleManualRefresh}
              >
                {t("刷新", "Refresh")}
              </button>
              <button
                className="nt-btn nt-btn--outline"
                type="button"
                onClick={() => void session.logout()}
              >
                {t("退出登录", "Sign out")}
              </button>
            </div>
          </header>

          {error ? (
            <div className="nt-alert nt-alert--danger" role="alert">
              <span>{error}</span>
            </div>
          ) : null}

          {successMessage ? (
            <div
              className="nt-alert nt-alert--success"
              role="status"
              aria-label="Gateway console last action"
            >
              <span>{successMessage}</span>
              <button type="button" onClick={() => setSuccessMessage(null)}>
                {t("关闭", "Dismiss")}
              </button>
            </div>
          ) : null}

          {routeConfig ? (
            <>
              <section className="nt-hud-strip" aria-label="Gateway console summary">
                <article className="nt-card nt-card--stat">
                  <span>{t("激活修订", "Active revision")}</span>
                  <strong>{routeConfig.routeConfig.revision.id}</strong>
                </article>
                <article className="nt-card nt-card--stat">
                  <span>{t("来源", "Source")}</span>
                  <strong>{routeConfig.routeConfig.source}</strong>
                </article>
                <article className="nt-card nt-card--stat">
                  <span>{t("Provider 数量", "Providers")}</span>
                  <strong>{routeConfig.routeConfig.document.providers.length}</strong>
                </article>
                <article className="nt-card nt-card--stat">
                  <span>{t("模型路由", "Model routes")}</span>
                  <strong>{routeConfig.routeConfig.document.model_routes.length}</strong>
                </article>
              </section>

              <section className="nt-stage">
                {activeDiagnosticsFeedback}
                {accountSummaryFeedback}
                <article className="nt-card nt-card--panel">
                  <div className="nt-section__head">
                    <div>
                      <p className="nt-kicker">// Workspace</p>
                      <h2>{workspaceItems.find((item) => item.id === activeWorkspace)?.label}</h2>
                    </div>
                    <div className="nt-actions nt-actions--right">
                      <span
                        className={draftDirty ? "nt-chip" : "nt-chip nt-chip--online"}
                        role="status"
                        aria-label="Draft status"
                        aria-live="polite"
                      >
                        {incompleteAccountGroupDraftCount > 0
                          ? t(
                              `有未保存修改 · ${incompleteAccountGroupDraftCount} 个未完成分组草稿`,
                              `Unsaved changes · ${incompleteAccountGroupDraftCount} incomplete group draft${incompleteAccountGroupDraftCount === 1 ? "" : "s"}`,
                            )
                          : draftDirty
                            ? t("有未保存修改", "Unsaved changes")
                            : t("草稿已同步", "Draft in sync")}
                      </span>
                      <button
                        className="nt-btn nt-btn--secondary"
                        type="button"
                        disabled={busy || actionBusy !== null || !mutationSupported}
                        onClick={() => void handleValidate()}
                      >
                        {t("校验草稿", "Validate draft")}
                      </button>
                      <button
                        className="nt-btn nt-btn--primary"
                        type="button"
                        disabled={busy || actionBusy !== null || !mutationSupported}
                        onClick={() => void handleSave()}
                      >
                        {t("保存路由配置", "Save route config")}
                      </button>
                    </div>
                  </div>

                  <label className="nt-field nt-field--wide">
                    <span>{t("修订说明", "Revision message")}</span>
                    <input
                      className="nt-input"
                      placeholder={t(
                        "可选：写入修订历史的说明",
                        "Optional: description stored in revision history",
                      )}
                      value={commitMessage}
                      disabled={editorLocked}
                      onChange={(event) => setCommitMessage(event.currentTarget.value)}
                    />
                  </label>

                  {!mutationSupported ? (
                    <div className="nt-validation-list nt-validation-list--warning">
                      <strong>{t("当前实例是只读模式", "This instance is read only")}</strong>
                      <ul>
                        <li>
                          {t(
                            "route-config runtime 没有启用写入支持，浏览器控制台暂时只能查看。",
                            "Route-config mutation support is disabled, so this browser console is currently view-only.",
                          )}
                        </li>
                      </ul>
                    </div>
                  ) : null}

                  {validationFeedback}
                </article>

                {workspaceContent}
              </section>
            </>
          ) : null}
        </section>
      </main>
      {credentialDialogState ? (
        <CredentialDialog
          open
          mode={credentialDialogState.mode}
          providerOptions={credentialProviderOptions}
          existingCredentialIds={explicitCredentialIds}
          initialValue={credentialDialogState.initialValue}
          locked={editorLocked}
          hasSecretAccess={hasSecretAccess}
          onOpenChange={(open) => {
            if (!open) {
              setCredentialDialogState(null);
            }
          }}
          onRequestSecretAccess={() => setSecretDialogOpen(true)}
          onSubmit={applyCredentialDialogValue}
        />
      ) : null}
      <DiscardDraftDialog
        open={discardRefreshDialogOpen}
        busy={busy}
        onOpenChange={setDiscardRefreshDialogOpen}
        onConfirm={handleConfirmDiscardRefresh}
      />
      <SecretConfirmDialog
        open={secretDialogOpen}
        busy={session.busy}
        error={session.error}
        onOpenChange={setSecretDialogOpen}
        onConfirm={session.confirmSecretAccess}
      />
      {selectedRevision && routeConfig ? (
        <RestoreRevisionDialog
          open={restoreDialogOpen}
          busy={actionBusy === "save"}
          activeRevisionId={routeConfig.routeConfig.revision.id}
          selectedRevisionId={selectedRevision.routeConfig.revision.id}
          commitMessage={restoreCommitMessage}
          secretPatchSummary={secretPatchSummary}
          aliasChangeLines={aliasChangeLines}
          providerChangeLines={providerChangeLines}
          modelRouteChangeLines={modelRouteChangeLines}
          accountGroupChangeLines={accountGroupChangeLines}
          onOpenChange={setRestoreDialogOpen}
          onConfirm={handleRestoreSelectedRevision}
        />
      ) : null}
    </>
  );
}
