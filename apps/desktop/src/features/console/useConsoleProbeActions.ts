import { useCallback, useLayoutEffect, useRef, type Dispatch, type SetStateAction, type MutableRefObject } from "react";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleProviderProbeResponse, ConsoleCredentialProbeResult, ConsoleCredentialProbeStatus } from "../../api/contracts";
import type { useManagementSession } from "../../session/useManagementSession";
import type { AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import type { RouteManagedAccount } from "./routeAccountCatalog";
export type CredentialProbeViewResult = ConsoleCredentialProbeResult | {
  credentialId: string;
  providerId: string;
  probePoint: string;
  status: ConsoleCredentialProbeStatus | "error";
  message: string;
  checkedAt: string;
};

type ConsoleProbeActionsOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  session: Pick<ReturnType<typeof useManagementSession>, "secretGrant">;
  draftDirty: boolean;
  draftMatchesActiveRevision: boolean;
  credentialProbeGenerationRef: MutableRefObject<number>;
  providerProbeGenerationRef: MutableRefObject<number>;
  providerProbeAbortRef?: MutableRefObject<AbortController | null>;
  currentApiRef: MutableRefObject<ConsoleApi>;
  currentManagementTokenRef: MutableRefObject<string | null>;
  currentSecretGrantEpochRef: MutableRefObject<number>;
  currentSecretGrantRef: MutableRefObject<string | null>;
  handleSecretAccessRequiredError: (cause: unknown, isCurrentRequest: () => boolean) => "not-required" | "recovered" | "stale";
  setCredentialProbeBusy: Dispatch<SetStateAction<string | null>>;
  setCredentialProbeResults: Dispatch<SetStateAction<Record<string, CredentialProbeViewResult>>>;
  setProviderProbeBusy: Dispatch<SetStateAction<boolean>>;
  setProviderProbeError: Dispatch<SetStateAction<string | null>>;
  setProviderProbeResponse: Dispatch<SetStateAction<ConsoleProviderProbeResponse | null>>;
  setSecretDialogOpen: Dispatch<SetStateAction<boolean>>;
  setError: Dispatch<SetStateAction<string | null>>;
  t: (zh: string, en: string) => string;
};

