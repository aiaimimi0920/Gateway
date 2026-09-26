import { useCallback, useEffect, useRef } from "react";
import { pushAppToast } from "../../components/AppToast";
import type { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import type { useConsoleRouteData } from "./useConsoleRouteData";
import type { useConsoleActionIdentity } from "./useConsoleActionIdentity";

type RouteDraft = ReturnType<typeof useConsoleRouteDraft>;
type RouteData = ReturnType<typeof useConsoleRouteData>;
type ActionIdentity = ReturnType<typeof useConsoleActionIdentity>;
type ConsoleDraftPersistenceOptions = {
  managementToken: string | null;
  editorLocked: boolean;
  selectedGroupIdInvalid: boolean;
  selectedGroupBillingInvalid: boolean;
  editorText: RouteDraft["editorText"];
  commitMessage: RouteDraft["commitMessage"];
  secretPatches: RouteDraft["secretPatches"];
  draftDirty: RouteDraft["draftDirty"];
  draftDocumentState: RouteDraft["draftDocumentState"];
  incompleteAccountGroupDraftCount: RouteDraft["incompleteAccountGroupDraftCount"];
  parseDraft: RouteDraft["parseDraft"];
  setRouteConfig: RouteData["setRouteConfig"];
  refresh: RouteData["refresh"];
  setError: RouteData["setError"];
  consoleActionGenerationRef: ActionIdentity["consoleActionGenerationRef"];
  beginConsoleActionRequest: ActionIdentity["beginConsoleActionRequest"];
  isConsoleActionRequestCurrent: ActionIdentity["isConsoleActionRequestCurrent"];
  isConsoleActionRecoveryCurrent: ActionIdentity["isConsoleActionRecoveryCurrent"];
  setActionBusy: ActionIdentity["setActionBusy"];
  handleSecretAccessRequiredError: (cause: unknown, isCurrentRequest: () => boolean) => "not-required" | "recovered" | "stale";
  t: (zh: string, en: string) => string;
};

export function useConsoleDraftPersistence({
  managementToken,
  editorLocked,
  selectedGroupIdInvalid,
  selectedGroupBillingInvalid,
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
}: ConsoleDraftPersistenceOptions) {
  /** The draft state autosave has already committed, so it commits it once. */
  const autosaveSignatureRef = useRef<string | null>(null);

  const handleSave = useCallback(
    async (options?: { silent?: boolean }) => {
      if (!managementToken) {
        setError(t("当前没有可用的 Gateway 管理密钥。", "Gateway management token is unavailable."));
        return;
      }
      const actionRequest = beginConsoleActionRequest(managementToken);
      setActionBusy("save");
      setError(null);
      try {
        const draft = parseDraft();
        const result = actionRequest.secretGrant
          ? await actionRequest.api.commitRouteConfig(
              actionRequest.managementToken,
              draft,
              actionRequest.secretGrant,
            )
          : await actionRequest.api.commitRouteConfig(actionRequest.managementToken, draft);
        if (!isConsoleActionRequestCurrent(actionRequest)) {
          return;
        }
        setRouteConfig({ routeConfig: result.routeConfig });
        // Autosave commits on every settled edit, so its successes stay quiet.
        if (!options?.silent) {
          pushAppToast(
            "success",
            t(
              `已将路由配置保存为激活修订 ${result.routeConfig.revision.id}。`,
              `Saved route config as active revision ${result.routeConfig.revision.id}.`,
            ),
          );
        }
        await refresh();
      } catch (cause) {
        const secretRecovery = handleSecretAccessRequiredError(
          cause,
          () => isConsoleActionRecoveryCurrent(actionRequest),
        );
        if (secretRecovery !== "not-required") {
          return;
        }
        if (!isConsoleActionRequestCurrent(actionRequest)) {
          return;
        }
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        if (actionRequest.generation === consoleActionGenerationRef.current) {
          setActionBusy(null);
        }
      }
    },
    [
      beginConsoleActionRequest,
      consoleActionGenerationRef,
      handleSecretAccessRequiredError,
      isConsoleActionRecoveryCurrent,
      isConsoleActionRequestCurrent,
      managementToken,
      parseDraft,
      refresh,
      setActionBusy,
      setError,
      setRouteConfig,
      t,
    ],
  );
  // Every card edit stages into the same draft, and there is no commit button
  // any more, so a settled draft commits itself. The signature ref makes each
  // draft state commit at most once: a rejected commit surfaces through
  // `error` and waits for the next edit instead of retrying in a loop.
  const autosaveSignature = `${editorText}\u0000${commitMessage}\u0000${JSON.stringify(secretPatches)}`;
  const autosaveReady =
    !editorLocked &&
    draftDirty &&
    !draftDocumentState.invalid &&
    incompleteAccountGroupDraftCount === 0 &&
    !selectedGroupIdInvalid &&
    !selectedGroupBillingInvalid;
  const autosavePending = autosaveReady && autosaveSignatureRef.current !== autosaveSignature;
  useEffect(() => {
    if (!autosaveReady || autosaveSignatureRef.current === autosaveSignature) {
      return;
    }
    // Inline id/name fields fire per keystroke, so the draft has to settle first.
    const timer = window.setTimeout(() => {
      autosaveSignatureRef.current = autosaveSignature;
      void handleSave({ silent: true });
    }, 1200);
    return () => window.clearTimeout(timer);
  }, [autosaveReady, autosaveSignature, handleSave]);

  return { autosavePending };
}
