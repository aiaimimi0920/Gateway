import type { RouteAccountCatalog, RouteManagedAccount } from "./accountCatalogTypes";
import {
  aggregateProviderMetrics,
  type ProviderAggregateMetrics,
  type ProviderMetricsAccountLike,
  type ProviderMetricsResolver,
} from "./providerCardMetrics";

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

/**
 * One member account behind a group's provider/model scope. The usage stays raw
 * rather than pre-aggregated: the card back re-rolls each model row over the
 * providers the operator selected, and summing already-summed success windows
 * would double count them.
 */
export type CredentialGroupScopeMember = {
  accountId: string;
  providerId: string;
  providerLabel: string;
  enabled: boolean;
  metrics: ProviderMetricsAccountLike | null;
};

/** A selectable provider in a group's scope, with what selecting it adds. */
export type CredentialGroupProviderScope = {
  providerId: string;
  providerLabel: string;
  accountCount: number;
  modelCount: number;
  /**
   * How many accounts the provider has in total. When it exceeds `accountCount`
   * the group holds only part of the pool, and the usage attributed to this tab
   * covers the whole provider account - the gateway records money and traffic
   * per provider account, so a narrower number would have to be invented.
   */
  providerAccountCount: number;
};

/**
 * One provider account's numbers for a model row. Resolved at provider-account
 * granularity because that is the finest grain the gateway records money and
 * traffic at, and kept per provider so the card back can re-roll the row over
 * exactly the provider tabs the operator selected.
 */
export type CredentialGroupModelProviderMetrics = {
  providerId: string;
  metrics: ProviderMetricsAccountLike;
};

/** A model the group can serve, with every member account that serves it. */
export type CredentialGroupModelScope = {
  model: string;
  members: CredentialGroupScopeMember[];
  /**
   * Per-provider usage for this model, preferred over `members[].metrics` when
   * the caller supplied a resolver. Empty when it did not.
   */
  providerMetrics: CredentialGroupModelProviderMetrics[];
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
  /** Provider tabs for the card back, sorted by label like `providerLabels`. */
  providerScopes: CredentialGroupProviderScope[];
  /** Model rows for the card back, sorted by model like `modelLabels`. */
  modelScopes: CredentialGroupModelScope[];
  /**
   * Usage rolled up over the group's member accounts, mirroring the provider
   * card front on the credential pool page. Null when the caller supplied no
   * per-account metrics, so the card renders placeholders instead of zeroes.
   */
  metrics: ProviderAggregateMetrics | null;
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

/**
 * A draft's billing multiplier as a number, or null while the field holds
 * something the operator is still typing. Null means "no explicit rate", not
 * "x1": the caller falls back to the rate its credentials resolve to.
 */
function draftBillingMultiplier(value: string): number | null {
  const parsed = Number(value.trim());
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

export function buildCredentialGroupDirectory(
  catalog: RouteAccountCatalog,
  rows: AccountGroupDraftLike[],
  metricsByAccountId?: ReadonlyMap<string, ProviderMetricsAccountLike>,
  resolver?: ProviderMetricsResolver,
): CredentialGroupDirectoryItem[] {
  const accountsById = new Map(catalog.accounts.map((account) => [account.id, account]));
  const providerAccountCounts = new Map<string, number>();
  for (const account of catalog.accounts) {
    providerAccountCounts.set(
      account.providerId,
      (providerAccountCounts.get(account.providerId) ?? 0) + 1,
    );
  }
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
    const memberMetrics = metricsByAccountId
      ? members
          .map((member) => metricsByAccountId.get(member.id))
          .filter((metrics): metrics is ProviderMetricsAccountLike => Boolean(metrics))
      : [];
    // A group bills at one rate, so the resolver is told it rather than left to
    // infer a ceiling from the members' other groups.
    const resolveOptions = {
      billingMultiplier: draftBillingMultiplier(row.billingMultiplier) ?? undefined,
      credentialRefs: members.map((member) => member.id),
    };
    const providerScopeIndex = new Map<
      string,
      { providerLabel: string; accountCount: number; models: Set<string> }
    >();
    const modelScopeIndex = new Map<string, CredentialGroupScopeMember[]>();
    for (const member of members) {
      const scopeMember: CredentialGroupScopeMember = {
        accountId: member.id,
        providerId: member.providerId,
        providerLabel: member.providerLabel,
        enabled: member.enabled,
        metrics: metricsByAccountId?.get(member.id) ?? null,
      };
      let providerScope = providerScopeIndex.get(member.providerId);
      if (!providerScope) {
        providerScope = {
          providerLabel: member.providerLabel,
          accountCount: 0,
          models: new Set<string>(),
        };
        providerScopeIndex.set(member.providerId, providerScope);
      }
      providerScope.accountCount += 1;
      // Dedupe per member: a credential may list the same model twice, which
      // would otherwise push the account onto a model row more than once and
      // double count its usage.
      for (const model of new Set(member.supportedModels)) {
        providerScope.models.add(model);
        const modelMembers = modelScopeIndex.get(model);
        if (modelMembers) {
          modelMembers.push(scopeMember);
        } else {
          modelScopeIndex.set(model, [scopeMember]);
        }
      }
    }
    const providerScopes = [...providerScopeIndex.entries()]
      .map(([providerId, scope]) => ({
        providerId,
        providerLabel: scope.providerLabel,
        accountCount: scope.accountCount,
        modelCount: scope.models.size,
        providerAccountCount: providerAccountCounts.get(providerId) ?? scope.accountCount,
      }))
      .sort((left, right) => left.providerLabel.localeCompare(right.providerLabel));
    const modelScopes = [...modelScopeIndex.entries()]
      .map(([model, scopeMembers]) => ({
        model,
        members: scopeMembers,
        providerMetrics: resolver
          ? [...new Set(scopeMembers.map((scopeMember) => scopeMember.providerId))]
              .map((providerId) => {
                const metrics = resolver.providerAccountModel(providerId, model, resolveOptions);
                return metrics ? { providerId, metrics } : null;
              })
              .filter((entry): entry is CredentialGroupModelProviderMetrics => entry !== null)
          : [],
      }))
      .sort((left, right) => left.model.localeCompare(right.model));
    // The group's own numbers come from its provider accounts when a resolver is
    // available: summing the member cards would either multiply a provider's
    // totals by its pool size or, since per-credential attribution is suppressed
    // for pooled providers, read as no data at all.
    const resolvedGroupMetrics = resolver
      ? resolver.providerAccounts([...providerScopeIndex.keys()], resolveOptions)
      : null;
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
      providerScopes,
      modelScopes,
      metrics: resolvedGroupMetrics
        ? aggregateProviderMetrics([resolvedGroupMetrics])
        : memberMetrics.length > 0
          ? aggregateProviderMetrics(memberMetrics)
          : null,
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
        account.providerId,
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
