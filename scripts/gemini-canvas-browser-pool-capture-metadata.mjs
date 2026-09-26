import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

const PROGRAM_RPC_CAPTURE_LIMIT = 80;
const PROGRAM_RPC_TEXT_LIMIT = 40000;
const INTERESTING_PROGRAM_RPC_IDS = new Set([
  "ujx1Bf",
  "ESY5D",
  "L5adhe",
  "XhaU0b",
  "PCck7e",
  "aPya6c",
  "MaZiqc",
  "hNvQHb",
  "kwDCne",
  "MUAZcd",
  "qpEbW",
  "k81mDb",
]);

export function createProgramCaptureOwner({ extractCanvasProxyWsUrlFromText, extractCanvasProxyTargetDomainFromText }) {
  function isInterestingMutation(url, method) {
    return String(method || "GET").toUpperCase() === "POST" &&
      (
        String(url || "").includes("generateContent") ||
        String(url || "").includes("streamGenerateContent") ||
        String(url || "").includes(":predict") ||
        String(url || "").includes("/predict") ||
        String(url || "").includes("imagen")
      );
  }

  function isCanvasProxyClientText(text) {
    const source = String(text || "");
    return (
      source.includes("Browser API Proxy Client") ||
      source.includes("Routing Google API requests via WebSocket") ||
      source.includes("Server WS Endpoint")
    );
  }

  function classifyHandlePairSurface(text) {
    if (!isCanvasProxyClientText(text)) {
      return {
        sourceSurface: null,
        sourceWsUrl: null,
        sourceTargetDomain: null,
      };
    }
    return {
      sourceSurface: "canvas_proxy_client",
      sourceWsUrl: extractCanvasProxyWsUrlFromText(text),
      sourceTargetDomain: extractCanvasProxyTargetDomainFromText(text),
    };
  }

  function readRpcIdFromRequestUrl(url) {
    try {
      return new URL(String(url || "")).searchParams.get("rpcids");
    } catch {
      return null;
    }
  }

  function shouldCaptureProgramHandleTraffic(url, method, operation) {
    const normalizedMethod = String(method || "GET").toUpperCase();
    if (normalizedMethod !== "POST") {
      return false;
    }
    if (/\/StreamGenerate|\/batchexecute|assistant\.lamda\.BardFrontendService|clients6\.google\.com\/punctual/i.test(url)) {
      return true;
    }
    if (operation === "image" || operation === "music" || operation === "video" || operation === "bootstrap_program") {
      return isInterestingMutation(url, normalizedMethod);
    }
    return false;
  }

  function trimProgramRpcCaptureText(value, limit = PROGRAM_RPC_TEXT_LIMIT) {
    if (typeof value !== "string") {
      return value ?? null;
    }
    if (
      value.includes("Browser API Proxy Client") ||
      value.includes("Routing Google API requests via WebSocket") ||
      value.includes("System Logs Output")
    ) {
      return value;
    }
    if (value.length <= limit) {
      return value;
    }
    return `${value.slice(0, limit)}...[truncated]`;
  }

  function classifyProgramRpcCapture(url, method) {
    const rpcId = readRpcIdFromRequestUrl(url);
    if (rpcId && INTERESTING_PROGRAM_RPC_IDS.has(rpcId)) {
      return {
        rpcId,
        label: rpcId,
        sourcePath: (() => {
          try {
            return new URL(url).searchParams.get("source-path");
          } catch {
            return null;
          }
        })(),
        method: String(method || "GET").toUpperCase(),
      };
    }
    if (/\/StreamGenerate/i.test(url)) {
      return {
        rpcId: null,
        label: "StreamGenerate",
        sourcePath: null,
        method: String(method || "GET").toUpperCase(),
      };
    }
    return null;
  }

  function pushProgramRpcCapture(state, capture) {
    if (!state.rpcCaptures) {
      state.rpcCaptures = [];
    }
    state.rpcCaptures.push({
      t: Date.now(),
      ...capture,
    });
    if (state.rpcCaptures.length > PROGRAM_RPC_CAPTURE_LIMIT) {
      state.rpcCaptures.splice(0, state.rpcCaptures.length - PROGRAM_RPC_CAPTURE_LIMIT);
    }
  }

  function extractRequestCookieHeader(headers) {
    if (!headers || typeof headers !== "object") {
      return null;
    }
    const direct = normalizeString(headers.cookie) ?? normalizeString(headers.Cookie);
    return direct && direct.includes("=") ? direct : null;
  }

  async function captureCookieHeaderFromContext(page, requestUrl) {
    try {
      const target = new URL(requestUrl);
      const origin = `${target.protocol}//${target.host}`;
      const cookies = await page.context().cookies([origin, requestUrl]);
      const cookieHeader = cookies
        .map((cookie) => `${cookie.name}=${cookie.value}`)
        .join("; ")
        .trim();
      return cookieHeader && cookieHeader.includes("=") ? cookieHeader : null;
    } catch {
      return null;
    }
  }

  return {
    classifyHandlePairSurface, readRpcIdFromRequestUrl, shouldCaptureProgramHandleTraffic,
    trimProgramRpcCaptureText, classifyProgramRpcCapture, pushProgramRpcCapture,
    extractRequestCookieHeader, captureCookieHeaderFromContext,
  };
}
