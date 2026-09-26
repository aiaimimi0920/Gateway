import { useCallback, useRef } from "react";
import type { ConsoleRouteDocument, ConsoleGeminiAuthFamily, ConsoleGeminiAuthSession } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { applyGeminiGeneratedDraftsToDocument } from "./geminiCredentialDraft";
import { mergeCredentialSecretEditEntries } from "./secretPatchDraft";
import { parseRouteDocument } from "./routeDocument";
import type { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import type { useConsoleRouteData } from "./useConsoleRouteData";
import type { useConsoleActionIdentity } from "./useConsoleActionIdentity";

type RouteDraft = ReturnType<typeof useConsoleRouteDraft>;
type RouteData = ReturnType<typeof useConsoleRouteData>;
type ActionIdentity = ReturnType<typeof useConsoleActionIdentity>;
type GeminiCredentialImportOptions = {
  managementToken: string | null;
  editorText: RouteDraft["editorText"];
  credentialSecretEdits: RouteDraft["credentialSecretEdits"];
  setCredentialSecretEdits: RouteDraft["setCredentialSecretEdits"];
  replaceEditorDocument: RouteDraft["replaceEditorDocument"];
  buildCommitRequest: RouteDraft["buildCommitRequest"];
  activeSecretPatches: RouteDraft["activeSecretPatches"];
  routeConfig: RouteData["routeConfig"];
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

export function useGeminiCredentialImport({
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
}: GeminiCredentialImportOptions) {
  const appliedGeminiManualAddSessionIdsRef = useRef<Set<string>>(new Set());

  const persistGeminiGeneratedDrafts = useCallback(
    async (
      document: ConsoleRouteDocument,
      secretEdits: CredentialSecretEdit[],
      sessionId: string,
      targetFamily: ConsoleGeminiAuthFamily,
      generatedDraftCount: number,
    ) => {
      if (
        targetFamily !== "gemini-canvas" &&
        targetFamily !== "gemini-canvas-chat" &&
        targetFamily !== "gemini-web"
      ) {
        return false;
      }
      if (!managementToken || !routeConfig) {
        return false;
      }

      const commitSecretPatches = buildCredentialSecretPatches({
        activeDocument: routeConfig.routeConfig.document,
        draftDocument: document,
        activeSecrets: routeConfig.routeConfig.secrets,
        activeSecretPatches,
        credentialSecretEdits: secretEdits,
      });
      const requiresSecretGrant = commitSecretPatches.some(
        (patch) => patch.operation !== "keep",
      );
      if (requiresSecretGrant) {
        return false;
      }

      const actionRequest = beginConsoleActionRequest(managementToken);
      setActionBusy("save");
      try {
        const draft = buildCommitRequest(
          document,
          `Import Gemini manual credentials (${sessionId})`,
          commitSecretPatches,
        );
        const result = await actionRequest.api.commitRouteConfig(
          actionRequest.managementToken,
          draft,
        );
        if (!isConsoleActionRequestCurrent(actionRequest)) {
          return true;
        }
        setRouteConfig({ routeConfig: result.routeConfig });
        setError(null);
        pushAppToast(
          "success",
          t(
            `Gemini 凭证已导入并保存为激活修订 ${result.routeConfig.revision.id}（${generatedDraftCount} 条）。`,
            `Gemini credentials were imported and saved as active revision ${result.routeConfig.revision.id} (${generatedDraftCount} entries).`,
          ),
        );
        await refresh();
        return true;
      } catch (cause) {
        const secretRecovery = handleSecretAccessRequiredError(
          cause,
          () => isConsoleActionRecoveryCurrent(actionRequest),
        );
        if (secretRecovery !== "not-required") {
          return false;
        }
        if (!isConsoleActionRequestCurrent(actionRequest)) {
          return true;
        }
        setError(cause instanceof Error ? cause.message : String(cause));
        return false;
      } finally {
        if (actionRequest.generation === consoleActionGenerationRef.current) {
          setActionBusy(null);
        }
      }
    },
    [
      activeSecretPatches,
      beginConsoleActionRequest,
      buildCommitRequest,
      handleSecretAccessRequiredError,
      isConsoleActionRecoveryCurrent,
      isConsoleActionRequestCurrent,
      managementToken,
      refresh,
      routeConfig,
      t,
    ],
  );

  const applyGeminiManualAddSessionResult = useCallback(
    (nextSession: ConsoleGeminiAuthSession) => {
      if (nextSession.status !== "succeeded") {
        return;
      }
      if (appliedGeminiManualAddSessionIdsRef.current.has(nextSession.id)) {
        return;
      }
      if (nextSession.generatedDrafts.length === 0) {
        appliedGeminiManualAddSessionIdsRef.current.add(nextSession.id);
        return;
      }

      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }

      try {
        const generated = applyGeminiGeneratedDraftsToDocument(document, nextSession.generatedDrafts);
        const mergedSecretEdits = mergeCredentialSecretEditEntries(
          credentialSecretEdits,
          generated.secretEdits,
        );
        setCredentialSecretEdits((current) =>
          mergeCredentialSecretEditEntries(current, generated.secretEdits),
        );
        replaceEditorDocument(generated.document, true);
        appliedGeminiManualAddSessionIdsRef.current.add(nextSession.id);
        setError(null);
        const persistence = persistGeminiGeneratedDrafts(
          generated.document,
          mergedSecretEdits,
          nextSession.id,
          nextSession.targetFamily,
          nextSession.generatedDrafts.length,
        );
        // Auto-persist starts synchronously and may advance the shared generation.
        const importGeneration = consoleActionGenerationRef.current;
        void persistence.then((autoPersisted) => {
          if (autoPersisted || importGeneration !== consoleActionGenerationRef.current) {
            return;
          }
          pushAppToast(
            "info",
            t(
              `Gemini 凭证已写入草稿（${nextSession.generatedDrafts.length} 条），保存路由配置后生效。`,
              `Gemini credentials were added to the draft (${nextSession.generatedDrafts.length} entries). Save the route config to apply them.`,
            ),
          );
        });
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [consoleActionGenerationRef, credentialSecretEdits, editorText, persistGeminiGeneratedDrafts, replaceEditorDocument, t],
  );

  return { applyGeminiManualAddSessionResult };
}
