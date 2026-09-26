import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { deriveConversationIdForAppPath } from "./gemini-canvas-browser-pool-program-handles.mjs";
import { extractCanvasProgramActionContractFromText } from "./gemini-canvas-browser-pool-action.mjs";

export function createBootstrapPreviewResultOwner({
  buildProgramHandleState, mergeInvokeContract, buildCanvasProgramInvokeContract,
}) {
  function buildBootstrapPreviewResult({
    baseUrl, args, previewSnapshot, afterPreview, captureState, aggregateHints,
    bootstrapOperation, bootstrapPrompt, shareUrl, shareId, shareFollow,
    discoveryOnly, canvasProxyPreview, shareMaterializationRetry, before,
  }) {
    const previewHandleState = buildProgramHandleState(
      baseUrl,
      args,
      previewSnapshot?.url ?? afterPreview.url,
      captureState,
    );
    const previewAppPath =
      previewHandleState.appPath ??
      [...aggregateHints.appPaths]
        .reverse()
        .find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ??
      null;
    const previewCanvasProgramUrl =
      previewHandleState.canvasProgramUrl ??
      (previewAppPath ? `${baseUrl.replace(/\/+$/, "")}${previewAppPath}` : null);
    const previewConversationId =
      previewHandleState.conversationId ??
      deriveConversationIdForAppPath(previewAppPath, aggregateHints.conversationIds);
    const previewResponseId =
      previewHandleState.responseId ??
      [...aggregateHints.responseIds].reverse()[0] ??
      null;
    const previewActionContract = {
      canvasProgramAction:
        captureState.actionContract.canvasProgramAction ??
        extractCanvasProgramActionContractFromText(previewSnapshot?.bodyText)?.canvasProgramAction ??
        null,
      canvasProgramActionInput:
        captureState.actionContract.canvasProgramActionInput ??
        extractCanvasProgramActionContractFromText(previewSnapshot?.bodyText)?.canvasProgramActionInput ??
        null,
    };
    const previewInvokeContract = mergeInvokeContract(
      captureState.invokeContract,
      buildCanvasProgramInvokeContract(
        bootstrapOperation,
        previewActionContract,
        captureState.transportHints,
        previewSnapshot ?? afterPreview,
        bootstrapPrompt,
        captureState,
      ),
    );
    if (!previewCanvasProgramUrl || !previewAppPath || !previewConversationId) {
      throw Object.assign(
        new Error("Gemini Canvas program direct launch completed without a concrete program handle."),
        {
          status: 504,
          code: "gemini_canvas_program_bootstrap_handle_missing",
          bodyText: previewSnapshot?.bodyText ?? afterPreview.bodyText ?? null,
        },
      );
    }
    return {
      operation: "bootstrap_program",
      runtimeStateObjectKey: normalizeString(args.runtimeStateObjectKey),
      shareUrl,
      shareId,
      shareFollowKind: shareFollow.kind,
      bootstrapOperation,
      bootstrapPrompt,
      discoveryOnly,
      beforeUrl: before.url,
      finalUrl: previewSnapshot?.url ?? afterPreview.url,
      pageUrl: previewSnapshot?.url ?? afterPreview.url,
      bodyText: previewSnapshot?.bodyText ?? afterPreview.bodyText ?? null,
      canvasProxyPreview,
      shareMaterializationRetry,
      newChatClicked: false,
      modeSelected: false,
      canvasProgramUrl: previewCanvasProgramUrl,
      appPath: previewAppPath,
      conversationId: previewConversationId,
      responseId: previewResponseId,
      lastSeenConversationId:
        previewHandleState.lastSeenConversationId ??
        [...aggregateHints.conversationIds].reverse()[0] ??
        null,
      lastSeenResponseId:
        previewHandleState.lastSeenResponseId ??
        [...aggregateHints.responseIds].reverse()[0] ??
        null,
      candidatePairs: previewHandleState.candidatePairs,
      stableProgramPair: previewHandleState.stableProgramPair,
      latestResponsePair: previewHandleState.latestResponsePair,
      aggregateHints,
      transportHints: captureState.transportHints,
      invokeBaseUrl: previewHandleState.invokeBaseUrl,
      musicWsUrl: previewHandleState.musicWsUrl,
      videoInvokePath: previewHandleState.videoInvokePath,
      canvasProgramAction: previewActionContract.canvasProgramAction,
      canvasProgramActionInput: previewActionContract.canvasProgramActionInput,
      canvasProgramInvokeContract: previewInvokeContract,
      networkEvents: captureState.events,
    };
  }

  return { buildBootstrapPreviewResult };
}
