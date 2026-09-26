import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createProxyDiscoveryOwner({ extractAppPath, extractModelHintFromRpcText }) {
  function extractCanvasProxyWsUrlFromText(text) {
    const source = String(text || "");
    const patterns = [
      /DEFAULT_ENDPOINT\s*=\s*"([^"]+)"/i,
      /constructor\s*\(\s*endpoint\s*=\s*"([^"]+)"/i,
      /placeholder="((?:wss?|WSS?):\/\/[^"]+)"/i,
      /\b((?:wss?|WSS?):\/\/127\.0\.0\.1:\d+(?:\/ws)?)\b/i,
    ];
    for (const pattern of patterns) {
      const matched = source.match(pattern)?.[1];
      const normalized = normalizeString(matched);
      if (normalized) {
        return normalized;
      }
    }
    return null;
  }

  function extractCanvasProxyTargetDomainFromText(text) {
    const source = String(text || "");
    return (
      normalizeString(source.match(/targetDomain\s*=\s*"([^"]+)"/i)?.[1]) ??
      normalizeString(source.match(/https:\/\/([^/"'\s]+generativelanguage\.googleapis\.com)/i)?.[1]) ??
      null
    );
  }

  function collectCanvasProxyContractTexts(snapshot, captureState) {
    const values = [
      snapshot?.bodyText,
      snapshot?.bodyBase64,
      ...(captureState?.rpcCaptures || []).flatMap((capture) => [capture?.bodyText, capture?.url]),
    ];
    return values
      .map((value) => String(value || ""))
      .filter(
        (value) =>
          value.includes("Browser API Proxy Client") ||
          value.includes("Routing Google API requests via WebSocket") ||
          value.includes("Server WS Endpoint") ||
          value.includes("targetDomain") ||
          value.includes("proxy_request"),
      );
  }

  function deriveCanvasProxyWsInvokeCandidate(operation, snapshot, captureState) {
    const appPath =
      normalizeString(snapshot?.appPath) ??
      extractAppPath(snapshot?.pageUrl) ??
      extractAppPath(snapshot?.canvasProgramUrl) ??
      extractAppPath(snapshot?.url) ??
      null;
    const texts = collectCanvasProxyContractTexts(snapshot, captureState);
    for (const text of texts) {
      const wsUrl = extractCanvasProxyWsUrlFromText(text);
      const targetDomain = extractCanvasProxyTargetDomainFromText(text);
      if (!wsUrl && !targetDomain) {
        continue;
      }
      return {
        transportKind: "canvas_program_ws_candidate",
        wsUrl,
        apiStyle: targetDomain ? "google_generative_language" : null,
        requestPath: null,
        requestEnvelopeKind: "canvas_proxy_request",
        sourcePath: appPath,
        modelHint: extractModelHintFromRpcText(text),
        operation,
      };
    }
    return null;
  }

  return { collectCanvasProxyContractTexts, deriveCanvasProxyWsInvokeCandidate,
    extractCanvasProxyWsUrlFromText, extractCanvasProxyTargetDomainFromText,
  };
}
