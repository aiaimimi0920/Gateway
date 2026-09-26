import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createProgramRpcOwner({ extractAppPath }) {
  function rpcCaptureSourcePathMatches(capture, appPath) {
    const sourcePath = normalizeString(capture?.sourcePath);
    const normalizedAppPath = normalizeString(appPath);
    if (!sourcePath || !normalizedAppPath) {
      return false;
    }
    return sourcePath === normalizedAppPath;
  }

  function rpcCaptureMatchesIds(capture, rpcIds) {
    const rpcId = normalizeString(capture?.rpcId);
    const label = normalizeString(capture?.label);
    return rpcIds.some((candidate) => {
      const normalizedCandidate = normalizeString(candidate);
      return normalizedCandidate && (rpcId === normalizedCandidate || label === normalizedCandidate);
    });
  }

  function selectProgramRpcCapture(captureState, type, rpcIds, appPath) {
    const captures = [...(captureState?.rpcCaptures || [])].reverse();
    return (
      captures.find(
        (capture) =>
          capture?.type === type &&
          rpcCaptureMatchesIds(capture, rpcIds) &&
          rpcCaptureSourcePathMatches(capture, appPath),
      ) ?? null
    );
  }

  function selectRecentRpcCapture(captureState, type, rpcIds) {
    const captures = [...(captureState?.rpcCaptures || [])].reverse();
    return (
      captures.find(
        (capture) => capture?.type === type && rpcCaptureMatchesIds(capture, rpcIds),
      ) ?? null
    );
  }

  function extractModelHintFromRpcText(text) {
    const source = String(text || "");
    const match = source.match(/models\/([^";,\]\\\s]+)/i);
    return normalizeString(match?.[1]);
  }

  function deriveProgramRpcInvokeCandidate(operation, snapshot, captureState) {
    const appPath =
      normalizeString(snapshot?.appPath) ??
      extractAppPath(snapshot?.pageUrl) ??
      extractAppPath(snapshot?.canvasProgramUrl) ??
      extractAppPath(snapshot?.url) ??
      null;
    if (!appPath) {
      return null;
    }
    if (operation === "video") {
      const streamRequestCapture =
        selectProgramRpcCapture(captureState, "request", ["StreamGenerate"], appPath) ??
        selectRecentRpcCapture(captureState, "request", ["StreamGenerate"]) ??
        null;
      const streamResponseCapture =
        selectProgramRpcCapture(captureState, "response", ["StreamGenerate"], appPath) ??
        selectRecentRpcCapture(captureState, "response", ["StreamGenerate"]) ??
        null;
      if (streamRequestCapture || streamResponseCapture) {
        return {
          transportKind: "program_video_streamgenerate_candidate",
          requestEnvelopeKind: "page_stream_generate_form",
          requestUrl:
            normalizeString(streamRequestCapture?.url) ??
            normalizeString(streamResponseCapture?.url),
          requestBody: normalizeString(streamRequestCapture?.bodyText),
          requestRpcId: normalizeString(streamRequestCapture?.rpcId),
          responseRpcId: normalizeString(streamResponseCapture?.rpcId),
          sourcePath:
            normalizeString(streamRequestCapture?.sourcePath) ??
            normalizeString(streamResponseCapture?.sourcePath) ??
            normalizeString(appPath),
          modelHint:
            extractModelHintFromRpcText(streamResponseCapture?.bodyText) ??
            extractModelHintFromRpcText(streamRequestCapture?.bodyText),
          cookieHeader: normalizeString(streamRequestCapture?.cookieHeader),
        };
      }
      const requestCapture =
        selectProgramRpcCapture(captureState, "request", ["hNvQHb", "kwDCne"], appPath) ?? null;
      const responseCapture =
        selectProgramRpcCapture(captureState, "response", ["hNvQHb", "MUAZcd", "kwDCne"], appPath) ??
        null;
      if (!requestCapture && !responseCapture) {
        return null;
      }
      return {
        transportKind: "program_video_batchexecute_candidate",
        requestEnvelopeKind: "page_rpc_form",
        requestUrl: normalizeString(requestCapture?.url) ?? normalizeString(responseCapture?.url),
        requestBody: normalizeString(requestCapture?.bodyText),
        requestRpcId: normalizeString(requestCapture?.rpcId),
        responseRpcId: normalizeString(responseCapture?.rpcId),
        sourcePath: normalizeString(requestCapture?.sourcePath) ?? normalizeString(responseCapture?.sourcePath),
        modelHint:
          extractModelHintFromRpcText(responseCapture?.bodyText) ??
          extractModelHintFromRpcText(requestCapture?.bodyText),
      };
    }
    if (operation === "music") {
      const streamRequestCapture =
        selectProgramRpcCapture(captureState, "request", ["StreamGenerate"], appPath) ??
        selectRecentRpcCapture(captureState, "request", ["StreamGenerate"]) ??
        null;
      const streamResponseCapture =
        selectProgramRpcCapture(captureState, "response", ["StreamGenerate"], appPath) ??
        selectRecentRpcCapture(captureState, "response", ["StreamGenerate"]) ??
        null;
      if (streamRequestCapture || streamResponseCapture) {
        return {
          transportKind: "program_music_streamgenerate_candidate",
          requestEnvelopeKind: "page_stream_generate_form",
          requestUrl:
            normalizeString(streamRequestCapture?.url) ??
            normalizeString(streamResponseCapture?.url),
          requestBody: normalizeString(streamRequestCapture?.bodyText),
          requestRpcId: normalizeString(streamRequestCapture?.rpcId),
          responseRpcId: normalizeString(streamResponseCapture?.rpcId),
          sourcePath:
            normalizeString(streamRequestCapture?.sourcePath) ??
            normalizeString(streamResponseCapture?.sourcePath) ??
            normalizeString(appPath),
          modelHint:
            extractModelHintFromRpcText(streamResponseCapture?.bodyText) ??
            extractModelHintFromRpcText(streamRequestCapture?.bodyText),
          cookieHeader: normalizeString(streamRequestCapture?.cookieHeader),
        };
      }
      const requestCapture =
        selectProgramRpcCapture(captureState, "request", ["hNvQHb", "kwDCne"], appPath) ?? null;
      const responseCapture =
        selectProgramRpcCapture(captureState, "response", ["hNvQHb", "MUAZcd", "kwDCne"], appPath) ??
        null;
      if (!requestCapture && !responseCapture) {
        return null;
      }
      return {
        transportKind: "program_music_batchexecute_candidate",
        requestEnvelopeKind: "page_rpc_form",
        requestUrl: normalizeString(requestCapture?.url) ?? normalizeString(responseCapture?.url),
        requestBody: normalizeString(requestCapture?.bodyText),
        requestRpcId: normalizeString(requestCapture?.rpcId),
        responseRpcId: normalizeString(responseCapture?.rpcId),
        sourcePath: normalizeString(requestCapture?.sourcePath) ?? normalizeString(responseCapture?.sourcePath),
        modelHint:
          extractModelHintFromRpcText(responseCapture?.bodyText) ??
          extractModelHintFromRpcText(requestCapture?.bodyText),
      };
    }
    return null;
  }

  return { deriveProgramRpcInvokeCandidate, extractModelHintFromRpcText };
}
