import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { collectProgramHandleSnapshot } from "./gemini-canvas-browser-pool-program-snapshot.mjs";
import {
  mergeProgramHandleHints, deriveConversationIdForAppPath,
} from "./gemini-canvas-browser-pool-program-handles.mjs";
import { extractCanvasProgramActionContractFromText } from "./gemini-canvas-browser-pool-action.mjs";

// Final media probes and result assembly share the caller's still-live capture.
export function createBootstrapResultOwner({
  mergeInvokeContract,
  buildCanvasProgramInvokeContract,
  buildProgramHandleState,
  clickMediaActionButton,
}) {
  async function finalizeBootstrapProgram({
    activePage,
    capture,
    baseUrl,
    args,
    bootstrapOperation,
    bootstrapPrompt,
    timeoutMs,
    discoveryOnly,
    lastSnapshot,
    aggregateHints,
    mergeSnapshot,
    shareUrl,
    shareId,
    shareFollow,
    before,
    canvasProxyPreview,
    shareMaterializationRetry,
    newChatClicked,
    modeSelected,
  }) {
    mergeProgramHandleHints(aggregateHints, capture.state.handleHints);
    const programHandleState = buildProgramHandleState(baseUrl, args, lastSnapshot.url, capture.state);
    const appPath =
      programHandleState.appPath ??
      [...aggregateHints.appPaths]
        .reverse()
        .find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ??
      null;
    const canvasProgramUrl =
      programHandleState.canvasProgramUrl ??
      (appPath ? `${baseUrl.replace(/\/+$/, "")}${appPath}` : null);
    const conversationId =
      programHandleState.conversationId ??
      deriveConversationIdForAppPath(appPath, aggregateHints.conversationIds);
    const responseId =
      programHandleState.responseId ??
      [...aggregateHints.responseIds].reverse()[0] ??
      null;
    const lastSnapshotActionContract =
      extractCanvasProgramActionContractFromText(lastSnapshot.bodyText);
    const actionContract = {
      canvasProgramAction:
        capture.state.actionContract.canvasProgramAction ??
        lastSnapshotActionContract.canvasProgramAction,
      canvasProgramActionInput:
        capture.state.actionContract.canvasProgramActionInput ??
        lastSnapshotActionContract.canvasProgramActionInput,
    };
    let invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        bootstrapOperation,
        actionContract,
        capture.state.transportHints,
        lastSnapshot,
        bootstrapPrompt,
        capture.state,
      ),
    );

    if (
      !discoveryOnly &&
      invokeContract &&
      !invokeContract.target &&
      (invokeContract.uiState === "music_player_ready" ||
        invokeContract.uiState === "video_player_ready")
    ) {
      const playClicked = await clickMediaActionButton(
        activePage,
        bootstrapOperation,
        "play",
        timeoutMs,
      );
      if (playClicked) {
        await activePage.waitForTimeout(2000);
        lastSnapshot = await collectProgramHandleSnapshot(activePage);
        mergeSnapshot(lastSnapshot);
        invokeContract = capture.state.invokeContract;
      }
    }

    if (
      !discoveryOnly &&
      invokeContract &&
      !invokeContract.target &&
      (invokeContract.uiState === "music_player_ready" ||
        invokeContract.uiState === "video_player_ready")
    ) {
      const downloadClicked = await clickMediaActionButton(
        activePage,
        bootstrapOperation,
        "download",
        timeoutMs,
      );
      if (downloadClicked) {
        await activePage.waitForTimeout(2500);
        lastSnapshot = await collectProgramHandleSnapshot(activePage);
        mergeSnapshot(lastSnapshot);
        invokeContract = capture.state.invokeContract;
      }
    }

    if (!canvasProgramUrl || !appPath || !conversationId) {
      throw Object.assign(
        new Error("Gemini Canvas program bootstrap completed without a concrete program handle."),
        {
          status: 504,
          code: "gemini_canvas_program_bootstrap_handle_missing",
          bodyText: lastSnapshot.bodyText ?? null,
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
      finalUrl: lastSnapshot.url,
      pageUrl: lastSnapshot.url,
      bodyText: lastSnapshot.bodyText ?? null,
      canvasProxyPreview,
      shareMaterializationRetry,
      newChatClicked,
      modeSelected,
      canvasProgramUrl,
      appPath,
      conversationId,
      responseId,
      lastSeenConversationId:
        programHandleState.lastSeenConversationId ??
        [...aggregateHints.conversationIds].reverse()[0] ??
        null,
      lastSeenResponseId:
        programHandleState.lastSeenResponseId ??
        [...aggregateHints.responseIds].reverse()[0] ??
        null,
      candidatePairs: programHandleState.candidatePairs,
      stableProgramPair: programHandleState.stableProgramPair,
      latestResponsePair: programHandleState.latestResponsePair,
      aggregateHints,
      transportHints: capture.state.transportHints,
      invokeBaseUrl: programHandleState.invokeBaseUrl,
      musicWsUrl: programHandleState.musicWsUrl,
      videoInvokePath: programHandleState.videoInvokePath,
      canvasProgramAction: actionContract.canvasProgramAction,
      canvasProgramActionInput: actionContract.canvasProgramActionInput,
      canvasProgramInvokeContract: invokeContract,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
      capturedAt: programHandleState.capturedAt,
      lastValidatedAt: programHandleState.lastValidatedAt,
    };
  }

  return { finalizeBootstrapProgram };
}
