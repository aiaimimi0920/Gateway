import { readStorageConnection } from "./providerStorageConnection";
import type {
  ConsoleGeminiAuthFamily,
  ConsoleProviderCredentialInventoryItem, ConsoleRouteDocument,
} from "../../api/contracts";
import type { UiLocale } from "../../i18n/UiLocaleProvider";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import { type AccountLedgerRow, resolveGeminiLogicalChannel } from "./accountManagementViewModel";
import { aggregateProviderMetrics, type ProviderAggregateMetrics } from "./providerCardMetrics";
import { readQuotaRemainingUsd, type ConsoleTelemetrySnapshot } from "./telemetry";
import { credentialStatusLabel, formatRelativeSince } from "./telemetryPresentation";
import { rollupProviderAccountTelemetry } from "./telemetryRollups";
import { optionalString, hostLabelFromUrl } from "./routeAccountCatalog";
import { optionalBoolean, readPilotProviderPolicy, readPilotIdentityCategories } from "./pilotPoolPolicy";
import { isRecord } from "./routeDocument";
import { credentialCardModelTraffic, providerCardModelTraffic } from "./cardModelTraffic";

function optionalStringArray(record: Record<string, unknown>, key: string): string[] {
  const value = record[key];
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string" && entry.trim().length > 0)
    .map((entry) => entry.trim());
}

function credentialLibraryName(
  providerId: string,
  credential: Record<string, unknown> | null,
  inventory: ConsoleProviderCredentialInventoryItem | null,
  categoryId?: string,
): string {
  const explicit = credential
    ? optionalString(credential, "account_library") ?? optionalString(credential, "accountLibrary")
    : null;
  if (explicit) {
    return explicit;
  }
  const sourcePath = inventory?.sourcePath;
  if (sourcePath) {
    const segments = sourcePath
      .replaceAll("\\", "/")
      .split("/")
      .map((segment) => segment.trim())
      .filter(Boolean);
    const parent = segments.length > 1 ? segments.at(-2) ?? null : null;
    if (parent && parent.toLocaleLowerCase() !== providerId.toLocaleLowerCase()) {
      return parent;
    }
  }
  return (
    categoryId ??
    (credential ? optionalString(credential, "credential_identity_category_id") : null) ??
    "default"
  );
}

function credentialRegisteredEmail(
  credential: Record<string, unknown> | null,
  inventory: ConsoleProviderCredentialInventoryItem | null,
): string | null {
  const sources = [credential, inventory?.credential ?? null];
  const keys = ["email", "account_email", "accountEmail", "registered_email", "registeredEmail"];
  for (const source of sources) {
    if (!source) {
      continue;
    }
    for (const key of keys) {
      const value = optionalString(source, key);
      if (value?.includes("@")) {
        return value;
      }
    }
  }
  return null;
}

/**
 * Everything an account card needs that does not come from the route document.
 *
 * `attributeProviderAccount` protects provider-level money and concurrency.
 * Requests prefer the explicit credential attribution in the audit summary. Copying provider
 * totals onto every credential of a pool would multiply them by the pool size,
 * so they are attributed to a single account row only when that row is the
 * provider's only routable identity; otherwise the provider card reads them from
 * `rollupProviderAccountTelemetry` and the account cards render `—`.
 */
type PilotAccountTelemetryContext = {
  snapshot: ConsoleTelemetrySnapshot;
  locale: UiLocale;
  attributeProviderAccount: boolean;
};

