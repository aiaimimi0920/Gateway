import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { settleOperations as settle, settleDatabaseOperations } from "./operationsAvailability";

import type { ConsoleApi, ConsoleRequestFilters } from "../../api/console";
import type {
  ConsoleAlertQueue,
  ConsoleAnalysisExportInventorySummary,
  ConsoleAnomalyIncident,
  ConsoleAnomalyIncidentSummary,
  ConsoleAnomalyPolicy,
  ConsoleGatewayReadiness,
  ConsoleOperatorSummary,
  ConsolePersistedAnalysisExport,
  ConsolePromptCacheSummary,
  ConsoleRateLimitHotspotSummary,
  ConsoleRemediationEffectiveness,
  ConsoleRemediationQueue,
  ConsoleRemediationRun,
  ConsoleRequestAudit,
  ConsoleRequestAuditFullSummary,
  ConsoleRuntimePressure,
  ConsoleUsageAggregateSummary,
} from "../../api/contracts";
import {
  OPERATIONS_DEFAULT_REQUEST_FILTERS,
  type OperationsPanelState,
  type OperationsRequestFilterDraft,
} from "./OperationsWorkspace";

/**
 * One entry per panel the operations workspace renders. Keeping the map in a
 * single state object means one render per refresh instead of seventeen.
 */
type OperationsPanelData = {
  pressure: ConsoleRuntimePressure;
  readiness: ConsoleGatewayReadiness;
  operatorSummary: ConsoleOperatorSummary;
  requests: ConsoleRequestAudit[];
  requestSummary: ConsoleRequestAuditFullSummary;
  usageSummary: ConsoleUsageAggregateSummary;
  promptCache: ConsolePromptCacheSummary;
  incidents: ConsoleAnomalyIncident[];
  incidentSummary: ConsoleAnomalyIncidentSummary;
  alertQueue: ConsoleAlertQueue;
  policies: ConsoleAnomalyPolicy[];
  remediationQueue: ConsoleRemediationQueue;
  remediationRuns: ConsoleRemediationRun[];
  remediationEffectiveness: ConsoleRemediationEffectiveness;
  hotspots: ConsoleRateLimitHotspotSummary;
  exports: ConsolePersistedAnalysisExport[];
  exportInventory: ConsoleAnalysisExportInventorySummary;
};

export type OperationsPanels = {
  [K in keyof OperationsPanelData]: OperationsPanelState<OperationsPanelData[K]>;
};

const PANEL_KEYS = [
  "pressure",
  "readiness",
  "operatorSummary",
  "requests",
  "requestSummary",
  "usageSummary",
  "promptCache",
  "incidents",
  "incidentSummary",
  "alertQueue",
  "policies",
  "remediationQueue",
  "remediationRuns",
  "remediationEffectiveness",
  "hotspots",
  "exports",
  "exportInventory",
] as const satisfies readonly (keyof OperationsPanelData)[];

/** Panels driven by the request filter bar; re-fetched on 应用筛选 alone. */
const REQUEST_PANEL_KEYS = [
  "requests",
  "requestSummary",
  "usageSummary",
  "promptCache",
  "hotspots",
] as const satisfies readonly (keyof OperationsPanelData)[];

function idlePanels(): OperationsPanels {
  const next = {} as OperationsPanels;
  for (const key of PANEL_KEYS) {
    // Each entry is `OperationsPanelState<T>` for its own `T`; the shared empty
    // shape is assignable to every one of them.
    (next as Record<string, OperationsPanelState<unknown>>)[key] = {
      data: null,
      error: null,
      loading: false,
    };
  }
  return next;
}

function markLoading(
  current: OperationsPanels,
  keys: readonly (keyof OperationsPanelData)[],
): OperationsPanels {
  const next = { ...current } as Record<string, OperationsPanelState<unknown>>;
  for (const key of keys) {
    const entry = (current as Record<string, OperationsPanelState<unknown>>)[key];
    next[key] = { ...entry, loading: true };
  }
  return next as OperationsPanels;
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/** Blank strings mean "no filter", so they are dropped before they hit the query. */
function toRequestFilters(draft: OperationsRequestFilterDraft): ConsoleRequestFilters {
  const parsedLimit = Number.parseInt(draft.limit, 10);
  const limit = Number.isFinite(parsedLimit) && parsedLimit > 0 ? Math.min(parsedLimit, 500) : 100;
  const trimmed = (value: string) => (value.trim().length > 0 ? value.trim() : undefined);
  return {
    status: trimmed(draft.status),
    projectId: trimmed(draft.projectId),
    providerAccountId: trimmed(draft.providerAccountId),
    endpointKind: trimmed(draft.endpointKind),
    errorCode: trimmed(draft.errorCode),
    limit,
  };
}

export type UseOperationsDataOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  /** Fetching only starts once the workspace is on screen. */
  active: boolean;
};

