import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createInvokeContractOwner({ scorePlayerReadyTargetCandidate }) {
  function mergeInvokeContract(target, incoming) {
    if (!incoming) {
      return target;
    }
    if (!target) {
      return { ...incoming };
    }
    const rankUiState = (value) => {
      switch (value) {
        case "music_player_ready":
        case "video_player_ready":
          return 3;
        case "retry_without_app_visible":
          return 2;
        case "music_generating":
        case "video_generating":
          return 1;
        default:
          return 0;
      }
    };
    const scoreContractTarget = (contract) => {
      if (!contract?.target) {
        return -1;
      }
      return scorePlayerReadyTargetCandidate(contract.operation ?? incoming.operation ?? target?.operation, {
        url: contract.target,
        source: contract.targetSource,
        mimeType: contract.targetMimeType,
        kind: contract.operation === "music" ? "audio" : contract.operation,
        download: contract.targetSource ? /download/i.test(String(contract.targetSource)) : false,
      });
    };
    const mergeTargetCandidates = (left, right) => {
      const merged = [];
      for (const candidate of [...(left || []), ...(right || [])]) {
        if (!candidate?.url) {
          continue;
        }
        const existingIndex = merged.findIndex((entry) => entry.url === candidate.url);
        if (existingIndex === -1) {
          merged.push(candidate);
        } else if ((merged[existingIndex]?.score ?? -1) <= (candidate?.score ?? -1)) {
          merged[existingIndex] = candidate;
        }
      }
      return merged.slice(0, 12);
    };
    const useIncomingTarget = scoreContractTarget(incoming) > scoreContractTarget(target);
    const rankLane = (contract) => {
      const transportKind = normalizeString(contract?.transportKind);
      const requestEnvelopeKind = normalizeString(contract?.requestEnvelopeKind);
      if (
        transportKind === "program_music_streamgenerate_candidate" ||
        transportKind === "program_video_streamgenerate_candidate"
      ) {
        return 50;
      }
      if (requestEnvelopeKind === "page_stream_generate_form") {
        return 40;
      }
      if (
        transportKind === "program_music_batchexecute_candidate" ||
        transportKind === "program_video_batchexecute_candidate"
      ) {
        return 30;
      }
      if (transportKind === "canvas_program_ws_candidate") {
        return 10;
      }
      if (requestEnvelopeKind === "canvas_proxy_request") {
        return 5;
      }
      return 0;
    };
    const useIncomingLane =
      rankLane(incoming) > rankLane(target) ||
      (rankLane(incoming) === rankLane(target) && useIncomingTarget);
    const primaryLane = useIncomingLane ? incoming : target;
    const secondaryLane = useIncomingLane ? target : incoming;
    return {
      operation: target.operation ?? incoming.operation ?? null,
      transportKind: primaryLane.transportKind ?? secondaryLane.transportKind ?? null,
      wsUrl: primaryLane.wsUrl ?? secondaryLane.wsUrl ?? null,
      apiStyle: primaryLane.apiStyle ?? secondaryLane.apiStyle ?? null,
      requestPath: primaryLane.requestPath ?? secondaryLane.requestPath ?? null,
      requestEnvelopeKind:
        primaryLane.requestEnvelopeKind ?? secondaryLane.requestEnvelopeKind ?? null,
      requestUrl: primaryLane.requestUrl ?? secondaryLane.requestUrl ?? null,
      requestBody: primaryLane.requestBody ?? secondaryLane.requestBody ?? null,
      requestRpcId: primaryLane.requestRpcId ?? secondaryLane.requestRpcId ?? null,
      responseRpcId:
        primaryLane.responseRpcId ?? secondaryLane.responseRpcId ?? null,
      sourcePath: primaryLane.sourcePath ?? secondaryLane.sourcePath ?? null,
      modelHint: primaryLane.modelHint ?? secondaryLane.modelHint ?? null,
      target: useIncomingTarget ? incoming.target ?? target.target ?? null : target.target ?? incoming.target ?? null,
      targetSource: useIncomingTarget
        ? incoming.targetSource ?? target.targetSource ?? null
        : target.targetSource ?? incoming.targetSource ?? null,
      targetMimeType: useIncomingTarget
        ? incoming.targetMimeType ?? target.targetMimeType ?? null
        : target.targetMimeType ?? incoming.targetMimeType ?? null,
      targetCandidates: mergeTargetCandidates(target.targetCandidates, incoming.targetCandidates),
      actionName: target.actionName ?? incoming.actionName ?? null,
      actionInput: target.actionInput ?? incoming.actionInput ?? null,
      prompt: target.prompt ?? incoming.prompt ?? null,
      durationSeconds: target.durationSeconds ?? incoming.durationSeconds ?? null,
      aspectRatio: target.aspectRatio ?? incoming.aspectRatio ?? null,
      uiState:
        rankUiState(incoming.uiState) > rankUiState(target.uiState)
          ? incoming.uiState ?? null
          : target.uiState ?? incoming.uiState ?? null,
    };
  }

  return { mergeInvokeContract };
}
