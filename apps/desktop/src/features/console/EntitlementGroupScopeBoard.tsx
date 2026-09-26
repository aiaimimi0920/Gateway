import { useMemo, useState } from "react";

import type {
  CredentialGroupModelScope,
  CredentialGroupProviderScope,
} from "./accountManagementViewModel";
import {
  EntitlementGroupModelScope,
  type EntitlementModelScopeRow,
} from "./EntitlementGroupModelScope";
import { ScopePager, type ScopePagerProps } from "./EntitlementScopePager";
import {
  aggregateProviderMetrics,
  type ProviderMetricsAccountLike,
} from "./providerCardMetrics";

export { ScopePager };
export type { ScopePagerProps };

type TranslateFn = (zh: string, en: string) => string;

/**
 * Both bands page instead of scrolling: the card face is a fixed height and the
 * standing rule is that no card ever grows a scrollbar, so a group with 16
 * providers and 40 models walks through pages at a stable height.
 */
const PROVIDER_TABS_PER_PAGE = 6;
const MODEL_ROWS_PER_PAGE = 3;

export type EntitlementGroupScopeBoardProps = {
  t: TranslateFn;
  /** Stable per-card prefix for React keys and DOM ids. */
  cardKey: string;
  label: string;
  providerScopes: CredentialGroupProviderScope[];
  modelScopes: CredentialGroupModelScope[];
  /**
   * The complement of the provider selection, owned by the card so the expanded
   * account panel can filter by the very same choice.
   */
  deselectedProviderIds: readonly string[];
  onDeselectedProviderIdsChange: (next: string[]) => void;
};

/**
 * The entitlement group card back: a multi-select provider band on top, and one
 * aggregate row per model below it. Selecting more providers widens the model
 * list, and every row re-rolls over the raw member accounts of the selected
 * providers, so the numbers are the same ones the provider cards would show for
 * that slice.
 */
