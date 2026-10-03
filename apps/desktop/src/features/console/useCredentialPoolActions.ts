import { useCallback, useLayoutEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleCredentialPoolAutomationProvider, ConsoleCredentialPoolAutomationResponse, ConsoleCredentialRefillDemand } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";

type CredentialPoolActionsOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  draftDirty: boolean;
  credentialPoolAutomationByProvider: ReadonlyMap<string, Pick<ConsoleCredentialPoolAutomationProvider, "driverConfigured">>;
  credentialRefillByProvider: ReadonlyMap<string, Pick<ConsoleCredentialRefillDemand, "userRequestEnabled">>;
  setCredentialPoolAutomation: Dispatch<SetStateAction<ConsoleCredentialPoolAutomationResponse | null>>;
  refresh: () => Promise<void>;
  setError: Dispatch<SetStateAction<string | null>>;
  t: (zh: string, en: string) => string;
};

export function useCredentialPoolActions({
  api,
  managementToken,
  draftDirty,
  credentialPoolAutomationByProvider,
  credentialRefillByProvider,
  setCredentialPoolAutomation,
  refresh,
  setError,
  t,
}: CredentialPoolActionsOptions) {
  const draftDirtyRef = useRef(draftDirty);
  draftDirtyRef.current = draftDirty;
  const [credentialPoolPruneBusy, setCredentialPoolPruneBusy] = useState<string | null>(null);

  const [credentialArchivePurgeBusy, setCredentialArchivePurgeBusy] =
    useState<string | null>(null);

  const [credentialRefillBusy, setCredentialRefillBusy] = useState<string | null>(null);

  const scope = useRef({ active: false, epoch: 0, api, managementToken, prune: 0, purge: 0, refill: 0 });
  useLayoutEffect(() => {
    Object.assign(scope.current, { active: true, api, managementToken });
    setCredentialPoolPruneBusy(null);
    setCredentialArchivePurgeBusy(null);
    setCredentialRefillBusy(null);
    return () => {
      scope.current.active = false;
      scope.current.epoch += 1;
    };
  }, [api, managementToken]);

  const isActiveIdentity = useCallback(() => scope.current.active &&
    scope.current.api === api && scope.current.managementToken === managementToken,
  [api, managementToken]);
  const beginRequest = useCallback((kind: "prune" | "purge" | "refill") => {
    const epoch = scope.current.epoch;
    const sequence = ++scope.current[kind];
    // Each operation has its own busy marker; older completions cannot clear it.
    return () => isActiveIdentity() && scope.current.epoch === epoch &&
      scope.current[kind] === sequence;
  }, [isActiveIdentity]);

  const handlePruneCredentialPool = useCallback(
    async (providerId: string) => {
      if (!isActiveIdentity()) return;
      if (!managementToken) {
        const message = t(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        );
        setError(message);
        pushAppToast("error", message);
        return;
      }
      if (draftDirtyRef.current) {
        pushAppToast(
          "warning",
          t(
            "请先保存当前路由草稿，再删除失效号。",
            "Save the current route draft before removing invalid credentials.",
          ),
        );
        return;
      }
      const automation = credentialPoolAutomationByProvider.get(providerId);
      if (!automation?.driverConfigured) {
        pushAppToast(
          "warning",
          t(
            `渠道 ${providerId} 尚未配置受信任的失效号识别驱动器。`,
            `Provider ${providerId} does not have a trusted invalid-credential driver.`,
          ),
        );
        return;
      }
      setError(null);
      const isCurrent = beginRequest("prune");
      setCredentialPoolPruneBusy(providerId);
      try {
        const result = await api.pruneCredentialPool(managementToken, providerId);
        if (!isCurrent()) return;
        setCredentialPoolAutomation((current) =>
          current
            ? {
                automation: {
                  ...current.automation,
                  providers: current.automation.providers.map((provider) =>
                    provider.providerId === providerId ? result.provider : provider,
                  ),
                },
              }
            : current,
        );
        pushAppToast(
          "success",
          result.provider.message ??
            t(
              `渠道 ${providerId} 的失效号检查已完成。`,
              `Invalid credential cleanup completed for ${providerId}.`,
            ),
        );
        await refresh();
      } catch (cause) {
        if (!isCurrent()) return;
        const message = cause instanceof Error ? cause.message : String(cause);
        setError(message);
        pushAppToast("error", message);
      } finally {
        if (isCurrent()) setCredentialPoolPruneBusy(null);
      }
    },
    [api, beginRequest, credentialPoolAutomationByProvider, draftDirty, isActiveIdentity, managementToken, refresh, t],
  );

  const handlePurgeCredentialArchive = useCallback(
    async (providerId: string) => {
      if (!isActiveIdentity()) return;
      if (!managementToken) {
        const message = t(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        );
        setError(message);
        pushAppToast("error", message);
        return;
      }
      setError(null);
      if (draftDirtyRef.current) {
        pushAppToast("warning", t(
          "请先保存当前路由草稿，再清空账号归档。",
          "Save the current route draft before purging the credential archive.",
        ));
        return;
      }
      const isCurrent = beginRequest("purge");
      setCredentialArchivePurgeBusy(providerId);
      try {
        const result = await api.purgeCredentialArchive(managementToken, providerId);
        if (!isCurrent()) return;
        pushAppToast(
          "success",
          t(
            `已彻底删除 ${result.purgedCount} 个归档账号。`,
            `Permanently deleted ${result.purgedCount} archived credentials.`,
          ),
        );
        await refresh();
      } catch (cause) {
        if (!isCurrent()) return;
        const message = cause instanceof Error ? cause.message : String(cause);
        // A failed/uncertain deletion can follow a committed durable purge barrier.
        try { await refresh(); } catch { /* Preserve the original purge error. */ }
        if (!isCurrent()) return;
        setError(message);
        pushAppToast("error", message);
      } finally {
        if (isCurrent()) setCredentialArchivePurgeBusy(null);
      }
    },
    [api, beginRequest, isActiveIdentity, managementToken, refresh, t],
  );

  const handleRequestCredentialRefill = useCallback(
    async (providerId: string) => {
      if (!isActiveIdentity()) return;
      if (!managementToken) {
        const message = t(
          "当前没有可用的 Gateway 管理密钥。",
          "Gateway management token is unavailable.",
        );
        setError(message);
        pushAppToast("error", message);
        return;
      }
      if (draftDirtyRef.current) {
        pushAppToast(
          "warning",
          t(
            "请先保存当前路由草稿，再发起主动补号。",
            "Save the current route draft before requesting a refill.",
          ),
        );
        return;
      }
      const demand = credentialRefillByProvider.get(providerId);
      if (!demand?.userRequestEnabled) {
        pushAppToast(
          "warning",
          t("当前补号任务框架不可用。", "The credential refill framework is unavailable."),
        );
        return;
      }
      setError(null);
      const isCurrent = beginRequest("refill");
      setCredentialRefillBusy(providerId);
      try {
        const result = await api.requestCredentialRefill(managementToken, providerId);
        if (!isCurrent()) return;
        pushAppToast(
          result.created ? "success" : "info",
          result.created
            ? t(
                `渠道 ${providerId} 的主动补号任务已投递。`,
                `A user-requested refill task was published for ${providerId}.`,
              )
            : t(
                `渠道 ${providerId} 已有未完成的补号任务。`,
                `Provider ${providerId} already has an outstanding refill task.`,
              ),
        );
        await refresh();
      } catch (cause) {
        if (!isCurrent()) return;
        const message = cause instanceof Error ? cause.message : String(cause);
        setError(message);
        pushAppToast("error", message);
      } finally {
        if (isCurrent()) setCredentialRefillBusy(null);
      }
    },
    [api, beginRequest, credentialRefillByProvider, draftDirty, isActiveIdentity, managementToken, refresh, t],
  );

  return { credentialPoolPruneBusy, credentialArchivePurgeBusy, credentialRefillBusy, handlePruneCredentialPool, handlePurgeCredentialArchive, handleRequestCredentialRefill };
}
