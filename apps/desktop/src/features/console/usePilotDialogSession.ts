import { useCallback, useRef, useState } from "react";
import type { ConsoleProviderProbeResponse } from "../../api/contracts";
import type {
  AccountsLedgerPilotAccount,
  AccountsLedgerPilotSection,
} from "./AccountsLedgerWorkspace";
import type { PilotActionDialogState } from "./PilotActionDialog";

export function usePilotDialogSession() {
  const providerProbeGenerationRef = useRef(0);
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
    setPilotActionDialog(null);
    setProviderProbeBusy(false);
  }, []);

  return {
    providerProbeGenerationRef,
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