function buildPilotSectionAccount(
  row: AccountLedgerRow,
  telemetry: PilotAccountTelemetryContext,
  options: {
    credential?: Record<string, unknown> | null;
    categoryId?: string;
    logicalLabels?: string[];
    previewOnly?: boolean;
    discoverySupported?: boolean;
    inventory?: ConsoleProviderCredentialInventoryItem | null;
    scheduleSource?: Record<string, unknown> | null;
  } = {},
): AccountsLedgerPilotAccount {
  const credential = options.credential ?? null;
  const inventory = options.inventory ?? null;
  const logicalLabels = options.logicalLabels ?? row.groupLabels;
  const scheduleSource = options.scheduleSource ?? credential;
  const scheduledProbeInterval = scheduleSource?.scheduled_probe_interval_minutes;
  const { snapshot, locale } = telemetry;
  const credentialHealth = snapshot.credentials.get(row.accountId) ?? null;
  const credentialAudit = snapshot.credentialAuditStats?.get(JSON.stringify([row.providerId, row.accountId]));
  const providerAccount = telemetry.attributeProviderAccount
    ? snapshot.providerAccounts.get(row.providerId) ?? null
    : null;

  const concurrencyUsed = providerAccount?.activeConcurrency ?? null;
  const concurrencyTotal = providerAccount?.concurrencyLimit ?? null;
  const upstreamCost = providerAccount?.upstreamCostUsd ?? null;
  const healthLabel = credentialStatusLabel(credentialHealth?.status ?? null, locale);
  const lastUsedAt = credentialAudit?.lastRequestAt ?? credentialHealth?.lastSuccessAt ?? providerAccount?.lastRequestAt ?? null;
  const requestCount = credentialAudit?.totalRequests ?? providerAccount?.requestCount ??
    (snapshot.providerAccounts.get(row.providerId)?.requestCount === 0 ? 0 : null);

  return {
    accountId: row.accountId,
    discoverySupported: options.discoverySupported,
    providerId: row.providerId,
    displayName: credentialRegisteredEmail(credential, inventory) ?? row.displayName,
    mode: row.mode,
    enabled: row.enabled,
    logicalLabels: [...logicalLabels],
    supportedModels: [...row.supportedModels],
    modelTraffic: row.mode === "provider-default"
      ? providerCardModelTraffic(snapshot, [row.providerId])
      : credentialCardModelTraffic(snapshot, row.providerId, row.accountId),
    logicalGroupIds: [...row.groupIds],
    libraryName: credentialLibraryName(
      row.providerId,
      credential,
      inventory,
      options.categoryId,
    ),
    // Kept for the pool-bar denominator only; the card renders the numeric pair.
    capacityLabel: `${concurrencyUsed ?? 0} / ${concurrencyTotal ?? "—"}`,
    statusLabel: !row.enabled
      ? locale === "zh-CN"
        ? "暂停"
        : "paused"
      : healthLabel ??
        // No health row yet means the credential has never been dispatched, which
        // is not the same claim as "healthy".
        (locale === "zh-CN" ? "待观测" : "not observed"),
    dispatchEnabled: row.enabled && credentialHealth?.status !== "blocked",
    dispatchEditable: row.mode === "credential",
    previewOnly: options.previewOnly ?? false,
    usageWindowBadges: [],
    recentUseLabel:
      formatRelativeSince(lastUsedAt, locale) ??
      (locale === "zh-CN" ? "暂无调用" : "no traffic"),
    verificationStatus: row.verificationStatus,
    verificationFamilies: [...row.verificationFamilies],
    verificationCheckedAt: row.verificationCheckedAt,
    verificationEvidenceRef: row.verificationEvidenceRef,
    verificationNote: row.verificationNote,
    concurrencyUsed,
    concurrencyTotal,
    requestCount,
    upstreamCost,
    // Usage pricing and group multipliers do not prove settled revenue.
    userCost: null,
    successWindows: credentialAudit?.windows.map((window) => ({
      label: window.label, success: window.successCount, requests: window.totalRequests,
    })) ?? providerAccount?.successWindows ?? null,
    quota: inventory?.providerQuota ?? null,
    quotaRemainingUsd:
      credentialHealth?.quotaRemainingUsd ??
      readQuotaRemainingUsd(inventory?.providerQuota?.rawData),
    scheduledProbeEnabled:
      scheduleSource ? optionalBoolean(scheduleSource, "scheduled_probe_enabled") === true : false,
    scheduledProbeIntervalMinutes:
      typeof scheduledProbeInterval === "number" && Number.isFinite(scheduledProbeInterval)
        ? Math.max(1, Math.round(scheduledProbeInterval))
        : 60,
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

/**
 * Roll one card's provider accounts up into the shared metric shape. Runs after
 * the Gemini merge so a merged channel covers every provider id it absorbed.
 */
function buildSectionTelemetry(
  snapshot: ConsoleTelemetrySnapshot,
  providerAccountIds: readonly string[],
): ProviderAggregateMetrics {
  const rollup = rollupProviderAccountTelemetry(snapshot, providerAccountIds);
  return aggregateProviderMetrics([
    {
      concurrencyUsed: rollup.concurrencyUsed,
      concurrencyTotal: rollup.concurrencyTotal,
      requestCount: rollup.requestCount,
      upstreamCost: rollup.upstreamCostUsd,
      userCost: null,
      successWindows: rollup.successWindows,
    },
  ]);
}

export function buildAccountLedgerSections(
  document: ConsoleRouteDocument | null,
  rows: AccountLedgerRow[],
  credentialInventoryById: ReadonlyMap<string, ConsoleProviderCredentialInventoryItem>,
  snapshot: ConsoleTelemetrySnapshot,
  locale: UiLocale,
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
      // Provider-account telemetry has no per-credential breakdown, so it may be
      // shown on a card only when that card is the provider's single identity.
      // The count comes from the route document rather than `visibleRows` so the
      // numbers do not appear and disappear as the operator filters the ledger.
      const attributeProviderAccount = credentialRecordById.size <= 1;
      const telemetryContextFor = (row: AccountLedgerRow): PilotAccountTelemetryContext => ({
        snapshot,
        locale,
        attributeProviderAccount,
      });

      const identityDefinitions = readPilotIdentityCategories(provider, providerId, providerLabel);
      const supportsIdentityCategories = identityDefinitions.length > 0;
      const explicitRowsByIdentityCategory = new Map<string, AccountLedgerRow[]>();
      for (const row of explicitRows) {
        const credential = credentialRecordById.get(row.accountId);
        const categoryId = credential
          ? optionalString(credential, "credential_identity_category_id")
          : null;
        if (categoryId) {
          const categoryRows = explicitRowsByIdentityCategory.get(categoryId) ?? [];
          categoryRows.push(row);
          explicitRowsByIdentityCategory.set(categoryId, categoryRows);
        }
      }
      const identityCategories = identityDefinitions.map((definition) => {
        const accounts = (explicitRowsByIdentityCategory.get(definition.id) ?? []).map((account) => {
          const credential = credentialRecordById.get(account.accountId) ?? {};
          const previewLogicalLabels = optionalStringArray(
            credential,
            "preview_logical_labels",
          );
          return buildPilotSectionAccount(account, telemetryContextFor(account), {
            discoverySupported: !provider.preset && ["openai_compatible", "anthropic_compatible"].includes(String(provider.adapter)),
            credential,
            categoryId: definition.id,
            inventory: credentialInventoryById.get(account.accountId) ?? null,
            logicalLabels:
              account.groupLabels.length > 0
                ? [...account.groupLabels]
                : [...previewLogicalLabels],
          });
        });
        return {
          id: definition.id,
          label: definition.label,
          count: accounts.length,
          poolTargetSize: definition.poolTargetSize,
          autoRefillEnabled: definition.autoRefillEnabled,
          autoPruneEnabled: definition.autoPruneEnabled,
          accounts,
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
          adapter: optionalString(provider, "adapter"),
          protocolProfile: optionalString(provider, "protocol_profile"),
          hostLabel: hostLabelFromUrl(optionalString(provider, "base_url")),
          defaultAccountId: defaultAccount?.accountId ?? null,
          manualAddFamily,
          hasExplicitAccounts: explicitRows.length > 0,
          poolTargetSize: providerPolicy.poolTargetSize,
          poolMinSize: providerPolicy.poolMinSize,
          credentialStoragePath: optionalString(provider, "credential_storage_path") ?? undefined,
          credentialArchivePath: optionalString(provider, "credential_archive_path") ?? undefined,
          credentialStorageConnection: readStorageConnection(provider.credential_storage_connection),
          credentialArchiveConnection: readStorageConnection(provider.credential_archive_connection),
          autoRefillEnabled: providerPolicy.autoRefillEnabled,
          autoPruneEnabled: providerPolicy.autoPruneEnabled,
          permanentDeleteEnabled: providerPolicy.permanentDeleteEnabled,
          supportsIdentityCategories,
          identityCategories,
          directAccounts: supportsIdentityCategories
            ? []
            : visibleRows.map((row) =>
                buildPilotSectionAccount(row, telemetryContextFor(row), {
                  discoverySupported: !provider.preset && ["openai_compatible", "anthropic_compatible"].includes(String(provider.adapter)),
                  credential:
                    row.mode === "credential"
                      ? credentialRecordById.get(row.accountId) ?? null
                      : null,
                  scheduleSource:
                    row.mode === "credential"
                      ? credentialRecordById.get(row.accountId) ?? null
                      : provider,
                  inventory: credentialInventoryById.get(row.accountId) ?? null,
                }),
              ),
        } satisfies AccountsLedgerPilotSection,
      ];
    });

  return mergeGeminiAccountLedgerSections(sections).map((section) => ({
    ...section,
    telemetry: buildSectionTelemetry(snapshot, section.providerIds),
    modelTraffic: providerCardModelTraffic(snapshot, section.providerIds),
  } satisfies AccountsLedgerPilotSection));
}
