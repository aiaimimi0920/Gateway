import { useCallback, useRef, useState } from "react";
import { GatewayApiError } from "../../api/errors";
import type { CredentialProbeViewResult } from "./useConsoleProbeActions";

type CredentialProbeStateOptions = {
  clearSecretGrant: () => void;
  setSecretDialogOpen: (open: boolean) => void;
};

export function useCredentialProbeState({
  clearSecretGrant,
  setSecretDialogOpen,
}: CredentialProbeStateOptions) {
  const credentialProbeGenerationRef = useRef(0);
  const [credentialProbeBusy, setCredentialProbeBusy] = useState<string | null>(null);
  const [credentialProbeResults, setCredentialProbeResults] = useState<
    Record<string, CredentialProbeViewResult>
  >({});

  const invalidateCredentialProbes = useCallback(() => {
    credentialProbeGenerationRef.current += 1;
    setCredentialProbeBusy(null);
    setCredentialProbeResults((current) =>
      Object.keys(current).length === 0 ? current : {},
    );
  }, []);

  const handleSecretAccessRequiredError = useCallback(
    (cause: unknown, isCurrentRequest: () => boolean): "not-required" | "recovered" | "stale" => {
      if (!(cause instanceof GatewayApiError && cause.status === 403 &&
        cause.code === "console_secret_access_required")) {
        return "not-required";
      }
      if (!isCurrentRequest()) {
        return "stale";
      }
      clearSecretGrant();
      invalidateCredentialProbes();
      setSecretDialogOpen(true);
      return "recovered";
    },
    [clearSecretGrant, invalidateCredentialProbes, setSecretDialogOpen],
  );

  return {
    credentialProbeGenerationRef,
    credentialProbeBusy,
    setCredentialProbeBusy,
    credentialProbeResults,
    setCredentialProbeResults,
    invalidateCredentialProbes,
    handleSecretAccessRequiredError,
  };
}