export function useConsoleProbeActions({
  api,
  managementToken,
  session,
  draftDirty,
  draftMatchesActiveRevision,
  credentialProbeGenerationRef,
  providerProbeGenerationRef,
  providerProbeAbortRef,
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
}: ConsoleProbeActionsOptions) {
  const mounted = useRef(false);
  const readOnlyProbe = useRef(false);
  useLayoutEffect(() => {
    mounted.current = true;
    setCredentialProbeBusy(null);
    setCredentialProbeResults(current => Object.keys(current).length === 0 ? current : {});
    setProviderProbeBusy(false);
    setProviderProbeError(null);
    setProviderProbeResponse(null);
    return () => {
      mounted.current = false;
      // Invalidate requests before a late rejection can reopen secret access.
      credentialProbeGenerationRef.current += 1;
      providerProbeGenerationRef.current += 1;
      providerProbeAbortRef?.current?.abort();
    };
  }, [api, managementToken, credentialProbeGenerationRef, providerProbeGenerationRef]);

  const handleCredentialProbe = useCallback(
    async (account: RouteManagedAccount) => {
      if (!mounted.current || api !== currentApiRef.current ||
          managementToken !== currentManagementTokenRef.current) return;
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
      }
      if (!session.secretGrant?.grant) {
        setError(
          t(
            "测试账号前需要先授权敏感信息访问权限。",
            "Confirm secret access before probing an account.",
          ),
        );
        setSecretDialogOpen(true);
        return;
      }
      if (
        !account.enabled ||
        draftDirty ||
        !draftMatchesActiveRevision
      ) {
        return;
      }

      const probeGeneration = credentialProbeGenerationRef.current + 1;
      credentialProbeGenerationRef.current = probeGeneration;
      const probeApi = api;
      const probeManagementToken = managementToken;
      const probeSecretGrant = session.secretGrant.grant;
      const probeSecretGrantEpoch = currentSecretGrantEpochRef.current;
      const isCurrentProbeIdentity = () =>
        probeGeneration === credentialProbeGenerationRef.current &&
        probeApi === currentApiRef.current &&
        probeManagementToken === currentManagementTokenRef.current;
      const isCurrentProbe = () =>
        isCurrentProbeIdentity() &&
        probeSecretGrantEpoch === currentSecretGrantEpochRef.current &&
        probeSecretGrant === currentSecretGrantRef.current;
      const isCurrentProbeRecovery = () => {
        if (!isCurrentProbeIdentity()) {
          return false;
        }
        const currentGrant = currentSecretGrantRef.current;
        const currentEpoch = currentSecretGrantEpochRef.current;
        return (
          (currentEpoch === probeSecretGrantEpoch && currentGrant === probeSecretGrant) ||
          (currentGrant === null && currentEpoch === probeSecretGrantEpoch + 1)
        );
      };
      setCredentialProbeBusy(account.id);
      setError(null);
      try {
        const response = await probeApi.probeCredential(
          probeManagementToken,
          probeSecretGrant,
          account.id,
        );
        if (!isCurrentProbe()) {
          return;
        }
        setCredentialProbeResults((current) => ({
          ...current,
          [account.id]: response.result,
        }));
      } catch (cause) {
        if (!mounted.current || !isCurrentProbeIdentity()) return;
        const secretRecovery = handleSecretAccessRequiredError(cause, isCurrentProbeRecovery);
        if (secretRecovery === "stale") {
          return;
        }
        if (secretRecovery === "recovered") {
          return;
        }
        if (!isCurrentProbe()) {
          return;
        }
        setCredentialProbeResults((current) => ({
          ...current,
          [account.id]: {
            credentialId: account.id,
            providerId: account.providerId,
            probePoint: t("未执行", "Not executed"),
            status: "error",
            message: cause instanceof Error ? cause.message : String(cause),
            checkedAt: new Date().toISOString(),
          },
        }));
      } finally {
        if (probeGeneration === credentialProbeGenerationRef.current) {
          setCredentialProbeBusy(null);
        }
      }
    },
    [
      api,
      draftDirty,
      draftMatchesActiveRevision,
      handleSecretAccessRequiredError,
      managementToken,
      session.secretGrant,
      t,
    ],
  );

  const handleProviderProbe = useCallback(
      async (section: Pick<AccountsLedgerPilotSection, "providerId">, options?: import("../../api/contracts").ConsoleProviderProbeRequest & { includePlans?: boolean }, readOnly = false) => {
      if (readOnly && !api.readProviderProbeResults) return;
      if (providerProbeAbortRef?.current && !providerProbeAbortRef.current.signal.aborted) {
        if (!readOnlyProbe.current) return;
        providerProbeAbortRef.current.abort();
      }
      if (!mounted.current || api !== currentApiRef.current ||
          managementToken !== currentManagementTokenRef.current) return;
      if (!managementToken) {
        setProviderProbeError(
          t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."),
        );
        return;
      }
      if (!session.secretGrant?.grant) {
        setProviderProbeError(
          t(
            "测试服务商前需要先授权敏感信息访问权限。",
            "Confirm secret access before probing a provider.",
          ),
        );
        setSecretDialogOpen(true);
        return;
      }
      if (!readOnly && (draftDirty || !draftMatchesActiveRevision)) {
        setProviderProbeError(
          t(
            "请先保存当前路由草稿，再测试已生效的服务商账号。",
            "Save the current route draft before testing the active provider accounts.",
          ),
        );
        return;
      }

      const probeGeneration = providerProbeGenerationRef.current + 1;
      providerProbeGenerationRef.current = probeGeneration;
      const probeApi = api;
      const probeManagementToken = managementToken;
      const probeSecretGrant = session.secretGrant.grant;
      const probeSecretGrantEpoch = currentSecretGrantEpochRef.current;
      const isCurrentProbeIdentity = () =>
        probeGeneration === providerProbeGenerationRef.current &&
        probeApi === currentApiRef.current &&
        probeManagementToken === currentManagementTokenRef.current;
      const isCurrentProbe = () =>
        isCurrentProbeIdentity() &&
        probeSecretGrantEpoch === currentSecretGrantEpochRef.current &&
        probeSecretGrant === currentSecretGrantRef.current;
      const isCurrentProbeRecovery = () => {
        if (!isCurrentProbeIdentity()) {
          return false;
        }
        const currentGrant = currentSecretGrantRef.current;
        const currentEpoch = currentSecretGrantEpochRef.current;
        return (
          (currentEpoch === probeSecretGrantEpoch && currentGrant === probeSecretGrant) ||
          (currentGrant === null && currentEpoch === probeSecretGrantEpoch + 1)
        );
      };

      setProviderProbeBusy(!readOnly);
      setProviderProbeError(null);
      setProviderProbeResponse(null);
      const abort = new AbortController();
      readOnlyProbe.current = readOnly;
      providerProbeAbortRef?.current?.abort();
      if (providerProbeAbortRef) providerProbeAbortRef.current = abort;
      try {
        const response = readOnly ? await probeApi.readProviderProbeResults!(probeManagementToken, probeSecretGrant, section.providerId, { signal: abort.signal }, options?.scope, { planId: options?.planId, includePlans: options?.includePlans }) : await probeApi.probeProvider(
          probeManagementToken,
          probeSecretGrant,
          section.providerId,
          options,
          { signal: abort.signal },
        );
        if (!isCurrentProbe()) {
          return;
        }
        setProviderProbeResponse(response);
        return response;
      } catch (cause) {
        if (!mounted.current || !isCurrentProbeIdentity()) return;
        const secretRecovery = handleSecretAccessRequiredError(cause, isCurrentProbeRecovery);
        if (secretRecovery !== "not-required" || !isCurrentProbe()) {
          return;
        }
        setProviderProbeError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        if (providerProbeAbortRef?.current === abort) providerProbeAbortRef.current = null;
        if (probeGeneration === providerProbeGenerationRef.current) {
          setProviderProbeBusy(false);
        }
      }
    },
    [
      api,
      draftDirty,
      draftMatchesActiveRevision,
      handleSecretAccessRequiredError,
      managementToken,
      session.secretGrant,
      t,
    ],
  );

  return { handleCredentialProbe, handleProviderProbe };
}
