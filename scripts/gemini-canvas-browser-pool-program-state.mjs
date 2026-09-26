import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { extractProgramAppPaths, deriveConversationIdForAppPath, dedupeProgramHandlePairs, isConcreteProgramUrlCandidate } from "./gemini-canvas-browser-pool-program-handles.mjs";

export function createProgramHandleStateOwner({ resolveProgramPageUrl }) {
  function selectStableProgramPair(handlePairs, requestedAppPath = null) {
    const reversed = [...(handlePairs || [])].reverse();
    const shareProxyPair = reversed.find(
      (pair) =>
        pair?.sourceSurface === "canvas_proxy_client" &&
        /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")) &&
        pair.appPath &&
        pair.conversationId &&
        pair.responseId,
    );
    if (shareProxyPair) {
      return shareProxyPair;
    }
    const proxyPair = reversed.find(
      (pair) =>
        pair?.sourceSurface === "canvas_proxy_client" &&
        pair.appPath &&
        pair.conversationId &&
        pair.responseId,
    );
    if (proxyPair) {
      return proxyPair;
    }
    const requestedMatch =
      requestedAppPath &&
      reversed.find(
        (pair) =>
          pair.appPath === requestedAppPath &&
          /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
      );
    if (requestedMatch) {
      return requestedMatch;
    }
    const sharePair = reversed.find((pair) =>
      /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
    );
    if (sharePair) {
      return sharePair;
    }
    return reversed.find((pair) => pair.appPath && pair.conversationId && pair.responseId) ?? null;
  }

  function concreteAppPathFromUrl(url) {
    const matched = extractProgramAppPaths(url || "")[0] ?? null;
    return matched && /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(matched) ? matched : null;
  }

  function selectCanonicalProgramPair(handlePairs, pageUrl, requestedAppPath = null) {
    const stableProgramPair = selectStableProgramPair(handlePairs, requestedAppPath);
    const latestResponsePair =
      [...(handlePairs || [])].reverse().find((pair) => pair.conversationId && pair.responseId) ?? null;
    const finalAppPath = concreteAppPathFromUrl(pageUrl);
    if (
      finalAppPath &&
      latestResponsePair?.appPath === finalAppPath &&
      latestResponsePair?.conversationId &&
      latestResponsePair?.responseId
    ) {
      return latestResponsePair;
    }
    const shareProxyPair =
      [...(handlePairs || [])]
        .reverse()
        .find(
          (pair) =>
            pair?.sourceSurface === "canvas_proxy_client" &&
            /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")) &&
            pair.appPath &&
            pair.conversationId &&
            pair.responseId,
        ) ?? null;
    if (shareProxyPair) {
      return shareProxyPair;
    }
    const proxyPair =
      [...(handlePairs || [])]
        .reverse()
        .find(
          (pair) =>
            pair?.sourceSurface === "canvas_proxy_client" &&
            pair.appPath &&
            pair.conversationId &&
            pair.responseId,
        ) ?? null;
    if (proxyPair) {
      return proxyPair;
    }
    return stableProgramPair ?? latestResponsePair;
  }

  function buildProgramHandleState(baseUrl, args, pageUrl, captureState) {
    const candidatePairs = dedupeProgramHandlePairs(captureState?.handlePairs || []);
    const requestedProgramUrl = resolveProgramPageUrl(baseUrl, args);
    const requestedAppPath =
      extractProgramAppPaths(requestedProgramUrl || "")[0] ||
      normalizeString(args?.appPath) ||
      null;
    const handleHints = captureState?.handleHints || {
      appPaths: [],
      conversationIds: [],
      responseIds: [],
      sharePaths: [],
    };
    const transportHints = captureState?.transportHints || {
      invokeBaseUrls: [],
      musicWsUrls: [],
      videoInvokePaths: [],
    };
    const stableProgramPair = selectStableProgramPair(candidatePairs, requestedAppPath);
    const latestResponsePair =
      [...candidatePairs].reverse().find((pair) => pair.conversationId && pair.responseId) ?? null;
    const canonicalProgramPair =
      selectCanonicalProgramPair(candidatePairs, pageUrl, requestedAppPath) ?? stableProgramPair;
    const finalAppPath = concreteAppPathFromUrl(pageUrl);
    const hintedAppPath =
      [...(handleHints.appPaths || [])]
        .reverse()
        .find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ?? null;
    const effectiveAppPath =
      finalAppPath ?? canonicalProgramPair?.appPath ?? requestedAppPath ?? hintedAppPath ?? null;
    return {
      canvasProgramUrl:
        canonicalProgramPair?.programUrl ??
        (effectiveAppPath ? `${baseUrl.replace(/\/+$/, "")}${effectiveAppPath}` : null) ??
        (isConcreteProgramUrlCandidate(requestedProgramUrl) ? requestedProgramUrl : null),
      pageUrl: pageUrl || null,
      appPath: effectiveAppPath,
      conversationId:
        canonicalProgramPair?.conversationId ??
        normalizeString(args?.conversationId) ??
        deriveConversationIdForAppPath(effectiveAppPath, handleHints.conversationIds) ??
        null,
      responseId: canonicalProgramPair?.responseId ?? [...(handleHints.responseIds || [])].reverse()[0] ?? null,
      lastSeenConversationId:
        latestResponsePair?.conversationId ?? [...(handleHints.conversationIds || [])].reverse()[0] ?? null,
      lastSeenResponseId:
        latestResponsePair?.responseId ?? [...(handleHints.responseIds || [])].reverse()[0] ?? null,
      invokeBaseUrl: [...(transportHints.invokeBaseUrls || [])].reverse()[0] ?? null,
      musicWsUrl: [...(transportHints.musicWsUrls || [])].reverse()[0] ?? null,
      videoInvokePath: [...(transportHints.videoInvokePaths || [])].reverse()[0] ?? null,
      candidatePairs,
      stableProgramPair,
      latestResponsePair,
      capturedAt: new Date().toISOString(),
      lastValidatedAt: new Date().toISOString(),
    };
  }

  return { selectStableProgramPair, concreteAppPathFromUrl, selectCanonicalProgramPair, buildProgramHandleState };
}
