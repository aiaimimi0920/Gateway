import { useConsoleCredentialSelectors } from "./useConsoleCredentialSelectors";
import { useCredentialProbeState } from "./useCredentialProbeState";
import { type ConsoleWorkspaceId } from "./consoleNavigation";
import { useConsoleDraftPersistence } from "./useConsoleDraftPersistence";
import { useProviderCatalogEditor } from "./useProviderCatalogEditor";
import { useGeminiCredentialImport } from "./useGeminiCredentialImport";
import { useConsoleActionIdentity } from "./useConsoleActionIdentity";
import { useConsoleGroupSelection } from "./useConsoleGroupSelection";
import { useConsoleProviderMetrics } from "./useConsoleProviderMetrics";
import { useConsoleAccountSelectors } from "./useConsoleAccountSelectors";
import { useConsoleModelSelectors } from "./useConsoleModelSelectors";
import { useCredentialPoolActions } from "./useCredentialPoolActions";
import { useConsoleProbeActions } from "./useConsoleProbeActions";
import { usePilotDialogSession } from "./usePilotDialogSession";
import { useProbeScheduleEditor } from "./useProbeScheduleEditor";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { usePilotPolicyEditor } from "./usePilotPolicyEditor";
import { useModelPoolEditor } from "./useModelPoolEditor";
import { useAccountGroupEditor } from "./useAccountGroupEditor";
import { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import { useConsoleRouteData } from "./useConsoleRouteData";
import { useGeminiManualAddSession } from "./useGeminiManualAddSession";
import {
  type AccountMembershipFilter,
  type AccountEnabledFilter,
} from "./routeAccountCatalog";
import { accountGroupDraftRowsFromDocument } from "./accountGroupDraft";
import {
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { createGatewayApiClient } from "../../api/client";
import {
  createConsoleApi,
  type ConsoleApi,
} from "../../api/console";
import { useAccessData } from "./useAccessData";
import { useOperationsData } from "./useOperationsData";
import {
  type ModelPoolModelDialogMode,
  type ModelPoolModelDialogValue,
} from "./ModelPoolModelDialog";
import { type PendingCredentialRemoval } from "./ProviderAccountCard";
import {
  type CredentialDialogMode,
  type CredentialDialogValue,
} from "./CredentialDialog";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

type CredentialDialogState = {
  mode: CredentialDialogMode;
  initialValue: CredentialDialogValue | null;
};

export function useConsoleController(consoleApi?: ConsoleApi) {
  const host = useGatewayHost();
  const session = useManagementSession();
  const { locale, t } = useUiLocale();
  const client = useMemo(() => createGatewayApiClient({ host }), [host]);
  const api = useMemo(() => consoleApi ?? createConsoleApi(client), [client, consoleApi]);
  const managementToken = session.managementToken;
  const {
    consoleActionGenerationRef,
    currentApiRef,
    currentManagementTokenRef,
    currentSecretGrantRef,
    currentSecretGrantEpochRef,
    beginConsoleActionRequest,
    isConsoleActionRequestCurrent,
    isConsoleActionRecoveryCurrent,
    actionBusy,
    setActionBusy,
  } = useConsoleActionIdentity({ api, managementToken, session });
  const [secretDialogOpen, setSecretDialogOpen] = useState(false);
  const [credentialDialogState, setCredentialDialogState] =
    useState<CredentialDialogState | null>(null);
  const [providerCatalogDialogOpen, setProviderCatalogDialogOpen] = useState(false);
  const [modelPoolDialog, setModelPoolDialog] = useState<{
    mode: ModelPoolModelDialogMode;
    initial: ModelPoolModelDialogValue | null;
  } | null>(null);
  const [pendingModelPoolRemoval, setPendingModelPoolRemoval] = useState<string | null>(null);
  /** The provider whose `model_map` the mapping dialog is editing. */
  const [providerModelMappingProviderId, setProviderModelMappingProviderId] = useState<
    string | null
  >(null);
  const {
    providerProbeGenerationRef,
    setPilotActionDialog,
    setProviderProbeResponse,
    setProviderProbeBusy,
    setProviderProbeError,
    pilotActionDialog,
    ...pilotDialogSession
  } = usePilotDialogSession();
  const [activeWorkspace, setActiveWorkspace] = useState<ConsoleWorkspaceId>("accounts");
  const [consoleRailCollapsed, setConsoleRailCollapsed] = useState(false);
  // Both of these own their own fetching, keyed off the active workspace, so
  // nothing is requested until the operator actually opens the page.
  const operations = useOperationsData({
    api,
    managementToken,
    active: activeWorkspace === "operations",
  });
  const access = useAccessData({
    api,
    managementToken,
    active: activeWorkspace === "access",
    t,
  });
  const [accountSearch, setAccountSearch] = useState("");
  const [accountMembershipFilter, setAccountMembershipFilter] =
    useState<AccountMembershipFilter>("all");
  const [accountEnabledFilter, setAccountEnabledFilter] = useState<AccountEnabledFilter>("all");
  const [selectedAccountGroupFilter, setSelectedAccountGroupFilter] = useState("all");
  const [selectedProviderFilter, setSelectedProviderFilter] = useState("all");
  const {
    credentialProbeGenerationRef,
    credentialProbeBusy,
    setCredentialProbeBusy,
    credentialProbeResults,
    setCredentialProbeResults,
    invalidateCredentialProbes,
    handleSecretAccessRequiredError,
  } = useCredentialProbeState({
    clearSecretGrant: session.clearSecretGrant,
    setSecretDialogOpen,
  });

  const {
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
  } = useConsoleRouteData({ api, managementToken, t, invalidateCredentialProbes });

  const hasSecretAccess = Boolean(session.session?.secretAccessGranted);
  const onRouteConfigHydrated = useCallback(() => {
    setCredentialDialogState(null);
    setProviderCatalogDialogOpen(false);
    setCredentialProbeBusy(null);
    setCredentialProbeResults({});
  }, [setCredentialProbeBusy, setCredentialProbeResults]);
  const {
    editorText,
    commitMessage,
    credentialSecretEdits,
    setCredentialSecretEdits,
    modelRouteDraftRows,
    setModelRouteDraftRows,
    providerDraftRows,
    accountGroupDraftRows,
    setAccountGroupDraftRows,
    activeSecretPatches,
    secretPatches,
    draftDocumentState,
    draftMatchesActiveRevision,
    incompleteAccountGroupDraftCount,
    draftDirty,
    buildCommitRequest,
    parseDraft,
    replaceEditorDocument,
    updateProviderStoragePassword,
  } = useConsoleRouteDraft({
    routeConfig,
    hasSecretAccess,
    t,
    invalidateCredentialProbes,
    accountGroupDraftRowsFromDocument,
    onRouteConfigHydrated,
    setError,
    setSecretDialogOpen,
  });

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const {
    draftAccountCatalog,
    displayedAccountCatalog,
    filteredAccountLedgerRows,
    consoleTelemetry,
    accountPilotSections,
    accountCardMetricsById,
    pilotAccountsById,
    accountLedgerGroupOptions,
    accountLedgerProviderOptions,
    activePilotManagedAccount,
    activePilotProbeResult,
    activePilotStatsView,
  } = useConsoleAccountSelectors({
    draftDocumentState,
    routeConfig,
    accountGroupSummary,
    draftMatchesActiveRevision,
    credentialProbeResults,
    runtimePressure,
    costOverview,
    requestAuditSummary,
    credentialModelStates,
    providerCredentialInventory,
    accountSearch,
    accountMembershipFilter,
    accountEnabledFilter,
    selectedAccountGroupFilter,
    selectedProviderFilter,
    locale,
    pilotActionDialog,
    t,
  });

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

  const [pendingGroupAccountRemoval, setPendingGroupAccountRemoval] =
    useState<PendingCredentialRemoval | null>(null);
  const credentialSelectors = useConsoleCredentialSelectors({
    credentialPoolAutomation,
    credentialRefill,
    draftDocumentState,
    draftAccountCatalog,
  });
  const { providerModelMapEntries, providerMetricsResolver } = useConsoleProviderMetrics({
    providerDraftRows,
    consoleTelemetry,
    accountGroupSummary,
  });
  const groupSelection = useConsoleGroupSelection({
    displayedAccountCatalog,
    accountGroupDraftRows,
    accountCardMetricsById,
    providerMetricsResolver,
  });
  // The model pool inverts the same catalog: one card per model, each carrying
  // the provider fallback chain that `model_routes` orders.
  const {
    modelPoolDirectory,
    modelPoolDialogProviderOptions,
    modelPoolModelNames,
    modelMappingCountByProvider,
    providerModelMappingTarget,
  } = useConsoleModelSelectors({
    displayedAccountCatalog,
    accountCardMetricsById,
    providerMetricsResolver,
    providerDraftRows,
    modelRouteDraftRows,
    providerModelMapEntries,
    accountPilotSections,
    providerModelMappingProviderId,
  });

  const { applyGeminiManualAddSessionResult } = useGeminiCredentialImport({
    managementToken,
    editorText,
    credentialSecretEdits,
    setCredentialSecretEdits,
    replaceEditorDocument,
    buildCommitRequest,
    activeSecretPatches,
    routeConfig,
    setRouteConfig,
    refresh,
    setError,
    consoleActionGenerationRef,
    beginConsoleActionRequest,
    isConsoleActionRequestCurrent,
    isConsoleActionRecoveryCurrent,
    setActionBusy,
    handleSecretAccessRequiredError,
    t,
  });

  const geminiManualAdd = useGeminiManualAddSession({
    api, managementToken, applyGeminiManualAddSessionResult, setError, t,
  });

  const pilotPolicyEditor = usePilotPolicyEditor({
    editorText,
    setError,
    replaceEditorDocument,
    credentialPoolAutomation,
    credentialPoolAutomationByProvider: credentialSelectors.credentialPoolAutomationByProvider,
    credentialRefill,
    t,
  });

  const credentialPoolActions = useCredentialPoolActions({
    api,
    managementToken,
    draftDirty,
    credentialPoolAutomationByProvider: credentialSelectors.credentialPoolAutomationByProvider,
    credentialRefillByProvider: credentialSelectors.credentialRefillByProvider,
    setCredentialPoolAutomation,
    refresh,
    setError,
    t,
  });

  const { updateCredentialSecretEdit, ...credentialDialogEditor } = useCredentialDialogEditor({
    editorText,
    credentialDialogState,
    setCredentialDialogState,
    setCredentialSecretEdits,
    setError,
    replaceEditorDocument,
    t,
  });

  const { applyProviderCatalogDraft } = useProviderCatalogEditor({
    editorText,
    replaceEditorDocument,
    updateCredentialSecretEdit,
    setActiveWorkspace,
    setError,
    t,
  });

  const probeScheduleEditor = useProbeScheduleEditor({
    editorText,
    pilotActionDialog,
    setPilotActionDialog,
    pilotScheduleEnabled: pilotDialogSession.pilotScheduleEnabled,
    setPilotScheduleEnabled: pilotDialogSession.setPilotScheduleEnabled,
    pilotScheduleIntervalMinutes: pilotDialogSession.pilotScheduleIntervalMinutes,
    setPilotScheduleIntervalMinutes: pilotDialogSession.setPilotScheduleIntervalMinutes,
    setError,
    replaceEditorDocument,
    t,
  });

  const { handleCredentialProbe, handleProviderProbe } = useConsoleProbeActions({
    api,
    managementToken,
    session,
    draftDirty,
    draftMatchesActiveRevision,
    credentialProbeGenerationRef,
    providerProbeGenerationRef,
    currentApiRef,
    currentManagementTokenRef,
    currentSecretGrantEpochRef,
    currentSecretGrantRef,
    handleSecretAccessRequiredError,
    setCredentialProbeBusy,
    setCredentialProbeResults,
    setProviderProbeBusy,
    setProviderProbeError,
    setProviderProbeResponse,
    setSecretDialogOpen,
    setError,
    t,
  });

  const startPilotProbe = useCallback(async () => {
    if (pilotActionDialog?.kind !== "probe" || !activePilotManagedAccount) {
      return;
    }
    await handleCredentialProbe(activePilotManagedAccount);
  }, [activePilotManagedAccount, handleCredentialProbe, pilotActionDialog]);

  const startProviderProbe = useCallback(async () => {
    if (pilotActionDialog?.kind !== "provider-probe") {
      return;
    }
    await handleProviderProbe(pilotActionDialog.section);
  }, [handleProviderProbe, pilotActionDialog]);

  const modelPoolEditor = useModelPoolEditor({
    editorText,
    modelRouteDraftRows,
    setModelRouteDraftRows,
    setError,
    replaceEditorDocument,
    modelPoolDirectory,
    modelPoolDialog,
    setModelPoolDialog,
    providerModelMappingProviderId,
    setProviderModelMappingProviderId,
    t,
  });

  const accountGroupEditor = useAccountGroupEditor({
    editorText,
    accountGroupDraftRows,
    setAccountGroupDraftRows,
    setError,
    replaceEditorDocument,
    setSelectedAccountGroupRowId: groupSelection.setSelectedAccountGroupRowId,
    setGroupMemberQuery: groupSelection.setGroupMemberQuery,
    setGroupMemberMode: groupSelection.setGroupMemberMode,
    t,
  });

  const activeRouteDiagnostics = routeConfig?.routeConfig.diagnostics?.diagnostics ?? [];
  const mutationSupported = routeConfig?.routeConfig.mutationSupported ?? false;
  const editorLocked = busy || actionBusy !== null || !mutationSupported;
  const { autosavePending } = useConsoleDraftPersistence({
    managementToken,
    editorLocked,
    selectedGroupIdInvalid: groupSelection.selectedGroupIdInvalid,
    selectedGroupBillingInvalid: groupSelection.selectedGroupBillingInvalid,
    editorText,
    commitMessage,
    secretPatches,
    draftDirty,
    draftDocumentState,
    incompleteAccountGroupDraftCount,
    parseDraft,
    setRouteConfig,
    refresh,
    setError,
    consoleActionGenerationRef,
    beginConsoleActionRequest,
    isConsoleActionRequestCurrent,
    isConsoleActionRecoveryCurrent,
    setActionBusy,
    handleSecretAccessRequiredError,
    t,
  });

  return {
    client,
    ...accountGroupEditor,
    ...credentialDialogEditor,
    ...credentialPoolActions,
    ...credentialSelectors,
    ...geminiManualAdd,
    ...groupSelection,
    ...modelPoolEditor,
    ...pilotPolicyEditor,
    ...probeScheduleEditor,
    access,
    accountGroupSummaryError,
    accountLedgerGroupOptions,
    accountLedgerProviderOptions,
    accountPilotSections,
    actionBusy,
    activePilotManagedAccount,
    activePilotProbeResult,
    activePilotStatsView,
    activeRouteDiagnostics,
    activeWorkspace,
    applyProviderCatalogDraft,
    autosavePending,
    busy,
    consoleRailCollapsed,
    credentialDialogState,
    credentialPoolAutomationError,
    credentialProbeBusy,
    credentialRefillError,
    displayedAccountCatalog,
    draftDirty,
    draftDocumentState,
    draftMatchesActiveRevision,
    editorLocked,
    error,
    filteredAccountLedgerRows,
    hasSecretAccess,
    modelMappingCountByProvider,
    modelPoolDialog,
    modelPoolDialogProviderOptions,
    modelPoolDirectory,
    modelPoolModelNames,
    mutationSupported,
    operations,
    pendingGroupAccountRemoval,
    pendingModelPoolRemoval,
    pilotAccountsById,
    pilotActionDialog,
    ...pilotDialogSession,
    providerCatalogDialogOpen,
    providerCredentialInventoryError,
    providerModelMappingTarget,
    refresh,
    routeConfig,
    secretDialogOpen,
    session,
    setActiveWorkspace,
    setConsoleRailCollapsed,
    setCredentialDialogState,
    setModelPoolDialog,
    setPendingGroupAccountRemoval,
    setPendingModelPoolRemoval,
    setProviderCatalogDialogOpen,
    setProviderModelMappingProviderId,
    setSecretDialogOpen,
    startPilotProbe,
    startProviderProbe,
    t,
    telemetryError,
    updateProviderStoragePassword,
  };
}
