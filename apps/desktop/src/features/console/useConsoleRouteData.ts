import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { ConsoleApi } from "../../api/console";
import type {
  ConsoleRouteConfigResponse, ConsoleAccountGroupSummaryResponse,
  ConsoleProviderCredentialInventoryResponse, ConsoleCredentialPoolAutomationResponse,
  ConsoleCredentialRefillResponse, ConsoleRuntimePressure, ConsoleCostOverview,
  ConsoleRequestAuditSummary, ConsoleProviderCredentialModelState,
} from "../../api/contracts";
import { ConsoleRefreshSingleFlight } from "./consoleRefreshSingleFlight";

type ConsoleRouteDataOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  t: (zh: string, en: string) => string;
  invalidateCredentialProbes: () => void;
};

export function useConsoleRouteData({ api, managementToken, t, invalidateCredentialProbes }: ConsoleRouteDataOptions) {
  const translationRef = useRef(t);

  const refreshRequestIdRef = useRef(0);

  const refreshSingleFlightRef = useRef<ConsoleRefreshSingleFlight | null>(null);

  if (refreshSingleFlightRef.current === null) {
    refreshSingleFlightRef.current = new ConsoleRefreshSingleFlight();
  }

  const mounted = useRef(false);
  const refreshLifecycle = useRef(0);

  useLayoutEffect(() => {
    mounted.current = true;
    setRouteConfig(null);
    setAccountGroupSummary(null);
    setAccountGroupSummaryError(null);
    setProviderCredentialInventory(null);
    setProviderCredentialInventoryError(null);
    setCredentialPoolAutomation(null);
    setCredentialPoolAutomationError(null);
    setCredentialRefill(null);
    setCredentialRefillError(null);
    setRuntimePressure(null);
    setCostOverview(null);
    setRequestAuditSummary(null);
    setCredentialModelStates(null);
    setTelemetryError(null);
    setError(null);
    setBusy(Boolean(managementToken));
    return () => {
      mounted.current = false;
      refreshLifecycle.current += 1;
      refreshRequestIdRef.current += 1;
      // A restored token must not join work from its previous session lifetime.
      refreshSingleFlightRef.current = new ConsoleRefreshSingleFlight();
    };
  }, [api, managementToken]);

  translationRef.current = t;

  const [routeConfig, setRouteConfig] = useState<ConsoleRouteConfigResponse | null>(null);

  const [accountGroupSummary, setAccountGroupSummary] =
    useState<ConsoleAccountGroupSummaryResponse["summary"] | null>(null);

  const [accountGroupSummaryError, setAccountGroupSummaryError] = useState<string | null>(null);

  const [providerCredentialInventory, setProviderCredentialInventory] =
    useState<ConsoleProviderCredentialInventoryResponse | null>(null);

  const [providerCredentialInventoryError, setProviderCredentialInventoryError] =
    useState<string | null>(null);

  const [credentialPoolAutomation, setCredentialPoolAutomation] =
    useState<ConsoleCredentialPoolAutomationResponse | null>(null);

  const [credentialPoolAutomationError, setCredentialPoolAutomationError] =
    useState<string | null>(null);

  const [credentialRefill, setCredentialRefill] =
    useState<ConsoleCredentialRefillResponse | null>(null);

  const [credentialRefillError, setCredentialRefillError] = useState<string | null>(null);

  const [runtimePressure, setRuntimePressure] = useState<ConsoleRuntimePressure | null>(null);

  const [costOverview, setCostOverview] = useState<ConsoleCostOverview | null>(null);

  const [requestAuditSummary, setRequestAuditSummary] =
    useState<ConsoleRequestAuditSummary | null>(null);

  const [credentialModelStates, setCredentialModelStates] = useState<
    ConsoleProviderCredentialModelState[] | null
  >(null);

  /** One banner for the whole telemetry group; individual gaps degrade to `—`. */
  const [telemetryError, setTelemetryError] = useState<string | null>(null);

  const [busy, setBusy] = useState(true);

  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback((): Promise<void> => {
    if (!mounted.current) return Promise.resolve();
    if (!managementToken) {
      setError(
        translationRef.current(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        ),
      );
      setBusy(false);
      return Promise.resolve();
    }
    const lifecycle = refreshLifecycle.current;
    return refreshSingleFlightRef.current!.run(api, managementToken, async () => {
      if (!mounted.current || lifecycle !== refreshLifecycle.current) return;
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
        const providerCredentialInventoryPromise =
          typeof api.getProviderCredentialInventory === "function"
            ? api
                .getProviderCredentialInventory(managementToken)
                .then((response) => ({ response, error: null as string | null }))
                .catch((cause: unknown) => ({
                  response: null,
                  error: cause instanceof Error ? cause.message : String(cause),
                }))
            : Promise.resolve({ response: null, error: null as string | null });
        const credentialRefillPromise = api
          .getCredentialRefill(managementToken)
          .then((response) => ({ response, error: null as string | null }))
          .catch((cause: unknown) => ({
            response: null,
            error: cause instanceof Error ? cause.message : String(cause),
          }));
        const runtimePressurePromise =
          typeof api.getRuntimePressure === "function"
            ? api
                .getRuntimePressure(managementToken)
                .then((response) => ({
                  pressure: response.pressure,
                  error: null as string | null,
                }))
                .catch((cause: unknown) => ({
                  pressure: null,
                  error: cause instanceof Error ? cause.message : String(cause),
                }))
            : Promise.resolve({ pressure: null, error: null as string | null });
        const costOverviewPromise =
          typeof api.getCostOverview === "function"
            ? api
                .getCostOverview(managementToken)
                .then((response) => ({
                  overview: response.overview,
                  error: null as string | null,
                }))
                .catch((cause: unknown) => ({
                  overview: null,
                  error: cause instanceof Error ? cause.message : String(cause),
                }))
            : Promise.resolve({ overview: null, error: null as string | null });
        const requestAuditSummaryPromise =
          typeof api.getRequestAuditSummary === "function"
            ? api
                .getRequestAuditSummary(managementToken)
                .then((response) => ({
                  summary: response.summary,
                  error: null as string | null,
                }))
                .catch((cause: unknown) => ({
                  summary: null,
                  error: cause instanceof Error ? cause.message : String(cause),
                }))
            : Promise.resolve({ summary: null, error: null as string | null });
        const credentialModelStatesPromise =
          typeof api.listProviderCredentialModelStates === "function"
            ? api
                .listProviderCredentialModelStates(managementToken)
                .then((response) => ({ states: response.states, error: null as string | null }))
                .catch((cause: unknown) => ({
                  states: null,
                  error: cause instanceof Error ? cause.message : String(cause),
                }))
            : Promise.resolve({ states: null, error: null as string | null });
        const [
          nextRouteConfig,
          nextAccountGroupSummary,
          nextProviderCredentialInventory,
          nextCredentialPoolAutomation,
          nextCredentialRefill,
          nextRuntimePressure,
          nextCostOverview,
          nextRequestAuditSummary,
          nextCredentialModelStates,
        ] = await Promise.all([
          api.getRouteConfig(managementToken),
          accountGroupSummaryPromise,
          providerCredentialInventoryPromise,
          credentialPoolAutomationPromise,
          credentialRefillPromise,
          runtimePressurePromise,
          costOverviewPromise,
          requestAuditSummaryPromise,
          credentialModelStatesPromise,
        ]);
        if (requestId !== refreshRequestIdRef.current) {
          return;
        }
        setRouteConfig({ ...nextRouteConfig });
        setAccountGroupSummary(nextAccountGroupSummary.summary);
        setAccountGroupSummaryError(nextAccountGroupSummary.error);
        setProviderCredentialInventory(nextProviderCredentialInventory.response);
        setProviderCredentialInventoryError(nextProviderCredentialInventory.error);
        setCredentialPoolAutomation(nextCredentialPoolAutomation.response);
        setCredentialPoolAutomationError(nextCredentialPoolAutomation.error);
        setCredentialRefill(nextCredentialRefill.response);
        setCredentialRefillError(nextCredentialRefill.error);
        setRuntimePressure(nextRuntimePressure.pressure);
        setCostOverview(nextCostOverview.overview);
        setRequestAuditSummary(nextRequestAuditSummary.summary);
        setCredentialModelStates(nextCredentialModelStates.states);
        setTelemetryError(
          [
            nextRuntimePressure.error,
            nextCostOverview.error,
            nextRequestAuditSummary.error,
            nextCredentialModelStates.error,
          ].find((message) => message !== null) ?? null,
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
    });
  }, [api, invalidateCredentialProbes, managementToken]);

  return {
    routeConfig,
    accountGroupSummary,
    accountGroupSummaryError,
    providerCredentialInventory,
    providerCredentialInventoryError,
    credentialPoolAutomation,
    credentialPoolAutomationError,
    credentialRefill,
    credentialRefillError,
    runtimePressure,
    costOverview,
    requestAuditSummary,
    credentialModelStates,
    telemetryError,
    busy,
    error,
    setRouteConfig,
    setCredentialPoolAutomation,
    setError,
    refresh,
  };
}