export type UseOperationsDataResult = {
  panels: OperationsPanels;
  refreshing: boolean;
  refresh: () => void;
  requestFilters: OperationsRequestFilterDraft;
  setRequestFilters: (next: OperationsRequestFilterDraft) => void;
  applyRequestFilters: () => void;
  incidentBusyId: string | null;
  acknowledgeIncident: (incidentId: string) => void;
  resolveIncident: (incidentId: string) => void;
};

export function useOperationsData({
  api,
  managementToken,
  active,
}: UseOperationsDataOptions): UseOperationsDataResult {
  const [panels, setPanels] = useState<OperationsPanels>(idlePanels);
  const [refreshing, setRefreshing] = useState(false);
  const [requestFilters, setRequestFilters] = useState<OperationsRequestFilterDraft>({
    ...OPERATIONS_DEFAULT_REQUEST_FILTERS,
  });
  const [appliedFilters, setAppliedFilters] = useState<OperationsRequestFilterDraft>({
    ...OPERATIONS_DEFAULT_REQUEST_FILTERS,
  });
  const [incidentBusyId, setIncidentBusyId] = useState<string | null>(null);

  // A later refresh always wins, so a slow first response cannot overwrite it.
  const generationRef = useRef(0);
  const filters = useMemo(() => toRequestFilters(appliedFilters), [appliedFilters]);

  const loadRequestPanels = useCallback(
    async (generation: number, activeFilters: ConsoleRequestFilters,
      summary: Promise<OperationsPanelState<ConsoleOperatorSummary>>) => {
      const [requests, requestSummary, usageSummary, promptCache, hotspots] = await Promise.all([
        settle(
          managementToken && typeof api.listRequestAudits === "function"
            ? () =>
                api.listRequestAudits!(managementToken, activeFilters).then(
                  (response) => response.requests,
                )
            : null,
        ),
        settle(
          managementToken && typeof api.getRequestAuditFullSummary === "function"
            ? () =>
                api.getRequestAuditFullSummary!(managementToken, activeFilters).then(
                  (response) => response.summary,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getUsageAggregateSummary === "function"
            ? () =>
                api.getUsageAggregateSummary!(managementToken, {
                  limit: activeFilters.limit,
                }).then((response) => response.summary)
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getPromptCacheSummary === "function"
            ? () =>
                api.getPromptCacheSummary!(managementToken, activeFilters).then(
                  (response) => response.summary,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getRateLimitHotspots === "function"
            ? () =>
                api.getRateLimitHotspots!(managementToken, activeFilters).then(
                  (response) => response.summary,
                )
            : null,
        ),
      ]);
      if (generation !== generationRef.current) {
        return;
      }
      setPanels((current) => ({
        ...current,
        requests,
        requestSummary,
        usageSummary,
        promptCache,
        hotspots,
      }));
    },
    [api, managementToken],
  );

  const loadAll = useCallback(
    async (generation: number, activeFilters: ConsoleRequestFilters) => {
      setRefreshing(true);
      const summary = settle(
        managementToken && typeof api.getOperatorSummary === "function"
          ? () => api.getOperatorSummary!(managementToken).then(response => response.summary)
          : null,
      );
      const requestPanelsPromise = loadRequestPanels(generation, activeFilters, summary);
      const [
        pressure,
        readiness,
        operatorSummary,
        incidents,
        incidentSummary,
        alertQueue,
        policies,
        remediationQueue,
        remediationRuns,
        remediationEffectiveness,
        exports,
        exportInventory,
      ] = await Promise.all([
        settle(
          managementToken && typeof api.getRuntimePressure === "function"
            ? () => api.getRuntimePressure!(managementToken).then((response) => response.pressure)
            : null,
        ),
        settle(
          managementToken && typeof api.getGatewayReadiness === "function"
            ? () => api.getGatewayReadiness!(managementToken).then((response) => response.readiness)
            : null,
        ),
        summary,
        settleDatabaseOperations(summary,
          managementToken && typeof api.listAnomalyIncidents === "function"
            ? () =>
                api.listAnomalyIncidents!(managementToken, { limit: 50 }).then(
                  (response) => response.incidents,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getAnomalyIncidentSummary === "function"
            ? () =>
                api.getAnomalyIncidentSummary!(managementToken).then((response) => response.summary)
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getAnomalyAlertQueue === "function"
            ? () => api.getAnomalyAlertQueue!(managementToken).then((response) => response.queue)
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.listAnomalyPolicies === "function"
            ? () =>
                api.listAnomalyPolicies!(managementToken, { limit: 50 }).then(
                  (response) => response.policies,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.listRemediationQueue === "function"
            ? () => api.listRemediationQueue!(managementToken).then((response) => response.queue)
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.listRemediationRuns === "function"
            ? () =>
                api.listRemediationRuns!(managementToken, { limit: 50 }).then(
                  (response) => response.runs,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getRemediationEffectiveness === "function"
            ? () =>
                api.getRemediationEffectiveness!(managementToken).then(
                  (response) => response.effectiveness,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.listPersistedAnalysisExports === "function"
            ? () =>
                api.listPersistedAnalysisExports!(managementToken, { limit: 50 }).then(
                  (response) => response.exports,
                )
            : null,
        ),
        settleDatabaseOperations(summary,
          managementToken && typeof api.getAnalysisExportInventorySummary === "function"
            ? () =>
                api.getAnalysisExportInventorySummary!(managementToken).then(
                  (response) => response.summary,
                )
            : null,
        ),
      ]);
      if (generation !== generationRef.current) {
        return;
      }
      setPanels((current) => ({
        ...current,
        pressure,
        readiness,
        operatorSummary,
        incidents,
        incidentSummary,
        alertQueue,
        policies,
        remediationQueue,
        remediationRuns,
        remediationEffectiveness,
        exports,
        exportInventory,
      }));
      await requestPanelsPromise;
      if (generation === generationRef.current) {
        setRefreshing(false);
      }
    },
    [api, loadRequestPanels, managementToken],
  );

  useEffect(() => {
    if (!active || !managementToken) {
      return;
    }
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    setPanels((current) => markLoading(current, PANEL_KEYS));
    void loadAll(generation, filters);
  }, [active, filters, loadAll, managementToken]);

  const refresh = useCallback(() => {
    if (!managementToken) {
      return;
    }
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    setPanels((current) => markLoading(current, PANEL_KEYS));
    void loadAll(generation, filters);
  }, [filters, loadAll, managementToken]);

  const applyRequestFilters = useCallback(() => {
    // Committing the draft changes `filters`, and the effect above reloads.
    setAppliedFilters({ ...requestFilters });
  }, [requestFilters]);

  const mutateIncident = useCallback(
    async (incidentId: string, kind: "acknowledge" | "resolve") => {
      const run =
        kind === "acknowledge" ? api.acknowledgeAnomalyIncident : api.resolveAnomalyIncident;
      if (!managementToken || typeof run !== "function") {
        return;
      }
      setIncidentBusyId(incidentId);
      try {
        const response = await run(managementToken, incidentId);
        setPanels((current) => {
          const rows = current.incidents.data;
          if (!rows) {
            return current;
          }
          return {
            ...current,
            incidents: {
              ...current.incidents,
              data: rows.map((row) => (row.id === incidentId ? response.incident : row)),
            },
          };
        });
        // The counters and the alert queue both move with the incident state.
        const generation = generationRef.current + 1;
        generationRef.current = generation;
        void loadAll(generation, filters);
      } catch (cause) {
        setPanels((current) => ({
          ...current,
          incidents: { ...current.incidents, error: errorMessage(cause) },
        }));
      } finally {
        setIncidentBusyId((busy) => (busy === incidentId ? null : busy));
      }
    },
    [api, filters, loadAll, managementToken],
  );

  const acknowledgeIncident = useCallback(
    (incidentId: string) => void mutateIncident(incidentId, "acknowledge"),
    [mutateIncident],
  );
  const resolveIncident = useCallback(
    (incidentId: string) => void mutateIncident(incidentId, "resolve"),
    [mutateIncident],
  );

  return {
    panels,
    refreshing,
    refresh,
    requestFilters,
    setRequestFilters,
    applyRequestFilters,
    incidentBusyId,
    acknowledgeIncident,
    resolveIncident,
  };
}

export { REQUEST_PANEL_KEYS };