export function EntitlementGroupScopeBoard({
  t,
  cardKey,
  label,
  providerScopes,
  modelScopes,
  deselectedProviderIds,
  onDeselectedProviderIdsChange,
}: EntitlementGroupScopeBoardProps) {
  const [providerPageState, setProviderPageState] = useState(1);
  const [modelPageState, setModelPageState] = useState(1);

  const selectedProviderIds = useMemo(() => {
    const deselected = new Set(deselectedProviderIds);
    return new Set(
      providerScopes
        .map((scope) => scope.providerId)
        .filter((providerId) => !deselected.has(providerId)),
    );
  }, [deselectedProviderIds, providerScopes]);

  const visibleModels = useMemo<EntitlementModelScopeRow[]>(() => {
    if (selectedProviderIds.size === 0) {
      return [];
    }
    return modelScopes
      .map((scope) => {
        const members = scope.members.filter((member) =>
          selectedProviderIds.has(member.providerId),
        );
        if (members.length === 0) {
          return null;
        }
        // Money and traffic are recorded per provider account and per model, so a
        // row reads that rollup for each selected provider. `members[].metrics` is
        // the fallback for a directory built without a resolver, and it renders
        // `—` for pooled providers rather than counting one account's traffic once
        // per credential.
        const scopedProviderMetrics = scope.providerMetrics.filter((entry) =>
          selectedProviderIds.has(entry.providerId),
        );
        const metricAccounts =
          scopedProviderMetrics.length > 0
            ? scopedProviderMetrics.map((entry) => entry.metrics)
            : members
                .map((member) => member.metrics)
                .filter((metrics): metrics is ProviderMetricsAccountLike => metrics != null);
        return {
          model: scope.model,
          accountsTotal: members.length,
          accountsEnabled: members.filter((member) => member.enabled).length,
          providerCount: new Set(members.map((member) => member.providerId)).size,
          metrics: aggregateProviderMetrics(metricAccounts),
        };
      })
      .filter((row): row is NonNullable<typeof row> => row != null);
  }, [modelScopes, selectedProviderIds]);

  const providerTotalPages = Math.max(
    1,
    Math.ceil(providerScopes.length / PROVIDER_TABS_PER_PAGE),
  );
  const providerPage = Math.min(providerPageState, providerTotalPages);
  const providerPageScopes = providerScopes.slice(
    (providerPage - 1) * PROVIDER_TABS_PER_PAGE,
    providerPage * PROVIDER_TABS_PER_PAGE,
  );
  const modelTotalPages = Math.max(1, Math.ceil(visibleModels.length / MODEL_ROWS_PER_PAGE));
  const modelPage = Math.min(modelPageState, modelTotalPages);
  const modelPageRows = visibleModels.slice(
    (modelPage - 1) * MODEL_ROWS_PER_PAGE,
    modelPage * MODEL_ROWS_PER_PAGE,
  );
  const allSelected =
    providerScopes.length > 0 && selectedProviderIds.size === providerScopes.length;

  const toggleProvider = (providerId: string) => {
    onDeselectedProviderIdsChange(
      deselectedProviderIds.includes(providerId)
        ? deselectedProviderIds.filter((candidate) => candidate !== providerId)
        : [...deselectedProviderIds, providerId],
    );
    setModelPageState(1);
  };

  return (
    <div className="nt-entitlement-scope" data-entitlement-scope-board={cardKey}>
      <section
        className="nt-entitlement-scope__band"
        aria-label={t(`${label} 服务商范围`, `${label} provider scope`)}
      >
        <header className="nt-entitlement-scope__band-head">
          <strong>{t("服务商范围", "Provider scope")}</strong>
          <span className="nt-entitlement-scope__band-tools">
            <span className="nt-entitlement-scope__count">
              {selectedProviderIds.size}/{providerScopes.length}
            </span>
            {providerScopes.length > 1 ? (
              <button
                className="nt-entitlement-scope__band-action"
                type="button"
                onClick={() => {
                  onDeselectedProviderIdsChange(
                    allSelected ? providerScopes.map((scope) => scope.providerId) : [],
                  );
                  setModelPageState(1);
                }}
              >
                {allSelected ? t("清空", "Clear") : t("全选", "Select all")}
              </button>
            ) : null}
          </span>
        </header>
        {providerScopes.length > 0 ? (
          <>
            <div
              className="nt-entitlement-scope__tabs"
              role="group"
              aria-label={t("服务商标签，可多选", "Provider tabs, multi-select")}
            >
              {providerPageScopes.map((scope) => {
                const selected = selectedProviderIds.has(scope.providerId);
                // The group holds a slice of this provider's pool, but upstream
                // only records traffic per provider account — so the rows below
                // cover the whole pool. Say so instead of implying the slice.
                const partialPool = scope.providerAccountCount > scope.accountCount;
                return (
                  <button
                    className="nt-entitlement-scope__tab"
                    type="button"
                    key={`${cardKey}:provider:${scope.providerId}`}
                    data-entitlement-scope-provider={scope.providerId}
                    data-entitlement-scope-partial={partialPool ? "true" : undefined}
                    aria-pressed={selected}
                    title={t(
                      `${scope.providerLabel}，${scope.accountCount} 个账号，${scope.modelCount} 个模型${
                        partialPool
                          ? `（服务商共 ${scope.providerAccountCount} 个账号，用量按服务商账号统计）`
                          : ""
                      }`,
                      `${scope.providerLabel}, ${scope.accountCount} accounts, ${scope.modelCount} models${
                        partialPool
                          ? ` (provider has ${scope.providerAccountCount} accounts; usage is measured per provider account)`
                          : ""
                      }`,
                    )}
                    onClick={() => toggleProvider(scope.providerId)}
                  >
                    <span>{scope.providerLabel}</span>
                    <small>{scope.modelCount}</small>
                  </button>
                );
              })}
            </div>
            <ScopePager
              t={t}
              page={providerPage}
              totalPages={providerTotalPages}
              navLabel={t("服务商范围分页", "Provider scope pagination")}
              onPage={(next) => setProviderPageState(next)}
            />
          </>
        ) : (
          <p className="nt-entitlement-scope__empty">
            {t("暂无服务商，请先挂载账号", "No providers yet, attach accounts first")}
          </p>
        )}
      </section>

      <EntitlementGroupModelScope
        t={t}
        cardKey={cardKey}
        label={label}
        selectedProviderIds={selectedProviderIds}
        visibleModels={visibleModels}
        modelPage={modelPage}
        modelTotalPages={modelTotalPages}
        modelPageRows={modelPageRows}
        onPage={(next) => setModelPageState(next)}
      />
    </div>
  );
}
