import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleGeminiAuthFamily, ConsoleGeminiAuthSession } from "../../api/contracts";
import { canManuallyCompleteGeminiAuthSession, isGeminiAuthSessionTerminal } from "./geminiCredentialDraft";

type GeminiManualAddDialogState = {
  targetFamily: ConsoleGeminiAuthFamily;
  providerId: string;
  session: ConsoleGeminiAuthSession | null;
  busy: boolean;
  importMethod?: "browser" | "json";
};

type GeminiManualAddSessionOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  applyGeminiManualAddSessionResult: (session: ConsoleGeminiAuthSession) => void;
  setError: (error: string | null) => void;
  t: (zh: string, en: string) => string;
  jsonImportContext?: { revision?: string; secretGrant: string | null };
};

export function useGeminiManualAddSession({
  api, managementToken, applyGeminiManualAddSessionResult, setError, t, jsonImportContext,
}: GeminiManualAddSessionOptions) {
  const [geminiManualAddDialogState, setGeminiManualAddDialogState] =
    useState<GeminiManualAddDialogState | null>(null);
  const [pollRevision, setPollRevision] = useState(0);
  const geminiManualAddPollTimeoutRef = useRef<number | null>(null);
  const lifecycleGeneration = useRef(0);
  const responseSequence = useRef(0);
  const completionPending = useRef(false);
  const jsonContext = useRef(jsonImportContext);
  const jsonContextEpoch = useRef(0);
  useLayoutEffect(() => {
    jsonContext.current = jsonImportContext;
    jsonContextEpoch.current += 1;
  }, [jsonImportContext?.revision, jsonImportContext?.secretGrant]);

  // Closing, replacing the host/session, or unmounting invalidates pending results.
  useLayoutEffect(() => {
    setGeminiManualAddDialogState(null);
    completionPending.current = false;
    return () => {
      lifecycleGeneration.current += 1;
      if (geminiManualAddPollTimeoutRef.current !== null) {
        window.clearTimeout(geminiManualAddPollTimeoutRef.current);
        geminiManualAddPollTimeoutRef.current = null;
      }
    };
  }, [api, managementToken]);

  const refreshGeminiManualAddSession = useCallback(
    async (sessionId: string) => {
      if (!managementToken || completionPending.current) {
        return;
      }
      const generation = lifecycleGeneration.current;
      const sequence = ++responseSequence.current;
      let response: Awaited<ReturnType<ConsoleApi["getGeminiAuthSession"]>>;
      try {
        response = await api.getGeminiAuthSession(managementToken, sessionId);
      } catch (cause) {
        if (generation === lifecycleGeneration.current && sequence === responseSequence.current) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
        return;
      } finally {
        // A waiting response may keep the same updatedAt; every settled read must rearm polling.
        if (generation === lifecycleGeneration.current && sequence === responseSequence.current) {
          setPollRevision((revision) => revision + 1);
        }
      }
      if (generation !== lifecycleGeneration.current || sequence !== responseSequence.current) return;
      setGeminiManualAddDialogState((current) => {
        if (!current || current.session?.id !== sessionId) {
          return current;
        }
        return {
          ...current,
          busy: false,
          session: response.session,
        };
      });
      applyGeminiManualAddSessionResult(response.session);
    },
    [api, applyGeminiManualAddSessionResult, managementToken, setError],
  );

  const requestGeminiManualAddCompletion = useCallback(async () => {
    const session = geminiManualAddDialogState?.session ?? null;
    if (!managementToken || !session || completionPending.current || !canManuallyCompleteGeminiAuthSession(session)) {
      return;
    }
    const sessionId = session.id;
    const generation = lifecycleGeneration.current;
    // A completion supersedes earlier reads; polls wait until it settles.
    const sequence = ++responseSequence.current;
    completionPending.current = true;
    setError(null);
    setGeminiManualAddDialogState((current) =>
      current
        ? {
            ...current,
            busy: true,
          }
        : current,
    );

    try {
      const response = await api.completeGeminiAuthSession(managementToken, sessionId);
      if (generation !== lifecycleGeneration.current || sequence !== responseSequence.current) return;
      completionPending.current = false;
      setGeminiManualAddDialogState((current) => {
        if (!current || current.session?.id !== sessionId) {
          return current;
        }
        return {
          ...current,
          busy: false,
          session: response.session,
        };
      });
      if (isGeminiAuthSessionTerminal(response.session.status)) {
        applyGeminiManualAddSessionResult(response.session);
      } else {
        void refreshGeminiManualAddSession(response.session.id);
      }
    } catch (cause) {
      if (generation !== lifecycleGeneration.current || sequence !== responseSequence.current) return;
      completionPending.current = false;
      setGeminiManualAddDialogState((current) =>
        current
          ? {
              ...current,
              busy: false,
            }
          : current,
      );
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [
    api,
    applyGeminiManualAddSessionResult,
    geminiManualAddDialogState?.session,
    managementToken,
    refreshGeminiManualAddSession,
    setError,
    t,
  ]);

  const closeGeminiManualAddDialog = useCallback(() => {
    lifecycleGeneration.current += 1;
    completionPending.current = false;
    if (geminiManualAddPollTimeoutRef.current !== null) {
      window.clearTimeout(geminiManualAddPollTimeoutRef.current);
      geminiManualAddPollTimeoutRef.current = null;
    }
    setGeminiManualAddDialogState(null);
  }, []);

  const openGeminiManualAddDialog = useCallback(
    async (targetFamily: ConsoleGeminiAuthFamily, providerId: string, accountLabel?: string, webSessionJson?: string) => {
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
      }
      const context = jsonContext.current;
      const contextEpoch = jsonContextEpoch.current;
      if (webSessionJson !== undefined && (!context?.revision || !context.secretGrant)) {
        setError(t("请确认敏感信息访问权限并刷新配置后重试。", "Confirm secret access and refresh configuration first.")); return;
      }

      const generation = ++lifecycleGeneration.current;
      completionPending.current = false;
      if (geminiManualAddPollTimeoutRef.current !== null) {
        window.clearTimeout(geminiManualAddPollTimeoutRef.current);
        geminiManualAddPollTimeoutRef.current = null;
      }
      setError(null);
      setGeminiManualAddDialogState({
        targetFamily,
        providerId,
        session: null,
        busy: true,
        importMethod: webSessionJson === undefined ? "browser" : "json",
      });

      try {
        const request = {
          targetFamily,
          providerId,
          ...(accountLabel ? { accountLabel } : {}),
        };
        const response = webSessionJson === undefined ? await api.createGeminiAuthSession(managementToken, request)
          : await api.createGeminiAuthSession(managementToken, { ...request, webSessionJson, expectedRevision: context!.revision }, context!.secretGrant!);
        if (generation !== lifecycleGeneration.current) return;
        if (webSessionJson !== undefined && (contextEpoch !== jsonContextEpoch.current
          || response.session.providerId !== providerId || response.session.targetFamily !== targetFamily)) {
          setGeminiManualAddDialogState(null); setError(t("导入目标、配置或授权已改变，请重新导入。", "Import target, revision or access changed; import again.")); return;
        }
        // Offline imports are one-shot drafts, never managed browser sessions to poll.
        if (webSessionJson !== undefined && (response.session.status !== "succeeded" || response.session.generatedDrafts.length === 0
          || response.session.generatedDrafts.some(draft => draft.providerId !== providerId))) {
          setGeminiManualAddDialogState(null); setError(t("网页 JSON 未返回有效的目标账号草稿，请重新导入。", "Web JSON did not return a valid target account draft; import again.")); return;
        }
        setGeminiManualAddDialogState({
          targetFamily,
          providerId,
          session: response.session,
          busy: false,
          importMethod: webSessionJson === undefined ? "browser" : "json",
        });
        if (isGeminiAuthSessionTerminal(response.session.status)) {
          applyGeminiManualAddSessionResult(response.session);
        } else {
          void refreshGeminiManualAddSession(response.session.id);
        }
      } catch (cause) {
        if (generation !== lifecycleGeneration.current) return;
        setGeminiManualAddDialogState(null);
        if (webSessionJson !== undefined && contextEpoch !== jsonContextEpoch.current) return;
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [api, applyGeminiManualAddSessionResult, managementToken, refreshGeminiManualAddSession, t],
  );

  useEffect(() => {
    if (geminiManualAddPollTimeoutRef.current !== null) {
      window.clearTimeout(geminiManualAddPollTimeoutRef.current);
      geminiManualAddPollTimeoutRef.current = null;
    }
    const sessionId = geminiManualAddDialogState?.session?.id;
    const sessionStatus = geminiManualAddDialogState?.session?.status;
    if (!sessionId || !sessionStatus || isGeminiAuthSessionTerminal(sessionStatus)) {
      return;
    }
    geminiManualAddPollTimeoutRef.current = window.setTimeout(() => {
      geminiManualAddPollTimeoutRef.current = null;
      void refreshGeminiManualAddSession(sessionId);
    }, 1500);
    return () => {
      if (geminiManualAddPollTimeoutRef.current !== null) {
        window.clearTimeout(geminiManualAddPollTimeoutRef.current);
        geminiManualAddPollTimeoutRef.current = null;
      }
    };
  }, [
    geminiManualAddDialogState?.session?.id,
    geminiManualAddDialogState?.session?.status,
    geminiManualAddDialogState?.session?.updatedAt,
    pollRevision,
    refreshGeminiManualAddSession,
  ]);

  return {
    geminiManualAddDialogState,
    requestGeminiManualAddCompletion,
    closeGeminiManualAddDialog,
    openGeminiManualAddDialog,
  };
}
