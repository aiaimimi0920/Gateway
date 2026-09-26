import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleGeminiAuthFamily, ConsoleGeminiAuthSession } from "../../api/contracts";
import { canManuallyCompleteGeminiAuthSession, isGeminiAuthSessionTerminal } from "./geminiCredentialDraft";

type GeminiManualAddDialogState = {
  targetFamily: ConsoleGeminiAuthFamily;
  providerId: string;
  session: ConsoleGeminiAuthSession | null;
  busy: boolean;
};

type GeminiManualAddSessionOptions = {
  api: ConsoleApi;
  managementToken: string | null;
  applyGeminiManualAddSessionResult: (session: ConsoleGeminiAuthSession) => void;
  setError: (error: string | null) => void;
  t: (zh: string, en: string) => string;
};

export function useGeminiManualAddSession({
  api, managementToken, applyGeminiManualAddSessionResult, setError, t,
}: GeminiManualAddSessionOptions) {
  const [geminiManualAddDialogState, setGeminiManualAddDialogState] =
    useState<GeminiManualAddDialogState | null>(null);
  const geminiManualAddPollTimeoutRef = useRef<number | null>(null);
  const lifecycleGeneration = useRef(0);
  const responseSequence = useRef(0);
  const completionPending = useRef(false);

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
    async (targetFamily: ConsoleGeminiAuthFamily, providerId: string) => {
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
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
      });

      try {
        const response = await api.createGeminiAuthSession(managementToken, {
          targetFamily,
          providerId,
        });
        if (generation !== lifecycleGeneration.current) return;
        setGeminiManualAddDialogState({
          targetFamily,
          providerId,
          session: response.session,
          busy: false,
        });
        if (isGeminiAuthSessionTerminal(response.session.status)) {
          applyGeminiManualAddSessionResult(response.session);
        } else {
          void refreshGeminiManualAddSession(response.session.id);
        }
      } catch (cause) {
        if (generation !== lifecycleGeneration.current) return;
        setGeminiManualAddDialogState(null);
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
    refreshGeminiManualAddSession,
  ]);

  return {
    geminiManualAddDialogState,
    requestGeminiManualAddCompletion,
    closeGeminiManualAddDialog,
    openGeminiManualAddDialog,
  };
}
