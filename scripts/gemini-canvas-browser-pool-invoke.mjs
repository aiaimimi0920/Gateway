import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { extractQuotedScalar, extractNumberScalar, extractDurationSecondsFromBodyText, inferInvokeUiState as defaultInferInvokeUiState } from "./gemini-canvas-browser-pool-action.mjs";

export function createInvokeAssemblyOwner({
  collectPlayerReadyTargetCandidates, deriveCanvasProxyWsInvokeCandidate,
  deriveProgramRpcInvokeCandidate, inferInvokeUiState = defaultInferInvokeUiState,
}) {
  function buildCanvasProgramInvokeContract(
    operation,
    actionContract,
    transportHints,
    snapshot,
    fallbackPrompt = null,
    captureState = null,
  ) {
    const transport = transportHints || {};
    const actionInput = actionContract?.canvasProgramActionInput ?? null;
    const prompt =
      extractQuotedScalar(actionInput, ["prompt"]) ??
      extractQuotedScalar(snapshot?.bodyText, ["prompt"]) ??
      normalizeString(fallbackPrompt);
    const durationSeconds =
      extractNumberScalar(actionInput, ["duration_seconds", "durationSeconds", "duration"]) ??
      extractDurationSecondsFromBodyText(snapshot?.bodyText);
    const aspectRatio = extractQuotedScalar(actionInput, ["aspect_ratio", "aspectRatio", "aspect"]);
    const uiState = inferInvokeUiState(operation, snapshot);
    const playerReadyTargetCandidates =
      operation === "music" || operation === "video"
        ? collectPlayerReadyTargetCandidates(operation, snapshot, captureState)
        : [];
    const playerReadyTarget = playerReadyTargetCandidates[0] ?? null;
    const wsInvokeCandidate = deriveCanvasProxyWsInvokeCandidate(operation, snapshot, captureState);
    const rpcInvokeCandidate = deriveProgramRpcInvokeCandidate(operation, snapshot, captureState);
    const preferredInvokeCandidate =
      rpcInvokeCandidate ??
      wsInvokeCandidate ??
      null;

    let transportKind = null;
    let target = null;
    if (operation === "music") {
      const laneTarget = [...(transport.musicWsUrls || [])].reverse()[0] ?? null;
      target =
        (uiState === "music_player_ready" ? playerReadyTarget?.url : null) ??
        rpcInvokeCandidate?.requestUrl ??
        wsInvokeCandidate?.wsUrl ??
        laneTarget;
      transportKind = rpcInvokeCandidate?.transportKind
        ?? wsInvokeCandidate?.transportKind
        ?? (
          [...(transport.musicWsUrls || [])].length
            ? "app_music_ws"
            : (uiState ? "official_music_ws_candidate" : null)
        );
    } else if (operation === "video") {
      const laneTarget =
        [...(transport.videoInvokePaths || [])].reverse()[0] ??
        [...(transport.invokeBaseUrls || [])].reverse()[0] ??
        null;
      target =
        (uiState === "video_player_ready" ? playerReadyTarget?.url : null) ??
        rpcInvokeCandidate?.requestUrl ??
        wsInvokeCandidate?.wsUrl ??
        laneTarget;
      transportKind = rpcInvokeCandidate?.transportKind
        ?? wsInvokeCandidate?.transportKind
        ?? (
          ([...(transport.videoInvokePaths || [])].length || [...(transport.invokeBaseUrls || [])].length)
            ? "app_video_http"
            : (uiState ? "official_video_http_candidate" : null)
        );
    } else if (operation === "image") {
      transportKind = wsInvokeCandidate?.transportKind ?? "app_image_http_candidate";
      target = wsInvokeCandidate?.wsUrl ?? null;
    }

    if (
      !transportKind &&
      !actionContract?.canvasProgramAction &&
      !actionInput &&
      !prompt &&
      durationSeconds == null &&
      !aspectRatio &&
      !uiState
    ) {
      return null;
    }

    return {
      operation,
      transportKind,
      target,
      wsUrl: preferredInvokeCandidate?.wsUrl ?? null,
      apiStyle: preferredInvokeCandidate?.apiStyle ?? null,
      requestPath: preferredInvokeCandidate?.requestPath ?? null,
      requestEnvelopeKind: preferredInvokeCandidate?.requestEnvelopeKind ?? null,
      requestUrl: preferredInvokeCandidate?.requestUrl ?? null,
      requestBody: preferredInvokeCandidate?.requestBody ?? null,
      requestRpcId: preferredInvokeCandidate?.requestRpcId ?? null,
      responseRpcId: preferredInvokeCandidate?.responseRpcId ?? null,
      sourcePath: preferredInvokeCandidate?.sourcePath ?? null,
      modelHint: preferredInvokeCandidate?.modelHint ?? null,
      cookieHeader:
        preferredInvokeCandidate?.cookieHeader ??
        normalizeString(captureState?.cookieHeader) ??
        null,
      targetSource: playerReadyTarget?.source ?? null,
      targetMimeType: playerReadyTarget?.mimeType ?? null,
      targetCandidates: playerReadyTargetCandidates,
      actionName: actionContract?.canvasProgramAction ?? null,
      actionInput,
      prompt,
      durationSeconds: Number.isFinite(durationSeconds) && durationSeconds > 0 ? durationSeconds : null,
      aspectRatio,
      uiState,
    };
  }

  return { buildCanvasProgramInvokeContract };
}
