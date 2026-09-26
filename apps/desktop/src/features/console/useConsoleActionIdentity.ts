import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { ConsoleApi } from "../../api/console";
import type { useManagementSession } from "../../session/useManagementSession";

export type ConsoleActionRequest = {
  api: ConsoleApi;
  generation: number;
  managementToken: string;
  secretGrant: string | null;
  secretGrantEpoch: number;
};

type ConsoleActionIdentityOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  session: Pick<ReturnType<typeof useManagementSession>, "secretGrant">;
};

export function useConsoleActionIdentity({ api, managementToken, session }: ConsoleActionIdentityOptions) {
  const consoleActionGenerationRef = useRef(0);
  const [actionBusy, setActionBusy] = useState<"save" | null>(null);
  const mounted = useRef(false);
  useLayoutEffect(() => {
    mounted.current = true;
    setActionBusy(null);
    return () => {
      mounted.current = false;
      // Restoring the same host/token must not revive an earlier request.
      consoleActionGenerationRef.current += 1;
    };
  }, [api, managementToken]);

  const currentApiRef = useRef(api);

  const currentManagementTokenRef = useRef(managementToken);

  const currentSecretGrant = session.secretGrant?.grant ?? null;

  const observedSecretGrantRef = useRef(currentSecretGrant);

  const currentSecretGrantRef = useRef(currentSecretGrant);

  const currentSecretGrantEpochRef = useRef(0);

  currentApiRef.current = api;

  currentManagementTokenRef.current = managementToken;

  useLayoutEffect(() => {
    if (observedSecretGrantRef.current !== currentSecretGrant) {
      observedSecretGrantRef.current = currentSecretGrant;
      currentSecretGrantEpochRef.current += 1;
    }
    currentSecretGrantRef.current = currentSecretGrant;
  }, [currentSecretGrant]);

  const beginConsoleActionRequest = useCallback(
    (requestManagementToken: string): ConsoleActionRequest => {
      const generation = consoleActionGenerationRef.current + 1;
      consoleActionGenerationRef.current = generation;
      return {
        api,
        generation,
        managementToken: requestManagementToken,
        secretGrant: session.secretGrant?.grant ?? null,
        secretGrantEpoch: currentSecretGrantEpochRef.current,
      };
    },
    [api, session.secretGrant?.grant],
  );

  const isConsoleActionIdentityCurrent = useCallback(
    (request: ConsoleActionRequest) =>
      mounted.current &&
      request.generation === consoleActionGenerationRef.current &&
      request.api === currentApiRef.current &&
      request.managementToken === currentManagementTokenRef.current,
    [],
  );

  const isConsoleActionRequestCurrent = useCallback(
    (request: ConsoleActionRequest) =>
      isConsoleActionIdentityCurrent(request) &&
      request.secretGrantEpoch === currentSecretGrantEpochRef.current &&
      request.secretGrant === currentSecretGrantRef.current,
    [isConsoleActionIdentityCurrent],
  );

  const isConsoleActionRecoveryCurrent = useCallback(
    (request: ConsoleActionRequest) => {
      if (!isConsoleActionIdentityCurrent(request)) {
        return false;
      }
      const currentGrant = currentSecretGrantRef.current;
      const currentEpoch = currentSecretGrantEpochRef.current;
      return (
        (currentEpoch === request.secretGrantEpoch && currentGrant === request.secretGrant) ||
        (request.secretGrant !== null &&
          currentGrant === null &&
          currentEpoch === request.secretGrantEpoch + 1)
      );
    },
    [isConsoleActionIdentityCurrent],
  );

  return {
    actionBusy,
    setActionBusy,
    consoleActionGenerationRef,
    currentApiRef,
    currentManagementTokenRef,
    currentSecretGrantRef,
    currentSecretGrantEpochRef,
    beginConsoleActionRequest,
    isConsoleActionRequestCurrent,
    isConsoleActionRecoveryCurrent,
  };
}
