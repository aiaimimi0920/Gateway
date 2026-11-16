import { useMemo, useState } from "react";
import { Button, Checkbox, EmptyState, MultiSelect } from "@neuro/ui";

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

// Large provider sets use the standard popup; model rows keep bounded pagination
// inside the card's existing scrollable face.
const INLINE_PROVIDER_LIMIT = 6;
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

  const changeProviderValues = (values: string[]) => {
    const next = new Set(values);
    // Preserve exclusions for temporarily absent providers, just as a single
    // inline toggle does; a directory refresh must not silently select them.
    onDeselectedProviderIdsChange([
      ...deselectedProviderIds.filter((providerId) => !next.has(providerId)),
      ...Array.from(selectedProviderIds).filter((providerId) => !next.has(providerId)),
    ]);
    setModelPageState(1);
  };
  const describeProvider = (scope: CredentialGroupProviderScope) => t(
    `${scope.accountCount} 个账号，${scope.modelCount} 个模型${scope.providerAccountCount > scope.accountCount
      ? `（服务商共 ${scope.providerAccountCount} 个账号，用量按服务商账号统计）` : ""}`,
    `${scope.accountCount} accounts, ${scope.modelCount} models${scope.providerAccountCount > scope.accountCount
      ? ` (provider has ${scope.providerAccountCount} accounts; usage is measured per provider account)` : ""}`,
  );
  const scopeAction = providerScopes.length > 1 ? (
    <Button
      compact
      variant="quiet"
      aria-label={allSelected ? t("清空", "Clear") : t("全选", "Select all")}
      onClick={() => {
        onDeselectedProviderIdsChange(
          allSelected ? providerScopes.map((scope) => scope.providerId) : [],
        );
        setModelPageState(1);
      }}
    >
      <span className="nt-entitlement-scope__action-label" aria-hidden="true">
        <span style={{ visibility: allSelected ? "visible" : "hidden" }}>{t("清空", "Clear")}</span>
        <span style={{ visibility: allSelected ? "hidden" : "visible" }}>{t("全选", "Select all")}</span>
      </span>
    </Button>
  ) : null;

  return (
    <div className="nt-entitlement-scope" data-entitlement-scope-board={cardKey}>
      <section
        className="nt-entitlement-scope__band"
        aria-label={t(`${label} 服务商范围`, `${label} provider scope`)}
      >
        {providerScopes.length > INLINE_PROVIDER_LIMIT ? (
          <div className="nt-entitlement-scope__selector-row">
            <MultiSelect
              compact
              className="nt-entitlement-scope__selector"
              label={t("服务商范围", "Provider scope")}
              options={providerScopes.map((scope) => ({
                value: scope.providerId,
                label: scope.providerLabel,
                description: scope.providerAccountCount > scope.accountCount ? t(
                  `组内 ${scope.accountCount}/${scope.providerAccountCount} 个账号，用量按整个服务商统计`,
                  `${scope.accountCount}/${scope.providerAccountCount} accounts; provider-wide usage`,
                ) : undefined,
                meta: t(`${scope.modelCount} 个模型`, `${scope.modelCount} models`),
              }))}
              values={Array.from(selectedProviderIds)}
              onValuesChange={changeProviderValues}
              formatSummary={(count) => t(`已选 ${count}/${providerScopes.length} 项`, `${count}/${providerScopes.length} selected`)}
              search={{ label: t("搜索服务商", "Search providers"), emptyMessage: t("未找到服务商", "No providers found") }}
            />
            {scopeAction}
          </div>
        ) : (
          <>
            <header className="nt-entitlement-scope__band-head">
              <strong>{t("服务商范围", "Provider scope")}</strong>
              <span className="nt-entitlement-scope__band-tools">
                <span className="nt-entitlement-scope__count" style={{ minWidth: `${String(providerScopes.length).length * 2 + 1}ch` }}>
                  {selectedProviderIds.size}/{providerScopes.length}
                </span>
                {scopeAction}
              </span>
            </header>
            {providerScopes.length > 0 ? (
              <div
                className="nt-entitlement-scope__choices"
                role="group"
                aria-label={t("服务商范围，可多选", "Providers, multi-select")}
              >
                {providerScopes.map((scope) => {
                  const selected = selectedProviderIds.has(scope.providerId);
                  const partialPool = scope.providerAccountCount > scope.accountCount;
                  return (
                    <Checkbox
                      className="nt-entitlement-scope__choice"
                      key={`${cardKey}:provider:${scope.providerId}`}
                      data-entitlement-scope-provider={scope.providerId}
                      data-entitlement-scope-partial={partialPool ? "true" : undefined}
                      checked={selected}
                      aria-label={scope.providerLabel}
                      aria-description={describeProvider(scope)}
                      title={`${scope.providerLabel}, ${describeProvider(scope)}`}
                      onChange={() => toggleProvider(scope.providerId)}
                      label={<><span>{scope.providerLabel}</span><small aria-hidden="true">{scope.modelCount}</small></>}
                    />
                  );
                })}
              </div>
            ) : (
              <EmptyState compact title={t("暂无服务商，请先挂载账号", "No providers yet, attach accounts first")} />
            )}
          </>
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
