import { BarChart3, CalendarClock, Play, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createGatewayApiClient } from "../../api/client";
import { createConsoleApi, type ConsoleApi } from "../../api/console";
import { GatewayApiError } from "../../api/errors";
import { ActionTooltip, NeuroTooltipProvider } from "../../components/ActionTooltip";
import { pushAppToast } from "../../components/AppToast";
import type {
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialProbeResult,
  ConsoleCredentialProbeStatus,
  ConsoleCredentialPoolAutomationProvider,
  ConsoleCredentialPoolAutomationResponse,
  ConsoleCredentialRefillDemand,
  ConsoleCredentialRefillResponse,
  ConsoleGeminiAuthFamily,
  ConsoleGeminiAuthSession,
  ConsoleGeminiGeneratedCredentialDraft,
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
  AccountsLedgerWorkspace,
  type AccountsLedgerPilotAccount,
  type AccountsLedgerPilotSection,
} from "./AccountsLedgerWorkspace";
import { CredentialGroupsWorkspace } from "./CredentialGroupsWorkspace";
import {
  CredentialDialog,
  type CredentialDialogMode,
  type CredentialDialogValue,
} from "./CredentialDialog";
import { DiscardDraftDialog } from "./DiscardDraftDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { RestoreRevisionDialog } from "./RestoreRevisionDialog";
import {
  addExplicitCredential,
  addProviderWithCredential,
  buildCredentialSecretPatches,
  deleteExplicitCredential,
  type CredentialSecretEdit,
  updateExplicitCredential,
} from "./credentialDocument";
import {
  type AccountLedgerRow,
  buildAccountLedgerRows,
  buildCredentialGroupDirectory,
  buildGroupMemberCandidates,
  filterAccountLedgerRows,
  resolveGeminiLogicalChannel,
} from "./accountManagementViewModel";
import {
  PROVIDER_CATALOG_TEMPLATES,
  providerCatalogClassification,
  providerDefinitionFromCatalogDraft,
  type ProviderCatalogDraft,
} from "./providerCatalog";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type BrowserConsoleAppProps = {
  consoleApi?: ConsoleApi;
};

type SecretAccessRecovery = "not-required" | "recovered" | "stale";

type PilotActionDialogState =
  | {
      kind: "probe";
      providerId: string;
      account: AccountsLedgerPilotAccount;
    }
  | {
      kind: "stats";
      providerId: string;
      account: AccountsLedgerPilotAccount;
    }
  | {
      kind: "schedule";
      providerId: string;
      account: AccountsLedgerPilotAccount;
    };

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

