import { useCallback, useRef, useState } from "react";
import type { ConsoleProviderProbeResponse } from "../../api/contracts";
import type {
  AccountsLedgerPilotAccount,
  AccountsLedgerPilotSection,
} from "./AccountsLedgerWorkspace";
import type { PilotActionDialogState } from "./PilotActionDialog";
import type { RouteManagedAccount } from "./routeAccountCatalog";
import type { useConsoleProbeActions } from "./useConsoleProbeActions";

export function usePilotProbeActions(dialog: PilotActionDialogState | null, account: RouteManagedAccount | null,
  actions: ReturnType<typeof useConsoleProbeActions>) {
  const startPilotProbe = useCallback(async () => {
    if (dialog?.kind === "probe" && account) await actions.handleCredentialProbe(account);
  }, [dialog, account, actions.handleCredentialProbe]);
  const startProviderProbe = useCallback(async (request?: import("../../api/contracts").ConsoleProviderProbeRequest, providerId?: string) => {
    if (dialog?.kind === "provider-probe" || dialog?.kind === "provider-schedule" || dialog?.kind === "probe") {
      return await actions.handleProviderProbe({ providerId: providerId ?? (dialog.kind === "probe" ? dialog.providerId : dialog.section.providerId) }, request);
    }
  }, [dialog, actions.handleProviderProbe]);
  const loadProviderProbeResults = useCallback(async (providerId: string, scope?: import("../../api/contracts").ConsoleProviderProbeRequest["scope"], query?: import("../../api/contracts").CredentialTestResultQuery) => {
    return await actions.handleProviderProbe({ providerId }, { scope, ...query }, true);
  }, [actions.handleProviderProbe]);
  return { startPilotProbe, startProviderProbe, loadProviderProbeResults };
}

export function usePilotDialogSession() {
  const providerProbeGenerationRef = useRef(0);
  const providerProbeAbortRef = useRef<AbortController | null>(null);
  const [pilotActionDialog, setPilotActionDialog] = useState<PilotActionDialogState | null>(null);
  const [providerProbeResponse, setProviderProbeResponse] =
    useState<ConsoleProviderProbeResponse | null>(null);
  const [providerProbeBusy, setProviderProbeBusy] = useState(false);
  const [providerProbeError, setProviderProbeError] = useState<string | null>(null);
  const [pilotScheduleEnabled, setPilotScheduleEnabled] = useState(false);
  const [pilotScheduleIntervalMinutes, setPilotScheduleIntervalMinutes] = useState("60");

  const openPilotProbeDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      setPilotActionDialog({
        kind: "probe",
        providerId,
        account,
      });
    },
    [],
  );

  const openProviderProbeDialog = useCallback((section: AccountsLedgerPilotSection) => {
    setProviderProbeResponse(null);
    setProviderProbeError(null);
    setProviderProbeBusy(false);
    setPilotActionDialog({ kind: "provider-probe", section });
  }, []);

  const closePilotActionDialog = useCallback(() => {
    // Invalidate pending provider probes before releasing the dialog's busy state.
    providerProbeGenerationRef.current += 1;
    providerProbeAbortRef.current?.abort();
    setPilotActionDialog(null);
    setProviderProbeBusy(false);
    setProviderProbeResponse(null);
    setProviderProbeError(null);
  }, []);

  return {
    providerProbeGenerationRef,
    providerProbeAbortRef,
    setPilotActionDialog,
    setProviderProbeResponse,
    setProviderProbeBusy,
    setProviderProbeError,
    pilotActionDialog,
    providerProbeResponse,
    providerProbeBusy,
    providerProbeError,
    pilotScheduleEnabled,
    setPilotScheduleEnabled,
    pilotScheduleIntervalMinutes,
    setPilotScheduleIntervalMinutes,
    openPilotProbeDialog,
    openProviderProbeDialog,
    closePilotActionDialog,
  };
}