type GeminiManualAddDialogState = {
  targetFamily: ConsoleGeminiAuthFamily;
  providerId: string;
  session: ConsoleGeminiAuthSession | null;
  busy: boolean;
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

function duplicateCredentialDialogValue(
  document: ConsoleRouteDocument,
  providerId: string,
  account: AccountsLedgerPilotAccount,
): CredentialDialogValue {
  const copiedValue = credentialDialogValueFromDocument(document, providerId, account.accountId);
  const sourceName = copiedValue?.accountName || account.displayName || account.accountId;
  return {
    providerId,
    credentialId: `${account.accountId}-copy`,
    accountName: `${sourceName} Copy`,
    enabled: account.enabled,
    baseUrl: copiedValue?.baseUrl ?? "",
    supportedModelsText: copiedValue?.supportedModelsText ?? "",
    apiKeyOperation: "replace",
    apiKeyValue: "",
  };
}

function isGeminiAuthSessionTerminal(status: ConsoleGeminiAuthSession["status"]): boolean {
  return status === "succeeded" || status === "failed";
}

function canManuallyCompleteGeminiAuthSession(
  session: ConsoleGeminiAuthSession | null,
): boolean {
  if (!session || session.status !== "waiting_user") {
    return false;
  }
  return (
    session.targetFamily === "gemini-canvas" || session.targetFamily === "gemini-canvas-chat"
  );
}

function mergeCredentialSecretEditEntries(
  current: CredentialSecretEdit[],
  nextEntries: CredentialSecretEdit[],
): CredentialSecretEdit[] {
  const merged = [...current];
  for (const nextEntry of nextEntries) {
    const existingIndex = merged.findIndex(
      (entry) =>
        entry.providerId === nextEntry.providerId &&
        entry.credentialId === nextEntry.credentialId &&
        entry.field === nextEntry.field,
    );
    if (existingIndex >= 0) {
      merged[existingIndex] = nextEntry;
      continue;
    }
    merged.push(nextEntry);
  }
  return merged;
}

function applyGeminiGeneratedDraftsToDocument(
  document: ConsoleRouteDocument,
  generatedDrafts: ConsoleGeminiGeneratedCredentialDraft[],
): {
  document: ConsoleRouteDocument;
  secretEdits: CredentialSecretEdit[];
} {
  let nextDocument = document;
  const secretEdits: CredentialSecretEdit[] = [];

  for (const draft of generatedDrafts) {
    const credentialIdValue = draft.credential.id;
    if (typeof credentialIdValue !== "string" || credentialIdValue.trim().length === 0) {
      throw new Error("Generated Gemini credential is missing a stable id.");
    }
    const credentialId = credentialIdValue.trim();
    const provider = nextDocument.providers.find(
      (entry) => isRecord(entry) && entry.id === draft.providerId,
    );
    if (!isRecord(provider)) {
      throw new Error(`Provider '${draft.providerId}' could not be found.`);
    }

    const updates = { ...draft.credential, id: credentialId };
    const existingCredential =
      Array.isArray(provider.credentials) &&
      provider.credentials.some((entry) => isRecord(entry) && entry.id === credentialId);
    nextDocument = existingCredential
      ? updateExplicitCredential(
          nextDocument,
          {
            providerId: draft.providerId,
            credentialId,
          },
          updates,
        )
      : addExplicitCredential(nextDocument, {
          providerId: draft.providerId,
          credential: updates,
        });

    for (const secretEdit of draft.secretEdits) {
      if (
        (secretEdit.field === "api_key" || secretEdit.field === "auth_token") &&
        secretEdit.operation === "replace"
      ) {
        secretEdits.push({
          providerId: draft.providerId,
          credentialId,
          field: secretEdit.field,
          operation: "replace",
          value: secretEdit.value,
        });
      }
    }
  }

  return { document: nextDocument, secretEdits };
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

type PilotIdentityCategoryDefinition = {
  id: string;
  label: string;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
};

type PilotProviderPolicyDefinition = {
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
};

const DEFAULT_PILOT_POOL_TARGET_SIZE = 30;

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

const CODEX_IDENTITY_CATEGORY_DEFAULTS: PilotIdentityCategoryDefinition[] = [
  {
    id: "free",
    label: "Free",
    poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
    autoRefillEnabled: false,
    autoPruneEnabled: false,
  },
  {
    id: "plus",
    label: "Plus",
    poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
    autoRefillEnabled: false,
    autoPruneEnabled: false,
  },
];

type CodexDemoAccountDefinition = {
  accountId: string;
  displayName: string;
  mode: "credential" | "provider-default";
  enabled: boolean;
  logicalLabels: string[];
  capacityLabel: string;
  statusLabel: string;
  dispatchEnabled: boolean;
  dispatchEditable: boolean;
  previewOnly: boolean;
  usageWindowBadges: string[];
  recentUseLabel: string;
  verificationStatus: "verified" | "failed" | "blocked" | "not-tested";
  verificationFamilies: string[];
  verificationCheckedAt: string | null;
  verificationEvidenceRef: string | null;
  verificationNote: string;
};

const CODEX_DEMO_ACCOUNTS_BY_CATEGORY: Record<
  string,
  CodexDemoAccountDefinition[]
> = {
  free: [
    {
      accountId: "codex-free-demo-a",
      displayName: "Codex Free Demo A",
      mode: "credential",
      enabled: true,
      logicalLabels: ["普通用户"],
      capacityLabel: "1 / 1",
      statusLabel: "正常",
      dispatchEnabled: true,
      dispatchEditable: true,
      previewOnly: true,
      usageWindowBadges: ["18 req", "0", "A $0.00", "U $0.00"],
      recentUseLabel: "12 分钟前",
      verificationStatus: "not-tested",
      verificationFamilies: [],
      verificationCheckedAt: null,
      verificationEvidenceRef: null,
      verificationNote: "演示账号未执行 live semantic canary.",
    },
  ],
  plus: [
    {
      accountId: "codex-plus-demo-a",
      displayName: "Codex Plus Demo A",
      mode: "credential",
      enabled: true,
      logicalLabels: ["VIP 用户"],
      capacityLabel: "2 / 3",
      statusLabel: "正常",
      dispatchEnabled: true,
      dispatchEditable: true,
      previewOnly: true,
      usageWindowBadges: ["32 req", "0", "A $0.00", "U $0.00"],
      recentUseLabel: "2 分钟前",
      verificationStatus: "not-tested",
      verificationFamilies: [],
      verificationCheckedAt: null,
      verificationEvidenceRef: null,
      verificationNote: "演示账号未执行 live semantic canary.",
    },
    {
      accountId: "codex-plus-demo-b",
      displayName: "Codex Plus Demo B",
      mode: "credential",
      enabled: true,
      logicalLabels: ["普通用户"],
      capacityLabel: "1 / 2",
      statusLabel: "正常",
      dispatchEnabled: true,
      dispatchEditable: true,
      previewOnly: true,
      usageWindowBadges: ["9 req", "0", "A $0.00", "U $0.00"],
      recentUseLabel: "8 分钟前",
      verificationStatus: "not-tested",
      verificationFamilies: [],
      verificationCheckedAt: null,
      verificationEvidenceRef: null,
      verificationNote: "演示账号未执行 live semantic canary.",
    },
  ],
};

function findCodexDemoAccount(
  credentialId: string,
): { categoryId: string; account: CodexDemoAccountDefinition } | null {
  for (const [categoryId, accounts] of Object.entries(CODEX_DEMO_ACCOUNTS_BY_CATEGORY)) {
    const account = accounts.find((entry) => entry.accountId === credentialId);
    if (account) {
      return { categoryId, account };
    }
  }
  return null;
}

function materializeCodexDemoCredential(
  document: ConsoleRouteDocument,
  providerId: string,
  credentialId: string,
  nextEnabled: boolean,
): ConsoleRouteDocument | null {
  const demoAccount = findCodexDemoAccount(credentialId);
  if (!demoAccount) {
    return null;
  }
  return addExplicitCredential(document, {
    providerId,
    credential: {
      id: demoAccount.account.accountId,
      account_name: demoAccount.account.displayName,
      enabled: nextEnabled,
      credential_identity_category_id: demoAccount.categoryId,
      preview_capacity: demoAccount.account.capacityLabel,
      preview_status: demoAccount.account.statusLabel,
      preview_usage_window_badges: [...demoAccount.account.usageWindowBadges],
      preview_recent_use: demoAccount.account.recentUseLabel,
      preview_logical_labels: [...demoAccount.account.logicalLabels],
      preview_seeded_demo: true,
    },
  });
}

function defaultPilotCapacityLabel(categoryId: string): string {
  switch (categoryId) {
    case "free":
      return "1 / 1";
    case "plus":
      return "3 / 3";
    default:
      return "2 / 2";
  }
}

function normalizePilotCapacityLabel(value: string | null, fallback: string): string {
  if (!value) {
    return fallback;
  }
  const normalized = value.trim();
  const legacyMatch = normalized.match(/^(\d+)\s*并发$/);
  if (legacyMatch) {
    return `${legacyMatch[1]} / ${legacyMatch[1]}`;
  }
  return normalized;
}

function defaultPilotUsageWindowBadges(categoryId: string): string[] {
  return categoryId === "free"
    ? ["0 req", "0", "A $0.00", "U $0.00"]
    : ["0 req", "0", "A $0.00", "U $0.00"];
}

function normalizePilotPoolTargetSize(
  value: unknown,
  fallback = DEFAULT_PILOT_POOL_TARGET_SIZE,
): number {
  if (typeof value === "number" && Number.isFinite(value) && value >= 1) {
    return Math.floor(value);
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value);
    if (Number.isFinite(parsed) && parsed >= 1) {
      return Math.floor(parsed);
    }
  }
  return fallback;
}

function readPilotProviderPolicy(
  provider: Record<string, unknown>,
): PilotProviderPolicyDefinition {
  return {
    poolTargetSize: normalizePilotPoolTargetSize(provider.pool_target_size),
    autoRefillEnabled: optionalBoolean(provider, "auto_refill_enabled") ?? false,
    autoPruneEnabled: optionalBoolean(provider, "auto_prune_enabled") ?? false,
  };
}

function optionalStringArray(record: Record<string, unknown>, key: string): string[] {
  const value = record[key];
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string" && entry.trim().length > 0)
    .map((entry) => entry.trim());
}

function readPilotUsageWindowBadges(
  categoryId: string,
  credential: Record<string, unknown>,
): string[] {
  const badges = optionalStringArray(credential, "preview_usage_window_badges");
  if (badges.length > 0) {
    return badges;
  }
  const legacyLabel = optionalString(credential, "preview_usage_window");
  if (legacyLabel) {
    return [legacyLabel];
  }
  return defaultPilotUsageWindowBadges(categoryId);
}

function buildPilotPreviewMetrics(
  categoryId: string,
  credential: Record<string, unknown>,
  enabled: boolean,
): {
  capacityLabel: string;
  statusLabel: string;
  dispatchEnabled: boolean;
  usageWindowBadges: string[];
  recentUseLabel: string;
} {
  return {
    capacityLabel: normalizePilotCapacityLabel(
      optionalString(credential, "preview_capacity"),
      defaultPilotCapacityLabel(categoryId),
    ),
    statusLabel: enabled ? optionalString(credential, "preview_status") ?? "正常" : "暂停",
    dispatchEnabled: enabled,
    usageWindowBadges: readPilotUsageWindowBadges(categoryId, credential),
    recentUseLabel: optionalString(credential, "preview_recent_use") ?? "刚刚同步",
  };
}

function buildFlatPilotPreviewMetrics(
  credential: Record<string, unknown> | null,
  enabled: boolean,
): {
  capacityLabel: string;
  statusLabel: string;
  dispatchEnabled: boolean;
  usageWindowBadges: string[];
  recentUseLabel: string;
} {
  const fallbackCapacity = enabled ? "1 / 1" : "0 / 1";
  return {
    capacityLabel: normalizePilotCapacityLabel(
      credential ? optionalString(credential, "preview_capacity") : null,
      fallbackCapacity,
    ),
    statusLabel:
      enabled && credential
        ? optionalString(credential, "preview_status") ?? "正常"
        : enabled
          ? "正常"
          : "暂停",
    dispatchEnabled: enabled,
    usageWindowBadges: credential
      ? readPilotUsageWindowBadges("default", credential)
      : defaultPilotUsageWindowBadges("default"),
    recentUseLabel: credential ? optionalString(credential, "preview_recent_use") ?? "刚刚同步" : "刚刚同步",
  };
}

function buildPilotSectionAccount(
  row: AccountLedgerRow,
  options: {
    credential?: Record<string, unknown> | null;
    categoryId?: string;
    logicalLabels?: string[];
    previewOnly?: boolean;
  } = {},
): AccountsLedgerPilotAccount {
  const credential = options.credential ?? null;
  const logicalLabels = options.logicalLabels ?? row.groupLabels;
  const metrics =
    credential && options.categoryId
      ? buildPilotPreviewMetrics(options.categoryId, credential, row.enabled)
      : buildFlatPilotPreviewMetrics(credential, row.enabled);

  return {
    accountId: row.accountId,
    providerId: row.providerId,
    displayName: row.displayName,
    mode: row.mode,
    enabled: row.enabled,
    logicalLabels: [...logicalLabels],
    dispatchEditable: row.mode === "credential",
    previewOnly: options.previewOnly ?? false,
    verificationStatus: row.verificationStatus,
    verificationFamilies: [...row.verificationFamilies],
    verificationCheckedAt: row.verificationCheckedAt,
    verificationEvidenceRef: row.verificationEvidenceRef,
    verificationNote: row.verificationNote,
    ...metrics,
  };
}

function mergeGeminiAccountLedgerSections(
  sections: AccountsLedgerPilotSection[],
): AccountsLedgerPilotSection[] {
  const passthrough: AccountsLedgerPilotSection[] = [];
  const geminiGroups = new Map<
    string,
    {
      channel: NonNullable<ReturnType<typeof resolveGeminiLogicalChannel>>;
      sections: AccountsLedgerPilotSection[];
    }
  >();

  for (const section of sections) {
    const channel = resolveGeminiLogicalChannel(
      section.providerId,
      section.providerPreset,
      section.providerLabel,
    );
    if (!channel) {
      passthrough.push(section);
      continue;
    }
    const group = geminiGroups.get(channel.key) ?? { channel, sections: [] };
    group.sections.push(section);
    geminiGroups.set(channel.key, group);
  }

  const mergedGeminiSections = [...geminiGroups.values()].map(({ channel, sections: members }) => {
    const primary =
      members.find((section) => section.providerId === channel.primaryProviderId) ?? members[0];
    const accountMap = new Map<string, AccountsLedgerPilotAccount>();
    for (const member of members) {
      const memberAccounts = [
        ...member.directAccounts,
        ...member.identityCategories.flatMap((category) => category.accounts),
      ];
      for (const account of memberAccounts) {
        accountMap.set(`${account.providerId}:${account.accountId}`, account);
      }
    }

    return {
      ...primary,
      providerIds: [...new Set(members.flatMap((section) => section.providerIds))],
      providerLabel: channel.label,
      vendorLabel: "Google / Gemini",
      manualAddFamily: channel.manualAddFamily,
      hasExplicitAccounts: members.some((section) => section.hasExplicitAccounts),
      supportsIdentityCategories: false,
      identityCategories: [],
      directAccounts: [...accountMap.values()].sort((left, right) =>
        left.displayName.localeCompare(right.displayName),
      ),
    } satisfies AccountsLedgerPilotSection;
  });

  return [...passthrough, ...mergedGeminiSections].sort((left, right) => {
    const leftChannel = resolveGeminiLogicalChannel(
      left.providerId,
      left.providerPreset,
      left.providerLabel,
    );
    const rightChannel = resolveGeminiLogicalChannel(
      right.providerId,
      right.providerPreset,
      right.providerLabel,
    );
    if (leftChannel && rightChannel && leftChannel.order !== rightChannel.order) {
      return leftChannel.order - rightChannel.order;
    }
    return left.providerLabel.localeCompare(right.providerLabel);
  });
}

type PilotStatsView = {
  totalStandardCostText: string;
  totalUserCostText: string;
  totalCostText: string;
  totalRequestsText: string;
  avgDailyCostText: string;
  avgDailyRequestsText: string;
  activeDaysText: string;
  activeDaysDenominatorText: string;
  todayRequestsText: string;
  todayTokenText: string;
  totalTokenText: string;
  avgDailyTokenText: string;
  avgResponseText: string;
};

function parsePilotCurrencyBadge(
  badges: string[],
  prefix: "A" | "U",
): number {
  const badge = badges.find((entry) => entry.startsWith(`${prefix} `));
  if (!badge) {
    return 0;
  }
  const match = badge.match(/\$([0-9]+(?:\.[0-9]+)?)/);
  return match ? Number(match[1]) : 0;
}

function parsePilotRequestBadge(badges: string[]): number {
  const badge = badges.find((entry) => /req$/i.test(entry));
  if (!badge) {
    return 0;
  }
  const match = badge.match(/(\d+)/);
  return match ? Number(match[1]) : 0;
}

function formatPilotMoney(value: number): string {
  return `$${value.toFixed(4)}`;
}

function buildPilotStatsView(account: AccountsLedgerPilotAccount): PilotStatsView {
  const totalRequests = parsePilotRequestBadge(account.usageWindowBadges);
  const totalStandardCost = parsePilotCurrencyBadge(account.usageWindowBadges, "A");
  const totalUserCost = parsePilotCurrencyBadge(account.usageWindowBadges, "U");
  const totalCost = totalStandardCost;
  const activeDays = totalRequests > 0 ? 1 : 0;
  const avgDailyCost = activeDays > 0 ? totalCost / activeDays : 0;
  const avgDailyRequests = activeDays > 0 ? totalRequests / activeDays : 0;

  return {
    totalStandardCostText: formatPilotMoney(totalStandardCost),
    totalUserCostText: formatPilotMoney(totalUserCost),
    totalCostText: formatPilotMoney(totalCost),
    totalRequestsText: String(totalRequests),
    avgDailyCostText: formatPilotMoney(avgDailyCost),
    avgDailyRequestsText: String(Math.round(avgDailyRequests)),
    activeDaysText: String(activeDays),
    activeDaysDenominatorText: "31",
    todayRequestsText: activeDays > 0 ? String(totalRequests) : "0",
    todayTokenText: "0",
    totalTokenText: "0",
    avgDailyTokenText: "0",
    avgResponseText: "0ms",
  };
}

function normalizePilotCategoryId(value: string, fallback: string): string {
  const normalized = normalizeBucketKey(value);
  return normalized.length > 0 ? normalized : fallback;
}

function isCodexPilotProvider(providerId: string, providerLabel: string): boolean {
  const normalizedId = providerId.trim().toLowerCase();
  const normalizedLabel = providerLabel.trim().toLowerCase();
  return normalizedId === "codex" || normalizedLabel === "codex";
}

function resolveGeminiManualAddFamily(
  provider: Record<string, unknown>,
  providerId: string,
): ConsoleGeminiAuthFamily | null {
  const candidates = [
    providerId,
    optionalString(provider, "preset"),
    optionalString(provider, "label"),
  ]
    .filter((value): value is string => Boolean(value))
    .map((value) => value.trim().toLowerCase());
  if (candidates.includes("gemini-canvas")) {
    return "gemini-canvas";
  }
  if (candidates.includes("gemini-canvas-chat")) {
    return "gemini-canvas-chat";
  }
  if (candidates.includes("gemini-business")) {
    return "gemini-business";
  }
  if (
    candidates.includes("gemini-web") ||
    candidates.includes("gemini-web-chat") ||
    candidates.includes("gemini-web-chat-modular") ||
    candidates.includes("gemini-web-secondary")
  ) {
    return "gemini-web";
  }
  return null;
}

function readPilotIdentityCategories(
  provider: Record<string, unknown>,
  providerId: string,
  providerLabel: string,
): PilotIdentityCategoryDefinition[] {
  const rawCategories = provider.credential_identity_categories;
  const definitions = Array.isArray(rawCategories)
    ? rawCategories
        .map((entry, index) => {
          if (typeof entry === "string" && entry.trim().length > 0) {
            const label = entry.trim();
            return {
              id: normalizePilotCategoryId(label, `identity-${index + 1}`),
              label,
              poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
              autoRefillEnabled: false,
              autoPruneEnabled: false,
            } satisfies PilotIdentityCategoryDefinition;
          }
          if (!isRecord(entry)) {
            return null;
          }
          const label = optionalString(entry, "label") ?? optionalString(entry, "name");
          if (!label) {
            return null;
          }
          return {
            id: optionalString(entry, "id") ?? normalizePilotCategoryId(label, `identity-${index + 1}`),
            label,
            poolTargetSize: normalizePilotPoolTargetSize(entry.pool_target_size),
            autoRefillEnabled: optionalBoolean(entry, "auto_refill_enabled") ?? false,
            autoPruneEnabled: optionalBoolean(entry, "auto_prune_enabled") ?? false,
          } satisfies PilotIdentityCategoryDefinition;
        })
        .filter(
          (entry): entry is PilotIdentityCategoryDefinition =>
            entry !== null && entry.id.trim().length > 0,
        )
    : [];

  const deduped = definitions.filter(
    (entry, index, collection) => collection.findIndex((candidate) => candidate.id === entry.id) === index,
  );
  if (deduped.length > 0) {
    return deduped;
  }
  return isCodexPilotProvider(providerId, providerLabel)
    ? CODEX_IDENTITY_CATEGORY_DEFAULTS
    : [];
}

function buildAccountLedgerSections(
  document: ConsoleRouteDocument | null,
  rows: AccountLedgerRow[],
): AccountsLedgerPilotSection[] {
  if (!document) {
    return [];
  }

  const visibleRowsByProviderId = new Map<string, AccountLedgerRow[]>();
  for (const row of rows) {
    const providerRows = visibleRowsByProviderId.get(row.providerId) ?? [];
    providerRows.push(row);
    visibleRowsByProviderId.set(row.providerId, providerRows);
  }

  const sections = document.providers
    .filter(isRecord)
    .flatMap((provider) => {
      const providerId = optionalString(provider, "id");
      if (!providerId) {
        return [];
      }
      const providerLabel = optionalString(provider, "label") ?? providerId;
      const visibleRows = visibleRowsByProviderId.get(providerId) ?? [];
      if (visibleRows.length === 0) {
        return [];
      }
      const providerPolicy = readPilotProviderPolicy(provider);
      const manualAddFamily = resolveGeminiManualAddFamily(provider, providerId);
      const providerPreset = optionalString(provider, "preset");
      const providerClassification = providerCatalogClassification(
        providerId,
        providerPreset,
        optionalString(provider, "adapter"),
        optionalString(provider, "protocol_profile"),
      );

      const defaultAccount = visibleRows.find((row) => row.mode === "provider-default") ?? null;
      const explicitRows = visibleRows.filter((row) => row.mode === "credential");
      const credentials = Array.isArray(provider.credentials)
        ? provider.credentials.filter(isRecord)
        : [];
      const credentialRecordById = new Map(
        credentials
          .map((entry) => {
            const id = optionalString(entry, "id");
            return id ? ([id, entry] as const) : null;
          })
          .filter((entry): entry is readonly [string, Record<string, unknown>] => entry !== null),
      );

      const identityDefinitions = readPilotIdentityCategories(provider, providerId, providerLabel);
      const supportsIdentityCategories = identityDefinitions.length > 0;
      const allowSeededDemoAccounts =
        explicitRows.length === 0 ||
        explicitRows.every((row) => {
          const credential = credentialRecordById.get(row.accountId);
          return credential ? optionalBoolean(credential, "preview_seeded_demo") === true : false;
        });
      const identityCategories = identityDefinitions.map((definition) => {
        const accounts = explicitRows
          .filter((row) => {
            const credential = credentialRecordById.get(row.accountId);
            if (!credential) {
              return false;
            }
            return optionalString(credential, "credential_identity_category_id") === definition.id;
          })
          .map((account) => {
            const credential = credentialRecordById.get(account.accountId) ?? {};
            const previewLogicalLabels = optionalStringArray(
              credential,
              "preview_logical_labels",
            );
            return buildPilotSectionAccount(account, {
              credential,
              categoryId: definition.id,
              logicalLabels:
                account.groupLabels.length > 0
                  ? [...account.groupLabels]
                  : [...previewLogicalLabels],
            });
          });
        const seededDemoAccounts =
          allowSeededDemoAccounts && accounts.length === 0
            ? (CODEX_DEMO_ACCOUNTS_BY_CATEGORY[definition.id] ?? []).map((account) => ({
                ...account,
                providerId,
                logicalLabels: [...account.logicalLabels],
              }))
            : [];
        const categoryAccounts = accounts.length > 0 ? accounts : seededDemoAccounts;
        return {
          id: definition.id,
          label: definition.label,
          count: categoryAccounts.length,
          poolTargetSize: definition.poolTargetSize,
          autoRefillEnabled: definition.autoRefillEnabled,
          autoPruneEnabled: definition.autoPruneEnabled,
          accounts: categoryAccounts,
        };
      });

      return [
        {
          providerId,
          providerIds: [providerId],
          providerLabel,
          vendorLabel:
            optionalString(provider, "vendor_name") ??
            optionalString(provider, "vendorName") ??
            providerLabel,
          providerPreset,
          providerCompatibility: providerClassification?.compatibility ?? null,
          hostLabel: hostLabelFromUrl(optionalString(provider, "base_url")),
          defaultAccountId: defaultAccount?.accountId ?? null,
          manualAddFamily,
          hasExplicitAccounts: explicitRows.length > 0,
          poolTargetSize: providerPolicy.poolTargetSize,
          autoRefillEnabled: providerPolicy.autoRefillEnabled,
          autoPruneEnabled: providerPolicy.autoPruneEnabled,
          supportsIdentityCategories,
          identityCategories,
          directAccounts: supportsIdentityCategories
            ? []
            : visibleRows.map((row) =>
                buildPilotSectionAccount(row, {
                  credential:
                    row.mode === "credential"
                      ? credentialRecordById.get(row.accountId) ?? null
                      : null,
                }),
              ),
        } satisfies AccountsLedgerPilotSection,
      ];
    });
  return mergeGeminiAccountLedgerSections(sections);
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

function optionalBoolean(record: Record<string, unknown>, key: string): boolean | null {
  const value = record[key];
  return typeof value === "boolean" ? value : null;
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

export function BrowserConsoleApp(props: BrowserConsoleAppProps) {
  return (
    <NeuroTooltipProvider>
      <BrowserConsoleContent {...props} />
    </NeuroTooltipProvider>
  );
}

function BrowserConsoleContent({ consoleApi }: BrowserConsoleAppProps) {
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
  const [credentialPoolAutomation, setCredentialPoolAutomation] =
    useState<ConsoleCredentialPoolAutomationResponse | null>(null);
  const [credentialPoolAutomationError, setCredentialPoolAutomationError] =
    useState<string | null>(null);
  const [credentialPoolAutomationBusy, setCredentialPoolAutomationBusy] =
    useState<string | null>(null);
  const [credentialRefill, setCredentialRefill] =
    useState<ConsoleCredentialRefillResponse | null>(null);
  const [credentialRefillError, setCredentialRefillError] = useState<string | null>(null);
  const [credentialRefillBusy, setCredentialRefillBusy] = useState<string | null>(null);
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
  const [providerCatalogDialogOpen, setProviderCatalogDialogOpen] = useState(false);
  const [restoreDialogOpen, setRestoreDialogOpen] = useState(false);
  const [discardRefreshDialogOpen, setDiscardRefreshDialogOpen] = useState(false);
  const [geminiManualAddDialogState, setGeminiManualAddDialogState] =
    useState<GeminiManualAddDialogState | null>(null);
  const [pilotActionDialog, setPilotActionDialog] = useState<PilotActionDialogState | null>(null);
  const [pilotProbeModel, setPilotProbeModel] = useState("GPT-5.6 (Sol)");
  const [pilotProbeMode, setPilotProbeMode] = useState("常规请求");
  const [aliasDraftRows, setAliasDraftRows] = useState<AliasDraftRow[]>([]);
  const [modelRouteDraftRows, setModelRouteDraftRows] = useState<ModelRouteDraftRow[]>([]);
  const [providerDraftRows, setProviderDraftRows] = useState<ProviderDraftRow[]>([]);
  const [accountGroupDraftRows, setAccountGroupDraftRows] = useState<AccountGroupDraftRow[]>([]);
  const [activeWorkspace, setActiveWorkspace] = useState<ConsoleWorkspaceId>("overview");
  const [accountSearch, setAccountSearch] = useState("");
  const [accountMembershipFilter, setAccountMembershipFilter] =
    useState<AccountMembershipFilter>("all");
  const [accountEnabledFilter, setAccountEnabledFilter] = useState<AccountEnabledFilter>("all");
  const [selectedAccountGroupFilter, setSelectedAccountGroupFilter] = useState("all");
  const [selectedProviderFilter, setSelectedProviderFilter] = useState("all");
  const [selectedAccountGroupRowId, setSelectedAccountGroupRowId] = useState<string | null>(null);
  const [groupMemberQuery, setGroupMemberQuery] = useState("");
  const [groupMemberMode, setGroupMemberMode] = useState<"all" | "members" | "ungrouped">("all");
  const [credentialProbeBusy, setCredentialProbeBusy] = useState<string | null>(null);
  const [credentialProbeResults, setCredentialProbeResults] = useState<
    Record<string, CredentialProbeViewResult>
  >({});
  const geminiManualAddPollTimeoutRef = useRef<number | null>(null);
  const appliedGeminiManualAddSessionIdsRef = useRef<Set<string>>(new Set());

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
      const credentialPoolAutomationPromise = api
        .getCredentialPoolAutomation(managementToken)
        .then((response) => ({ response, error: null as string | null }))
        .catch((cause: unknown) => ({
          response: null,
          error: cause instanceof Error ? cause.message : String(cause),
        }));
      const credentialRefillPromise = api
        .getCredentialRefill(managementToken)
        .then((response) => ({ response, error: null as string | null }))
        .catch((cause: unknown) => ({
          response: null,
          error: cause instanceof Error ? cause.message : String(cause),
        }));
      const [
        nextRouteConfig,
        nextRevisions,
        nextAccountGroupSummary,
        nextCredentialPoolAutomation,
        nextCredentialRefill,
      ] = await Promise.all([
        api.getRouteConfig(managementToken),
        api.listRouteConfigRevisions(managementToken),
        accountGroupSummaryPromise,
        credentialPoolAutomationPromise,
        credentialRefillPromise,
      ]);
      if (requestId !== refreshRequestIdRef.current) {
        return;
      }
      setValidation(null);
      setRouteConfig({ ...nextRouteConfig });
      setRevisions(nextRevisions);
      setAccountGroupSummary(nextAccountGroupSummary.summary);
      setAccountGroupSummaryError(nextAccountGroupSummary.error);
      setCredentialPoolAutomation(nextCredentialPoolAutomation.response);
      setCredentialPoolAutomationError(nextCredentialPoolAutomation.error);
      setCredentialRefill(nextCredentialRefill.response);
      setCredentialRefillError(nextCredentialRefill.error);
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
    setProviderCatalogDialogOpen(false);
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
  const accountPilotSections = useMemo(
    () =>
      buildAccountLedgerSections(
        draftDocumentState.document,
        filteredAccountLedgerRows,
      ),
    [draftDocumentState.document, filteredAccountLedgerRows],
  );
  const credentialPoolAutomationByProvider = useMemo(
    () =>
      new Map<string, ConsoleCredentialPoolAutomationProvider>(
        credentialPoolAutomation?.automation.providers.map((provider) => [
          provider.providerId,
          provider,
        ]) ?? [],
      ),
    [credentialPoolAutomation],
  );
  const credentialRefillByProvider = useMemo(
    () =>
      new Map<string, ConsoleCredentialRefillDemand>(
        credentialRefill?.refill.providers.map((provider) => [provider.providerId, provider]) ?? [],
      ),
    [credentialRefill],
  );
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
      pilotActionDialog
        ? (displayedAccountsById.get(pilotActionDialog.account.accountId) ?? null)
        : null,
    [displayedAccountsById, pilotActionDialog],
  );
  const activePilotProbeResult = useMemo(
    () =>
      pilotActionDialog
        ? (credentialProbeResults[pilotActionDialog.account.accountId] ?? null)
        : null,
    [credentialProbeResults, pilotActionDialog],
  );
  const activePilotStatsView = useMemo(
    () => (pilotActionDialog ? buildPilotStatsView(pilotActionDialog.account) : null),
    [pilotActionDialog],
  );
  const groupDirectory = useMemo(
    () => buildCredentialGroupDirectory(displayedAccountCatalog, accountGroupDraftRows),
    [accountGroupDraftRows, displayedAccountCatalog],
  );
  const enabledGroupCount = useMemo(
    () => groupDirectory.filter((group) => group.enabled).length,
    [groupDirectory],
  );
  const effectiveSelectedAccountGroupRowId = selectedAccountGroupRowId ?? groupDirectory[0]?.rowId ?? null;
  const selectedAccountGroupDraft = useMemo(
    () =>
      accountGroupDraftRows.find((row) => row.id === effectiveSelectedAccountGroupRowId) ?? null,
    [accountGroupDraftRows, effectiveSelectedAccountGroupRowId],
  );
  const selectedGroupMemberCandidates = useMemo(
    () =>
      buildGroupMemberCandidates(displayedAccountCatalog.accounts, {
        selectedCredentialIds: selectedAccountGroupDraft?.providerCredentialIds ?? [],
        query: groupMemberQuery,
        mode: groupMemberMode,
      }),
    [displayedAccountCatalog.accounts, groupMemberMode, groupMemberQuery, selectedAccountGroupDraft],
  );
  const selectedGroupIdInvalid = selectedAccountGroupDraft
    ? accountGroupDraftNeedsId(selectedAccountGroupDraft)
    : false;
  const selectedGroupBillingInvalid = selectedAccountGroupDraft
    ? parseAccountGroupBillingMultiplier(selectedAccountGroupDraft.billingMultiplier) === null
    : false;

  useEffect(() => {
    if (groupDirectory.length === 0) {
      if (selectedAccountGroupRowId !== null) {
        setSelectedAccountGroupRowId(null);
      }
      return;
    }
    if (
      selectedAccountGroupRowId === null ||
      !groupDirectory.some((group) => group.rowId === selectedAccountGroupRowId)
    ) {
      setSelectedAccountGroupRowId(groupDirectory[0]?.rowId ?? null);
    }
  }, [groupDirectory, selectedAccountGroupRowId]);
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

  const persistGeminiGeneratedDrafts = useCallback(
    async (
      document: ConsoleRouteDocument,
      secretEdits: CredentialSecretEdit[],
      sessionId: string,
      targetFamily: ConsoleGeminiAuthFamily,
      generatedDraftCount: number,
    ) => {
      if (
        targetFamily !== "gemini-canvas" &&
        targetFamily !== "gemini-canvas-chat" &&
        targetFamily !== "gemini-web"
      ) {
        return false;
      }
      if (!managementToken || !routeConfig) {
        return false;
      }

      const commitSecretPatches = buildCredentialSecretPatches({
        activeDocument: routeConfig.routeConfig.document,
        draftDocument: document,
        activeSecrets: routeConfig.routeConfig.secrets,
        activeSecretPatches,
        credentialSecretEdits: secretEdits,
      });
      const requiresSecretGrant = commitSecretPatches.some(
        (patch) => patch.operation !== "keep",
      );
      if (requiresSecretGrant) {
        return false;
      }

      const actionRequest = beginConsoleActionRequest(managementToken);
      setActionBusy("save");
      try {
        const draft = buildCommitRequest(
          document,
          `Import Gemini manual credentials (${sessionId})`,
          commitSecretPatches,
        );
        const result = await actionRequest.api.commitRouteConfig(
          actionRequest.managementToken,
          draft,
        );
        if (!isConsoleActionRequestCurrent(actionRequest)) {
          return true;
        }
        setRouteConfig({ routeConfig: result.routeConfig });
        setValidation(null);
        setError(null);
        pushAppToast(
          "success",
          t(
            `Gemini 凭证已导入并保存为激活修订 ${result.routeConfig.revision.id}（${generatedDraftCount} 条）。`,
            `Gemini credentials were imported and saved as active revision ${result.routeConfig.revision.id} (${generatedDraftCount} entries).`,
          ),
        );
        await refresh();
        return true;
      } catch (cause) {
        const secretRecovery = handleSecretAccessRequiredError(
          cause,
          () => isConsoleActionRecoveryCurrent(actionRequest),
        );
        if (secretRecovery !== "not-required") {
          return false;
        }
        if (!isConsoleActionRequestCurrent(actionRequest)) {
          return true;
        }
        setError(cause instanceof Error ? cause.message : String(cause));
        return false;
      } finally {
        if (actionRequest.generation === consoleActionGenerationRef.current) {
          setActionBusy(null);
        }
      }
    },
    [
      activeSecretPatches,
      beginConsoleActionRequest,
      buildCommitRequest,
      handleSecretAccessRequiredError,
      isConsoleActionRecoveryCurrent,
      isConsoleActionRequestCurrent,
      managementToken,
      refresh,
      routeConfig,
      t,
    ],
  );

  const applyGeminiManualAddSessionResult = useCallback(
    (nextSession: ConsoleGeminiAuthSession) => {
      if (nextSession.status !== "succeeded") {
        return;
      }
      if (appliedGeminiManualAddSessionIdsRef.current.has(nextSession.id)) {
        return;
      }
      if (nextSession.generatedDrafts.length === 0) {
        appliedGeminiManualAddSessionIdsRef.current.add(nextSession.id);
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
        const generated = applyGeminiGeneratedDraftsToDocument(document, nextSession.generatedDrafts);
        const mergedSecretEdits = mergeCredentialSecretEditEntries(
          credentialSecretEdits,
          generated.secretEdits,
        );
        setCredentialSecretEdits((current) =>
          mergeCredentialSecretEditEntries(current, generated.secretEdits),
        );
        replaceEditorDocument(generated.document, true);
        appliedGeminiManualAddSessionIdsRef.current.add(nextSession.id);
        setError(null);
        void persistGeminiGeneratedDrafts(
          generated.document,
          mergedSecretEdits,
          nextSession.id,
          nextSession.targetFamily,
          nextSession.generatedDrafts.length,
        ).then((autoPersisted) => {
          if (autoPersisted) {
            return;
          }
          pushAppToast(
            "info",
            t(
              `Gemini 凭证已写入草稿（${nextSession.generatedDrafts.length} 条），保存路由配置后生效。`,
              `Gemini credentials were added to the draft (${nextSession.generatedDrafts.length} entries). Save the route config to apply them.`,
            ),
          );
        });
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [credentialSecretEdits, editorText, persistGeminiGeneratedDrafts, replaceEditorDocument, t],
  );

  const refreshGeminiManualAddSession = useCallback(
    async (sessionId: string) => {
      if (!managementToken) {
        return;
      }
      const response = await api.getGeminiAuthSession(managementToken, sessionId);
      setGeminiManualAddDialogState((current) => {
        if (!current || current.session?.id !== sessionId) {
          return current;
        }
        return {
          ...current,
          busy: false,
          session: response.session,
        };
      });
      applyGeminiManualAddSessionResult(response.session);
    },
    [api, applyGeminiManualAddSessionResult, managementToken],
  );

  const requestGeminiManualAddCompletion = useCallback(async () => {
    const session = geminiManualAddDialogState?.session ?? null;
    if (!managementToken || !session || !canManuallyCompleteGeminiAuthSession(session)) {
      return;
    }
    const sessionId = session.id;
    setError(null);
    setGeminiManualAddDialogState((current) =>
      current
        ? {
            ...current,
            busy: true,
          }
        : current,
    );

    try {
      const response = await api.completeGeminiAuthSession(managementToken, sessionId);
      setGeminiManualAddDialogState((current) => {
        if (!current || current.session?.id !== sessionId) {
          return current;
        }
        return {
          ...current,
          busy: false,
          session: response.session,
        };
      });
      if (isGeminiAuthSessionTerminal(response.session.status)) {
        applyGeminiManualAddSessionResult(response.session);
      } else {
        void refreshGeminiManualAddSession(response.session.id);
      }
    } catch (cause) {
      setGeminiManualAddDialogState((current) =>
        current
          ? {
              ...current,
              busy: false,
            }
          : current,
      );
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [
    api,
    applyGeminiManualAddSessionResult,
    geminiManualAddDialogState?.session,
    managementToken,
    refreshGeminiManualAddSession,
    setError,
    t,
  ]);

  const closeGeminiManualAddDialog = useCallback(() => {
    if (geminiManualAddPollTimeoutRef.current !== null) {
      window.clearTimeout(geminiManualAddPollTimeoutRef.current);
      geminiManualAddPollTimeoutRef.current = null;
    }
    setGeminiManualAddDialogState(null);
  }, []);

  const openGeminiManualAddDialog = useCallback(
    async (targetFamily: ConsoleGeminiAuthFamily, providerId: string) => {
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
      }

      if (geminiManualAddPollTimeoutRef.current !== null) {
        window.clearTimeout(geminiManualAddPollTimeoutRef.current);
        geminiManualAddPollTimeoutRef.current = null;
      }
      setError(null);
      setGeminiManualAddDialogState({
        targetFamily,
        providerId,
        session: null,
        busy: true,
      });

      try {
        const response = await api.createGeminiAuthSession(managementToken, {
          targetFamily,
          providerId,
        });
        setGeminiManualAddDialogState({
          targetFamily,
          providerId,
          session: response.session,
          busy: false,
        });
        if (isGeminiAuthSessionTerminal(response.session.status)) {
          applyGeminiManualAddSessionResult(response.session);
        } else {
          void refreshGeminiManualAddSession(response.session.id);
        }
      } catch (cause) {
        setGeminiManualAddDialogState(null);
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [api, applyGeminiManualAddSessionResult, managementToken, refreshGeminiManualAddSession, t],
  );

  useEffect(() => {
    if (geminiManualAddPollTimeoutRef.current !== null) {
      window.clearTimeout(geminiManualAddPollTimeoutRef.current);
      geminiManualAddPollTimeoutRef.current = null;
    }
    const sessionId = geminiManualAddDialogState?.session?.id;
    const sessionStatus = geminiManualAddDialogState?.session?.status;
    if (!sessionId || !sessionStatus || isGeminiAuthSessionTerminal(sessionStatus)) {
      return;
    }
    geminiManualAddPollTimeoutRef.current = window.setTimeout(() => {
      geminiManualAddPollTimeoutRef.current = null;
      void refreshGeminiManualAddSession(sessionId);
    }, 1500);
    return () => {
      if (geminiManualAddPollTimeoutRef.current !== null) {
        window.clearTimeout(geminiManualAddPollTimeoutRef.current);
        geminiManualAddPollTimeoutRef.current = null;
      }
    };
  }, [
    geminiManualAddDialogState?.session?.id,
    geminiManualAddDialogState?.session?.status,
    geminiManualAddDialogState?.session?.updatedAt,
    refreshGeminiManualAddSession,
  ]);

  const handleAddIdentityCategory = useCallback(
    (providerId: string) => {
      const requestedLabel = window.prompt(
        t("输入新的账号类别名称。", "Enter the new identity class label."),
      );
      const label = requestedLabel?.trim() ?? "";
      if (label.length === 0) {
        return;
      }

      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find(
        (entry) => isRecord(entry) && entry.id === providerId,
      );
      if (!isRecord(provider)) {
        setError(
          t(
            `找不到服务商 ${providerId}。`,
            `Provider ${providerId} could not be found.`,
          ),
        );
        return;
      }

      const providerLabel = optionalString(provider, "label") ?? providerId;
      const existingCategories = readPilotIdentityCategories(provider, providerId, providerLabel);
      const existingIds = new Set(existingCategories.map((category) => category.id));
      const baseId = normalizePilotCategoryId(label, `identity-${existingCategories.length + 1}`);
      let nextId = baseId;
      let suffix = 2;
      while (existingIds.has(nextId)) {
        nextId = `${baseId}-${suffix}`;
        suffix += 1;
      }

      provider.credential_identity_categories = [
        ...existingCategories,
        {
          id: nextId,
          label,
          poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
          autoRefillEnabled: false,
          autoPruneEnabled: false,
        },
      ];

      replaceEditorDocument(document, true);
      setError(null);
      pushAppToast(
        "success",
        t(
          `服务商 ${providerLabel} 已新增账号类别 ${label}。`,
          `Provider ${providerLabel} now includes the identity class ${label}.`,
        ),
      );
    },
    [editorText, replaceEditorDocument, t],
  );

  const updatePilotIdentityCategoryPolicy = useCallback(
    (
      providerId: string,
      categoryId: string,
      updates: Partial<
        Pick<
          PilotIdentityCategoryDefinition,
          "poolTargetSize" | "autoRefillEnabled" | "autoPruneEnabled"
        >
      >,
    ) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find(
        (entry) => isRecord(entry) && entry.id === providerId,
      );
      if (!isRecord(provider)) {
        setError(
          t(
            `找不到服务商 ${providerId}。`,
            `Provider ${providerId} could not be found.`,
          ),
        );
        return;
      }

      const providerLabel = optionalString(provider, "label") ?? providerId;
      const existingCategories = readPilotIdentityCategories(provider, providerId, providerLabel);
      const nextCategories = existingCategories.map((category) =>
        category.id === categoryId
          ? {
              ...category,
              ...updates,
              poolTargetSize:
                updates.poolTargetSize !== undefined
                  ? Math.max(1, Math.floor(updates.poolTargetSize))
                  : category.poolTargetSize,
            }
          : category,
      );

      provider.credential_identity_categories = nextCategories.map((category) => ({
        id: category.id,
        label: category.label,
        pool_target_size: category.poolTargetSize,
        auto_refill_enabled: category.autoRefillEnabled,
        auto_prune_enabled: category.autoPruneEnabled,
      }));
      if (updates.autoRefillEnabled === true) {
        provider.auto_refill_enabled = true;
      }
      if (updates.autoPruneEnabled === true) {
        provider.auto_prune_enabled = true;
      }

      replaceEditorDocument(document, true);
      setError(null);
    },
    [editorText, replaceEditorDocument, t],
  );

  const updatePilotProviderPolicy = useCallback(
    (
      providerId: string,
      updates: Partial<
        Pick<
          PilotProviderPolicyDefinition,
          "poolTargetSize" | "autoRefillEnabled" | "autoPruneEnabled"
        >
      >,
    ) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      const provider = document.providers.find(
        (entry) => isRecord(entry) && entry.id === providerId,
      );
      if (!isRecord(provider)) {
        setError(
          t(
            `找不到服务商 ${providerId}。`,
            `Provider ${providerId} could not be found.`,
          ),
        );
        return;
      }

      const currentPolicy = readPilotProviderPolicy(provider);
      provider.pool_target_size =
        updates.poolTargetSize !== undefined
          ? Math.max(1, Math.floor(updates.poolTargetSize))
          : currentPolicy.poolTargetSize;
      provider.auto_refill_enabled =
        updates.autoRefillEnabled !== undefined
          ? updates.autoRefillEnabled
          : currentPolicy.autoRefillEnabled;
      provider.auto_prune_enabled =
        updates.autoPruneEnabled !== undefined
          ? updates.autoPruneEnabled
          : currentPolicy.autoPruneEnabled;

      replaceEditorDocument(document, true);
      setError(null);
    },
    [editorText, replaceEditorDocument, t],
  );

  const updateProviderAutomationToggle = useCallback(
    (
      providerId: string,
      field: "autoRefillEnabled" | "autoPruneEnabled",
      nextEnabled: boolean,
    ) => {
      const automation = credentialPoolAutomationByProvider.get(providerId);
      const refillQueueAvailable = credentialRefill?.refill.enabled === true;
      const requiresDirectDriver = field === "autoPruneEnabled";
      if (
        nextEnabled &&
        credentialPoolAutomation &&
        !automation?.driverConfigured &&
        (requiresDirectDriver || !refillQueueAvailable)
      ) {
        pushAppToast(
          "warning",
          t(
            requiresDirectDriver
              ? `渠道 ${providerId} 尚未配置受信任的自动剔号驱动器。`
              : `渠道 ${providerId} 尚未配置补号队列或受信任驱动器。`,
            requiresDirectDriver
              ? `Provider ${providerId} does not have a trusted prune driver configured.`
              : `Provider ${providerId} has neither a refill queue nor a trusted driver.`,
          ),
        );
        return;
      }
      updatePilotProviderPolicy(providerId, { [field]: nextEnabled });
    },
    [
      credentialPoolAutomation,
      credentialPoolAutomationByProvider,
      credentialRefill,
      t,
      updatePilotProviderPolicy,
    ],
  );

  const updateIdentityCategoryAutomationToggle = useCallback(
    (
      providerId: string,
      categoryId: string,
      field: "autoRefillEnabled" | "autoPruneEnabled",
      nextEnabled: boolean,
    ) => {
      const automation = credentialPoolAutomationByProvider.get(providerId);
      const refillQueueAvailable = credentialRefill?.refill.enabled === true;
      const requiresDirectDriver = field === "autoPruneEnabled";
      if (
        nextEnabled &&
        credentialPoolAutomation &&
        !automation?.driverConfigured &&
        (requiresDirectDriver || !refillQueueAvailable)
      ) {
        pushAppToast(
          "warning",
          t(
            requiresDirectDriver
              ? `渠道 ${providerId} 尚未配置受信任的自动剔号驱动器。`
              : `渠道 ${providerId} 尚未配置补号队列或受信任驱动器。`,
            requiresDirectDriver
              ? `Provider ${providerId} does not have a trusted prune driver configured.`
              : `Provider ${providerId} has neither a refill queue nor a trusted driver.`,
          ),
        );
        return;
      }
      updatePilotIdentityCategoryPolicy(providerId, categoryId, { [field]: nextEnabled });
    },
    [
      credentialPoolAutomation,
      credentialPoolAutomationByProvider,
      credentialRefill,
      t,
      updatePilotIdentityCategoryPolicy,
    ],
  );

  const handleRunCredentialPoolAutomation = useCallback(
    async (providerId: string) => {
      if (!managementToken) {
        const message = t(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        );
        setError(message);
        pushAppToast("error", message);
        return;
      }
      if (draftDirty) {
        pushAppToast(
          "warning",
          t(
            "请先保存当前路由草稿，再运行自动补号/剔号。",
            "Save the current route draft before running pool automation.",
          ),
        );
        return;
      }
      const automation = credentialPoolAutomationByProvider.get(providerId);
      if (!automation?.driverConfigured) {
        pushAppToast(
          "warning",
          t(
            `渠道 ${providerId} 尚未配置受信任的自动补号驱动器。`,
            `Provider ${providerId} does not have a trusted automation driver configured.`,
          ),
        );
        return;
      }
      setError(null);
      setCredentialPoolAutomationBusy(providerId);
      try {
        const result = await api.runCredentialPoolAutomation(managementToken, providerId);
        setCredentialPoolAutomation((current) =>
          current
            ? {
                automation: {
                  ...current.automation,
                  providers: current.automation.providers.map((provider) =>
                    provider.providerId === providerId ? result.provider : provider,
                  ),
                },
              }
            : current,
        );
        pushAppToast(
          "success",
          result.provider.message ??
            t(
              `渠道 ${providerId} 的凭证池自动化已完成。`,
              `Credential pool automation completed for ${providerId}.`,
            ),
        );
        await refresh();
      } catch (cause) {
        const message = cause instanceof Error ? cause.message : String(cause);
        setError(message);
        pushAppToast("error", message);
      } finally {
        setCredentialPoolAutomationBusy(null);
      }
    },
    [api, credentialPoolAutomationByProvider, draftDirty, managementToken, refresh, t],
  );

  const handleRequestCredentialRefill = useCallback(
    async (providerId: string) => {
      if (!managementToken) {
        const message = t(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        );
        setError(message);
        pushAppToast("error", message);
        return;
      }
      if (draftDirty) {
        pushAppToast(
          "warning",
          t(
            "请先保存当前路由草稿，再发起主动补号。",
            "Save the current route draft before requesting a refill.",
          ),
        );
        return;
      }
      const demand = credentialRefillByProvider.get(providerId);
      if (!demand?.userRequestEnabled) {
        pushAppToast(
          "warning",
          t("当前补号任务框架不可用。", "The credential refill framework is unavailable."),
        );
        return;
      }
      setError(null);
      setCredentialRefillBusy(providerId);
      try {
        const result = await api.requestCredentialRefill(managementToken, providerId);
        pushAppToast(
          result.created ? "success" : "info",
          result.created
            ? t(
                `渠道 ${providerId} 的主动补号任务已投递。`,
                `A user-requested refill task was published for ${providerId}.`,
              )
            : t(
                `渠道 ${providerId} 已有未完成的补号任务。`,
                `Provider ${providerId} already has an outstanding refill task.`,
              ),
        );
        await refresh();
      } catch (cause) {
        const message = cause instanceof Error ? cause.message : String(cause);
        setError(message);
        pushAppToast("error", message);
      } finally {
        setCredentialRefillBusy(null);
      }
    },
    [api, credentialRefillByProvider, draftDirty, managementToken, refresh, t],
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
        pushAppToast(
          "info",
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

  const applyProviderCatalogDraft = useCallback(
    (value: ProviderCatalogDraft) => {
      const template = PROVIDER_CATALOG_TEMPLATES.find(
        (candidate) => candidate.id === value.templateId,
      );
      if (!template) {
        setError(
          t(
            `找不到服务商模板 ${value.templateId}。`,
            `Provider template ${value.templateId} could not be found.`,
          ),
        );
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
        const provider = providerDefinitionFromCatalogDraft(template, value);
        const nextDocument = addProviderWithCredential(document, {
          provider,
          credential: {
            id: value.credentialId,
            account_name: value.accountName || undefined,
            enabled: true,
            supported_models: [...value.supportedModels],
          },
          routePatterns: value.supportedModels,
        });
        updateCredentialSecretEdit({
          providerId: value.providerId,
          credentialId: value.credentialId,
          accountName: value.accountName,
          enabled: true,
          baseUrl: "",
          supportedModelsText: value.supportedModels.join("\n"),
          apiKeyOperation: "replace",
          apiKeyValue: value.apiKey,
        });
        replaceEditorDocument(nextDocument, true);
        setActiveWorkspace("accounts");
        setError(null);
        pushAppToast(
          "info",
          t(
            `服务商 ${value.providerLabel}、首个账号和 ${value.supportedModels.length} 条模型聚合路由已写入草稿。保存路由配置后生效。`,
            `Provider ${value.providerLabel}, its first account, and ${value.supportedModels.length} model aggregation routes were added to the draft. Save the route config to apply them.`,
          ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [editorText, replaceEditorDocument, t, updateCredentialSecretEdit],
  );

  const togglePilotCredentialDispatch = useCallback(
    (providerId: string, credentialId: string, nextEnabled: boolean) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      try {
        const nextDocument =
          credentialDialogValueFromDocument(document, providerId, credentialId) !== null
            ? updateExplicitCredential(
                document,
                {
                  providerId,
                  credentialId,
                },
                {
                  enabled: nextEnabled,
                },
              )
            : materializeCodexDemoCredential(
                document,
                providerId,
                credentialId,
                nextEnabled,
              ) ??
              updateExplicitCredential(
                document,
                {
                  providerId,
                  credentialId,
                },
                {
                  enabled: nextEnabled,
                },
              );
        replaceEditorDocument(nextDocument, true);
        setError(null);
        pushAppToast(
          "info",
          nextEnabled
            ? t(
                `账号 ${credentialId} 已恢复调度，保存路由配置后生效。`,
                `Account ${credentialId} was resumed in the draft and will take effect after saving the route config.`,
              )
            : t(
                `账号 ${credentialId} 已暂停调度，保存路由配置后生效。`,
                `Account ${credentialId} was paused in the draft and will take effect after saving the route config.`,
              ),
        );
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [editorText, replaceEditorDocument, t],
  );

  const openPilotProbeDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      setPilotProbeModel("GPT-5.6 (Sol)");
      setPilotProbeMode(t("常规请求", "Normal request"));
      setPilotActionDialog({
        kind: "probe",
        providerId,
        account,
      });
    },
    [t],
  );

  const openPilotStatsDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      setPilotActionDialog({
        kind: "stats",
        providerId,
        account,
      });
    },
    [],
  );

  const openPilotScheduleDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      setPilotActionDialog({
        kind: "schedule",
        providerId,
        account,
      });
    },
    [],
  );

  const openDuplicateCredentialDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      setCredentialDialogState({
        mode: "add",
        initialValue: duplicateCredentialDialogValue(document, providerId, account),
      });
    },
    [editorText, t],
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
        pushAppToast(
          "info",
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

  const startPilotProbe = useCallback(async () => {
    if (pilotActionDialog?.kind !== "probe" || !activePilotManagedAccount) {
      return;
    }
    await handleCredentialProbe(activePilotManagedAccount);
  }, [activePilotManagedAccount, handleCredentialProbe, pilotActionDialog]);

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
    const nextRow = createAccountGroupDraftRow();
    applyAccountGroupDraftRows([...accountGroupDraftRows, nextRow]);
    setSelectedAccountGroupRowId(nextRow.id);
    setGroupMemberQuery("");
    setGroupMemberMode("all");
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
      const nextRows = accountGroupDraftRows.filter((row) => row.id !== rowId);
      applyAccountGroupDraftRows(nextRows);
      setSelectedAccountGroupRowId((current) =>
        current === rowId ? nextRows[0]?.id ?? null : current,
      );
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
      pushAppToast(
        "success",
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
      pushAppToast(
        "success",
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
      label: t("凭证分组", "Credential Groups"),
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
                <li className="nt-secret-entry" key={secret.path}>
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
                        aria-label={t(`保留 ${secret.path}`, `Keep ${secret.path}`)}
                        aria-pressed={draft.operation === "keep"}
                        disabled={editorLocked}
                        onClick={() =>
                          setSecretDrafts((current) => ({
                            ...current,
                            [secret.path]: { operation: "keep", value: "" },
                          }))
                        }
                      >
                        {t("保留", "Keep")}
                      </button>
                      <button
                        className={`nt-btn${draft.operation === "replace" ? " nt-btn--primary" : " nt-btn--outline"}`}
                        type="button"
                        aria-label={t(`替换 ${secret.path}`, `Replace ${secret.path}`)}
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
                        {t("替换", "Replace")}
                      </button>
                      <button
                        className={`nt-btn${draft.operation === "clear" ? " nt-btn--primary" : " nt-btn--outline"}`}
                        type="button"
                        aria-label={t(`清空 ${secret.path}`, `Clear ${secret.path}`)}
                        aria-pressed={draft.operation === "clear"}
                        disabled={!hasSecretAccess || editorLocked}
                        onClick={() =>
                          setSecretDrafts((current) => ({
                            ...current,
                            [secret.path]: { operation: "clear", value: "" },
                          }))
                        }
                      >
                        {t("清空", "Clear")}
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
              disabled={editorLocked}
              onClick={() => setProviderCatalogDialogOpen(true)}
            >
              {t("添加服务商", "Add provider")}
            </button>
            <button
              className="nt-btn nt-btn--outline"
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
    <AccountsLedgerWorkspace
      t={t}
      notice={
        <>
          {draftStructureNotice}
          {credentialPoolAutomationError ? (
            <div className="nt-alert nt-alert--warning" role="status">
              <span>
                {t(
                  `自动补号状态暂时不可用：${credentialPoolAutomationError}`,
                  `Pool automation status is temporarily unavailable: ${credentialPoolAutomationError}`,
                )}
              </span>
            </div>
          ) : null}
          {credentialRefillError ? (
            <div className="nt-alert nt-alert--warning" role="status">
              <span>
                {t(
                  `补号任务状态暂时不可用：${credentialRefillError}`,
                  `Credential refill task status is temporarily unavailable: ${credentialRefillError}`,
                )}
              </span>
            </div>
          ) : null}
        </>
      }
      editorLocked={editorLocked}
      totalAccounts={displayedAccountCatalog.accounts.length}
      totalGroups={displayedAccountCatalog.groups.length}
      ungroupedCount={displayedAccountCatalog.ungroupedCount}
      providerCount={accountLedgerProviderOptions.length - 1}
      visibleCount={filteredAccountLedgerRows.length}
      rows={filteredAccountLedgerRows}
      query={accountSearch}
      membership={accountMembershipFilter}
      enabled={accountEnabledFilter}
      selectedGroupId={selectedAccountGroupFilter}
      selectedProviderKey={selectedProviderFilter}
      groupOptions={accountLedgerGroupOptions}
      providerOptions={accountLedgerProviderOptions}
      pilotSections={accountPilotSections}
      automationByProvider={credentialPoolAutomationByProvider}
      automationBusyProviderId={credentialPoolAutomationBusy}
      refillByProvider={credentialRefillByProvider}
      refillBusyProviderId={credentialRefillBusy}
      onQueryChange={setAccountSearch}
      onMembershipChange={setAccountMembershipFilter}
      onEnabledChange={setAccountEnabledFilter}
      onGroupChange={setSelectedAccountGroupFilter}
      onProviderChange={setSelectedProviderFilter}
      onOpenAddProvider={() => setProviderCatalogDialogOpen(true)}
      onOpenAddAccount={() => openAddCredentialDialog()}
      onOpenGroups={() => setActiveWorkspace("groups")}
      onBackToEditor={() => setActiveWorkspace("editor")}
      onAddIdentityCategory={handleAddIdentityCategory}
      onOpenGeminiManualAdd={openGeminiManualAddDialog}
      onEdit={openEditCredentialDialog}
      onRemove={removeCredential}
      onAddExplicit={(providerId) => openAddCredentialDialog(providerId)}
      onToggleDispatch={togglePilotCredentialDispatch}
      onUpdatePoolTargetSize={(providerId, categoryId, nextTargetSize) =>
        updatePilotIdentityCategoryPolicy(providerId, categoryId, {
          poolTargetSize: nextTargetSize,
        })
      }
      onToggleAutoRefill={(providerId, categoryId, nextEnabled) =>
        updateIdentityCategoryAutomationToggle(
          providerId,
          categoryId,
          "autoRefillEnabled",
          nextEnabled,
        )
      }
      onToggleAutoPrune={(providerId, categoryId, nextEnabled) =>
        updateIdentityCategoryAutomationToggle(
          providerId,
          categoryId,
          "autoPruneEnabled",
          nextEnabled,
        )
      }
      onUpdateProviderPoolTargetSize={(providerId, nextTargetSize) =>
        updatePilotProviderPolicy(providerId, {
          poolTargetSize: nextTargetSize,
        })
      }
      onToggleProviderAutoRefill={(providerId, nextEnabled) =>
        updateProviderAutomationToggle(providerId, "autoRefillEnabled", nextEnabled)
      }
      onToggleProviderAutoPrune={(providerId, nextEnabled) =>
        updateProviderAutomationToggle(providerId, "autoPruneEnabled", nextEnabled)
      }
      onRunProviderAutomation={(providerId) =>
        void handleRunCredentialPoolAutomation(providerId)
      }
      onRequestProviderRefill={(providerId) => void handleRequestCredentialRefill(providerId)}
      onOpenProbe={openPilotProbeDialog}
      onOpenStats={openPilotStatsDialog}
      onOpenSchedule={openPilotScheduleDialog}
      onDuplicate={openDuplicateCredentialDialog}
    />
  );
  const groupsWorkspace = (
    <CredentialGroupsWorkspace
      t={t}
      notice={draftStructureNotice}
      editorLocked={editorLocked}
      totalAccounts={displayedAccountCatalog.accounts.length}
      totalGroups={groupDirectory.length}
      enabledGroupCount={enabledGroupCount}
      ungroupedCount={displayedAccountCatalog.ungroupedCount}
      groups={groupDirectory}
      selectedGroupRowId={effectiveSelectedAccountGroupRowId}
      selectedGroup={selectedAccountGroupDraft}
      selectedGroupIdInvalid={selectedGroupIdInvalid}
      selectedGroupBillingInvalid={selectedGroupBillingInvalid}
      memberCandidates={selectedGroupMemberCandidates}
      memberQuery={groupMemberQuery}
      memberMode={groupMemberMode}
      onSelectGroup={setSelectedAccountGroupRowId}
      onAddGroup={addAccountGroupRow}
      onBackToAccounts={() => setActiveWorkspace("accounts")}
      onUpdateField={updateAccountGroupRow}
      onToggleEnabled={updateAccountGroupEnabled}
      onRemoveGroup={removeAccountGroupRow}
      onMemberQueryChange={setGroupMemberQuery}
      onMemberModeChange={setGroupMemberMode}
      onToggleMember={toggleAccountGroupMember}
    />
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
  const primaryAdminWorkspace = activeWorkspace === "accounts" || activeWorkspace === "groups";

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
            <div className="nt-rail__footer-actions">
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
          </div>
        </aside>

        <section className="nt-board nt-board--browser-console">
          {error ? (
            <div className="nt-alert nt-alert--danger" role="alert">
              <span>{error}</span>
            </div>
          ) : null}

          {routeConfig ? (
            <>
              {!primaryAdminWorkspace ? (
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
              ) : null}

              <section className="nt-stage">
                {activeDiagnosticsFeedback}
                {accountSummaryFeedback}
                <section
                  className="nt-workspace-action-bar"
                  aria-label={t("草稿操作", "Draft actions")}
                >
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
                  <div className="nt-actions nt-actions--right">
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
                </section>
                {primaryAdminWorkspace ? (
                  <section className="nt-workspace-ops">
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
                  </section>
                ) : (
                  <article
                    className="nt-card nt-card--panel nt-workspace-toolbar"
                    aria-label={t("工作区操作", "Workspace actions")}
                  >
                    <h2 className="nt-visually-hidden">
                      {workspaceItems.find((item) => item.id === activeWorkspace)?.label}
                    </h2>

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
                )}

                {workspaceContent}
              </section>
            </>
          ) : null}
        </section>
      </main>
      <ProviderCatalogDialog
        open={providerCatalogDialogOpen}
        existingProviderIds={credentialProviderOptions.map((provider) => provider.id)}
        existingCredentialIds={explicitCredentialIds}
        locked={editorLocked}
        hasSecretAccess={hasSecretAccess}
        onOpenChange={setProviderCatalogDialogOpen}
        onRequestSecretAccess={() => setSecretDialogOpen(true)}
        onAddAccount={openAddCredentialDialog}
        onSubmit={applyProviderCatalogDraft}
      />
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
      {geminiManualAddDialogState ? (
        <>
          <div className="dialog-overlay" onClick={closeGeminiManualAddDialog} />
          <section
            className="dialog-content nt-pilot-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={t("Gemini 手动添加", "Gemini manual add")}
          >
            <div className="nt-pilot-dialog__header">
              <h2>{t("Gemini 手动添加", "Gemini manual add")}</h2>
              <button
                className="nt-icon-close"
                type="button"
                aria-label={t("关闭", "Close")}
                onClick={closeGeminiManualAddDialog}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>
            <div className="nt-stack">
              <article className="nt-card nt-card--panel">
                <div className="nt-copy">
                  <strong>{geminiManualAddDialogState.providerId}</strong>
                </div>
                <p className="nt-copy">
                  {geminiManualAddDialogState.busy
                    ? t("正在启动本地 Gemini 认证助手。", "Starting the local Gemini auth helper.")
                    : geminiManualAddDialogState.session?.message ??
                      t("等待 Gemini 认证结果。", "Waiting for Gemini auth results.")}
                </p>
                {geminiManualAddDialogState.session ? (
                  <div className="nt-ledger-chip-list nt-ledger-chip-list--dense">
                    <span className="nt-chip nt-chip--muted">
                      {t("状态", "Status")} {geminiManualAddDialogState.session.status}
                    </span>
                    <span className="nt-chip nt-chip--muted">
                      {t("目标族", "Target family")} {geminiManualAddDialogState.session.targetFamily}
                    </span>
                  </div>
                ) : null}
              </article>
              {geminiManualAddDialogState.session?.status === "succeeded" ? (
                <article className="nt-card nt-card--panel">
                  <p className="nt-copy">
                    {t(
                      "认证结果已经写入当前草稿。保存路由配置后生效。",
                      "The generated credentials were merged into the current draft. Save the route config to apply them.",
                    )}
                  </p>
                </article>
              ) : null}
              {canManuallyCompleteGeminiAuthSession(geminiManualAddDialogState.session) ? (
                <article className="nt-card nt-card--panel">
                  <p className="nt-copy">
                    {t(
                      "如果你已经在弹出的浏览器里完成 Gemini 登录，但仍然没有自动导入，请点击下面的按钮继续导入。",
                      "If you already completed Gemini login in the opened browser window but the import did not finish automatically, click the button below to continue the import.",
                    )}
                  </p>
                  <div className="dialog-actions">
                    <button
                      className="nt-btn"
                      type="button"
                      disabled={geminiManualAddDialogState.busy}
                      onClick={() => {
                        void requestGeminiManualAddCompletion();
                      }}
                    >
                      {geminiManualAddDialogState.busy
                        ? t("正在导入…", "Importing…")
                        : t("已完成登录，继续导入", "I finished login, continue import")}
                    </button>
                  </div>
                </article>
              ) : null}
            </div>
          </section>
        </>
      ) : null}
      {pilotActionDialog ? (
        <>
          <div className="dialog-overlay" onClick={() => setPilotActionDialog(null)} />
          <section
            className="dialog-content nt-pilot-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={
              pilotActionDialog.kind === "probe"
                ? t("测试账号连接", "Test account connection")
                : pilotActionDialog.kind === "stats"
                  ? t("查看账号统计", "View account stats")
                  : t("定时测试", "Scheduled tests")
            }
          >
            <div className="nt-pilot-dialog__header">
              <h2>
                {pilotActionDialog.kind === "probe"
                  ? t("测试账号连接", "Test account connection")
                  : pilotActionDialog.kind === "stats"
                    ? t("查看账号统计", "View account stats")
                    : t("定时测试", "Scheduled tests")}
              </h2>
              <button
                className="nt-icon-close"
                type="button"
                aria-label={t("关闭", "Close")}
                onClick={() => setPilotActionDialog(null)}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>

            {pilotActionDialog.kind === "probe" ? (
              <div className="nt-stack">
                <article className="nt-pilot-dialog__hero">
                  <div className="nt-pilot-dialog__hero-icon">
                    <Play size={20} aria-hidden="true" />
                  </div>
                  <div className="nt-pilot-dialog__hero-copy">
                    <strong>{pilotActionDialog.account.displayName}</strong>
                    <div className="nt-ledger-chip-list nt-ledger-chip-list--dense">
                      <span className="nt-chip nt-chip--muted">APIKEY</span>
                      <span className="nt-chip nt-chip--muted">{t("账号", "Account")}</span>
                    </div>
                  </div>
                  <span
                    className={
                      pilotActionDialog.account.dispatchEnabled
                        ? "nt-badge nt-badge--success"
                        : "nt-badge nt-badge--warning"
                    }
                  >
                    {pilotActionDialog.account.dispatchEnabled ? "active" : t("暂停", "Paused")}
                  </span>
                </article>

                <label className="nt-field">
                  <span>{t("选择测试模型", "Select test model")}</span>
                  <select
                    className="nt-select"
                    value={pilotProbeModel}
                    onChange={(event) => setPilotProbeModel(event.currentTarget.value)}
                  >
                    <option>GPT-5.6 (Sol)</option>
                    <option>GPT-5.5</option>
                    <option>GPT-5.4</option>
                  </select>
                </label>

                <label className="nt-field">
                  <span>{t("测试模式", "Test mode")}</span>
                  <select
                    className="nt-select"
                    value={pilotProbeMode}
                    onChange={(event) => setPilotProbeMode(event.currentTarget.value)}
                  >
                    <option>{t("常规请求", "Normal request")}</option>
                    <option>{t("快速探测", "Quick probe")}</option>
                  </select>
                </label>

                <div className="nt-pilot-terminal">
                  {pilotActionDialog.account.previewOnly
                    ? t(
                        "演示账号仅展示 UI，不会发起真实连接测试。",
                        "Demo accounts only preview the UI and do not start a real connection test.",
                      )
                    : credentialProbeBusy === pilotActionDialog.account.accountId
                      ? t("正在测试连接，请稍候...", "Testing connectivity, please wait...")
                      : activePilotProbeResult
                        ? activePilotProbeResult.message
                        : t(
                            "准备测试。点击“开始测试”按钮开始...",
                            'Ready to test. Click "Start test" to begin...',
                          )}
                </div>

                <div className="nt-pilot-dialog__meta">
                  <span>{t("测试模型", "Test model")}</span>
                  <span>{t('提示词: "hi"', 'Prompt: "hi"')}</span>
                </div>

                <div className="dialog-actions">
                  <button
                    className="nt-btn nt-btn--secondary"
                    type="button"
                    onClick={() => setPilotActionDialog(null)}
                  >
                    {t("关闭", "Close")}
                  </button>
                  <button
                    className="nt-btn nt-btn--primary"
                    type="button"
                    disabled={
                      pilotActionDialog.account.previewOnly ||
                      !activePilotManagedAccount ||
                      credentialProbeBusy === pilotActionDialog.account.accountId
                    }
                    onClick={() => void startPilotProbe()}
                  >
                    {t("开始测试", "Start test")}
                  </button>
                </div>
              </div>
            ) : null}

            {pilotActionDialog.kind === "stats" ? (
              <div className="nt-stack">
                <article className="nt-pilot-dialog__hero">
                  <div className="nt-pilot-dialog__hero-icon">
                    <BarChart3 size={20} aria-hidden="true" />
                  </div>
                  <div className="nt-pilot-dialog__hero-copy">
                    <strong>{pilotActionDialog.account.displayName}</strong>
                    <span>
                      {t(
                        "近30天使用统计（日均基于实际使用天数）",
                        "Usage summary for the last 30 days (daily averages use active days only)",
                      )}
                    </span>
                  </div>
                  <span
                    className={
                      pilotActionDialog.account.dispatchEnabled
                        ? "nt-badge nt-badge--success"
                        : "nt-badge nt-badge--warning"
                    }
                  >
                    {pilotActionDialog.account.dispatchEnabled ? "active" : "paused"}
                  </span>
                </article>

                <div className="nt-pilot-stats-overview-grid">
                  <article className="nt-card nt-card--panel nt-pilot-stat-card nt-pilot-stat-card--emerald">
                    <div className="nt-pilot-stat-card__head">
                      <span>{t("30天总费用", "30-day total cost")}</span>
                    </div>
                    <strong>{activePilotStatsView?.totalCostText ?? "$0.0000"}</strong>
                    <p>
                      {t("累计成本", "Accumulated cost")} (
                      {t("用户扣费", "User charge")}: {activePilotStatsView?.totalUserCostText ?? "$0.0000"} ·
                      {t("标准计费", "Standard billing")}: {activePilotStatsView?.totalStandardCostText ?? "$0.0000"})
                    </p>
                  </article>
                  <article className="nt-card nt-card--panel nt-pilot-stat-card nt-pilot-stat-card--blue">
                    <div className="nt-pilot-stat-card__head">
                      <span>{t("30天总请求", "30-day total requests")}</span>
                    </div>
                    <strong>{activePilotStatsView?.totalRequestsText ?? "0"}</strong>
                    <p>{t("累计调用次数", "Accumulated request count")}</p>
                  </article>
                  <article className="nt-card nt-card--panel nt-pilot-stat-card nt-pilot-stat-card--orange">
                    <div className="nt-pilot-stat-card__head">
                      <span>{t("日均费用", "Average daily cost")}</span>
                    </div>
                    <strong>{activePilotStatsView?.avgDailyCostText ?? "$0.0000"}</strong>
                    <p>
                      {t("基于", "Based on")} {activePilotStatsView?.activeDaysText ?? "0"}{" "}
                      {t("天实际使用", "active day(s)")}
                    </p>
                  </article>
                  <article className="nt-card nt-card--panel nt-pilot-stat-card nt-pilot-stat-card--purple">
                    <div className="nt-pilot-stat-card__head">
                      <span>{t("日均请求", "Average daily requests")}</span>
                    </div>
                    <strong>{activePilotStatsView?.avgDailyRequestsText ?? "0"}</strong>
                    <p>{t("平均每日调用", "Average calls per active day")}</p>
                  </article>
                </div>

                <div className="nt-pilot-stats-detail-grid">
                  <article className="nt-card nt-card--panel nt-pilot-metric-card">
                    <h3>{t("今日概览", "Today overview")}</h3>
                    <dl className="nt-pilot-metric-list">
                      <div>
                        <dt>{t("账号计费", "Account billing")}</dt>
                        <dd>{activePilotStatsView?.totalStandardCostText ?? "$0.0000"}</dd>
                      </div>
                      <div>
                        <dt>{t("用户扣费", "User charge")}</dt>
                        <dd>{activePilotStatsView?.totalUserCostText ?? "$0.0000"}</dd>
                      </div>
                      <div>
                        <dt>{t("请求", "Requests")}</dt>
                        <dd>{activePilotStatsView?.todayRequestsText ?? "0"}</dd>
                      </div>
                      <div>
                        <dt>Token</dt>
                        <dd>{activePilotStatsView?.todayTokenText ?? "0"}</dd>
                      </div>
                    </dl>
                  </article>

                  <article className="nt-card nt-card--panel nt-pilot-metric-card">
                    <h3>{t("最高费用日", "Highest cost day")}</h3>
                    <dl className="nt-pilot-metric-list">
                      <div>
                        <dt>{t("日期", "Date")}</dt>
                        <dd>-</dd>
                      </div>
                      <div>
                        <dt>{t("账号计费", "Account billing")}</dt>
                        <dd>{activePilotStatsView?.totalStandardCostText ?? "$0.0000"}</dd>
                      </div>
                      <div>
                        <dt>{t("用户扣费", "User charge")}</dt>
                        <dd>{activePilotStatsView?.totalUserCostText ?? "$0.0000"}</dd>
                      </div>
                      <div>
                        <dt>{t("请求", "Requests")}</dt>
                        <dd>{activePilotStatsView?.totalRequestsText ?? "0"}</dd>
                      </div>
                    </dl>
                  </article>

                  <article className="nt-card nt-card--panel nt-pilot-metric-card">
                    <h3>{t("最高请求日", "Highest request day")}</h3>
                    <dl className="nt-pilot-metric-list">
                      <div>
                        <dt>{t("日期", "Date")}</dt>
                        <dd>-</dd>
                      </div>
                      <div>
                        <dt>{t("请求", "Requests")}</dt>
                        <dd>{activePilotStatsView?.totalRequestsText ?? "0"}</dd>
                      </div>
                      <div>
                        <dt>{t("账号计费", "Account billing")}</dt>
                        <dd>{activePilotStatsView?.totalStandardCostText ?? "$0.0000"}</dd>
                      </div>
                      <div>
                        <dt>{t("用户扣费", "User charge")}</dt>
                        <dd>{activePilotStatsView?.totalUserCostText ?? "$0.0000"}</dd>
                      </div>
                    </dl>
                  </article>

                  <article className="nt-card nt-card--panel nt-pilot-metric-card">
                    <h3>{t("累计 Token", "Token totals")}</h3>
                    <dl className="nt-pilot-metric-list">
                      <div>
                        <dt>{t("30天总计", "30-day total")}</dt>
                        <dd>{activePilotStatsView?.totalTokenText ?? "0"}</dd>
                      </div>
                      <div>
                        <dt>{t("日均 Token", "Average daily token")}</dt>
                        <dd>{activePilotStatsView?.avgDailyTokenText ?? "0"}</dd>
                      </div>
                    </dl>
                  </article>

                  <article className="nt-card nt-card--panel nt-pilot-metric-card">
                    <h3>{t("性能", "Performance")}</h3>
                    <dl className="nt-pilot-metric-list">
                      <div>
                        <dt>{t("平均响应", "Average response")}</dt>
                        <dd>{activePilotStatsView?.avgResponseText ?? "0ms"}</dd>
                      </div>
                      <div>
                        <dt>{t("活跃天数", "Active days")}</dt>
                        <dd>
                          {activePilotStatsView?.activeDaysText ?? "0"} /{" "}
                          {activePilotStatsView?.activeDaysDenominatorText ?? "31"}
                        </dd>
                      </div>
                    </dl>
                  </article>

                  <article className="nt-card nt-card--panel nt-pilot-metric-card">
                    <h3>{t("最近统计", "Recent stats")}</h3>
                    <dl className="nt-pilot-metric-list">
                      <div>
                        <dt>{t("今日请求", "Today's requests")}</dt>
                        <dd>{activePilotStatsView?.todayRequestsText ?? "0"}</dd>
                      </div>
                      <div>
                        <dt>{t("今日 Token", "Today's token")}</dt>
                        <dd>{activePilotStatsView?.todayTokenText ?? "0"}</dd>
                      </div>
                      <div>
                        <dt>{t("今日费用", "Today's cost")}</dt>
                        <dd>{activePilotStatsView?.totalCostText ?? "$0.0000"}</dd>
                      </div>
                      <div>
                        <dt>{t("最近使用", "Recent use")}</dt>
                        <dd>{pilotActionDialog.account.recentUseLabel}</dd>
                      </div>
                    </dl>
                  </article>
                </div>

                <article className="nt-card nt-card--panel nt-pilot-chart-panel">
                  <h3>{t("30天费用与请求趋势", "30-day cost and request trend")}</h3>
                  <div className="nt-pilot-chart-empty">{t("暂无数据", "No data")}</div>
                </article>

                <article className="nt-card nt-card--panel nt-pilot-chart-panel">
                  <h3>{t("模型分布", "Model distribution")}</h3>
                  <div className="nt-pilot-chart-empty">{t("暂无数据", "No data")}</div>
                </article>

                <article className="nt-card nt-card--panel nt-pilot-chart-panel">
                  <h3>{t("入站端点", "Ingress nodes")}</h3>
                  <div className="nt-pilot-chart-empty">{t("暂无数据", "No data")}</div>
                </article>

                <article className="nt-card nt-card--panel nt-pilot-chart-panel">
                  <h3>{t("上游端点", "Upstream nodes")}</h3>
                  <div className="nt-pilot-chart-empty">{t("暂无数据", "No data")}</div>
                </article>

                <div className="dialog-actions">
                  <button
                    className="nt-btn nt-btn--secondary"
                    type="button"
                    onClick={() => setPilotActionDialog(null)}
                  >
                    {t("关闭", "Close")}
                  </button>
                </div>
              </div>
            ) : null}

            {pilotActionDialog.kind === "schedule" ? (
              <div className="nt-stack">
                <div className="nt-actions nt-actions--right">
                  <button className="nt-btn nt-btn--primary" type="button">
                    {t("添加计划", "Add schedule")}
                  </button>
                </div>
                <article className="nt-pilot-schedule-empty">
                  <CalendarClock size={24} aria-hidden="true" />
                  <span>{t("暂无定时测试计划", "No scheduled test plans")}</span>
                </article>
              </div>
            ) : null}
          </section>
        </>
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
