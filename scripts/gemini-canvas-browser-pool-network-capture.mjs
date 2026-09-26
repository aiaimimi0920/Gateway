import { extractProgramHandleHintsFromText, mergeProgramHandleHints, dedupeProgramHandlePairs, extractProgramHandlePairs } from "./gemini-canvas-browser-pool-program-handles.mjs";
import { mergeActionContract, extractCanvasProgramActionContractFromText } from "./gemini-canvas-browser-pool-action.mjs";
import { createProgramResponseCapture } from "./gemini-canvas-program-handle-response-capture.mjs";
import {
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
  readPlaywrightResponseBody,
  readPlaywrightResponseText,
} from "./gemini-canvas-browser-pool-body.mjs";

export function createNetworkCaptureOwner({
  extractTransportHintsFromNetworkUrl, mergeTransportHints, mergeInvokeContract, buildCanvasProgramInvokeContract,
  classifyHandlePairSurface, readRpcIdFromRequestUrl, shouldCaptureProgramHandleTraffic,
  trimProgramRpcCaptureText, classifyProgramRpcCapture, pushProgramRpcCapture,
  extractRequestCookieHeader, captureCookieHeaderFromContext,
  isInterestingNetworkUrl, isLikelyAvatarUrl, inferMimeTypeFromUrl, isAudioLikeMimeType, isAudioLikeUrl, pushUniqueMediaUrl,
  createResponseCapture = createProgramResponseCapture,
}) {
  function startNetworkCapture(page, operation, existingState = null) {
    const state = existingState ?? {
      streamGenerateRequestAt: null,
      streamGenerateResponseAt: null,
      imageUrls: [],
      audioUrls: [],
      videoUrls: [],
      handlePairs: [],
      rpcCaptures: [],
      handleHints: {
        appPaths: [],
        conversationIds: [],
        responseIds: [],
        sharePaths: [],
      },
      transportHints: {
        invokeBaseUrls: [],
        musicWsUrls: [],
        videoInvokePaths: [],
      },
      actionContract: {
        canvasProgramAction: null,
        canvasProgramActionInput: null,
      },
      invokeContract: null,
      events: [],
    };

    // Page switches reuse state; stopped listeners must not publish late results.
    let stopped = false;
    let responseCapture = null;

    const pushEvent = (event) => {
      state.events.push({
        t: Date.now(),
        ...event,
      });
      if (state.events.length > 160) {
        state.events.shift();
      }
    };

    const summarizeHeaders = (headers) =>
      Object.fromEntries(
        Object.entries(headers || {}).flatMap(([key, value]) => {
          const normalized = String(key || "").toLowerCase();
          if (![
            "authorization",
            "x-origin",
            "x-goog-authuser",
            "x-goog-api-key",
            "content-type",
            "origin",
            "referer",
          ].includes(normalized)) {
            return [];
          }
          if (normalized === "authorization" || normalized === "x-goog-api-key") {
            return [[normalized, value ? "<present>" : "<missing>"]];
          }
          return [[normalized, value]];
        }),
      );

    const onRequest = (request) => {
      if (stopped) return;
      const url = request.url();
      if (!isInterestingNetworkUrl(url)) {
        return;
      }
      if (
        request.method().toUpperCase() === "POST" &&
        url.includes("/StreamGenerate")
      ) {
        state.streamGenerateRequestAt = Date.now();
      }
      if (shouldCaptureProgramHandleTraffic(url, request.method(), operation)) {
        let postData = null;
        try {
          postData = request.postData();
        } catch {
          postData = null;
        }
        const rawHeaders = request.headers() || {};
        const requestCookieHeader = extractRequestCookieHeader(rawHeaders);
        const handleHints = extractProgramHandleHintsFromText(`${url}\n${postData || ""}`);
        mergeProgramHandleHints(state.handleHints, handleHints);
        mergeTransportHints(state.transportHints, extractTransportHintsFromNetworkUrl(url));
        mergeActionContract(
          state.actionContract,
          extractCanvasProgramActionContractFromText(`${url}\n${postData || ""}`),
        );
        state.invokeContract = mergeInvokeContract(
          state.invokeContract,
          buildCanvasProgramInvokeContract(
            operation,
            state.actionContract,
            state.transportHints,
            { bodyText: postData ?? "", buttons: [] },
            null,
            state,
          ),
        );
        state.handlePairs = dedupeProgramHandlePairs([
          ...state.handlePairs,
          ...extractProgramHandlePairs(`${url}\n${postData || ""}`, {
            sourceKind: "request",
            sourceUrl: url,
            sourceRpc: readRpcIdFromRequestUrl(url),
            ...classifyHandlePairSurface(`${url}\n${postData || ""}`),
            ts: new Date().toISOString(),
          }),
        ]);
        pushEvent({
          type: "request",
          method: request.method(),
          url,
          headers: summarizeHeaders(request.headers()),
          postData:
            typeof postData === "string" && postData.length > 6000
              ? `${postData.slice(0, 6000)}...[truncated]`
              : postData,
        });
        const rpcCapture = classifyProgramRpcCapture(url, request.method());
        if (rpcCapture) {
          pushProgramRpcCapture(state, {
            type: "request",
            ...rpcCapture,
            url,
            cookieHeader: requestCookieHeader,
            headers: summarizeHeaders(request.headers()),
            bodyText: trimProgramRpcCaptureText(postData),
          });
        }
        if (
          requestCookieHeader &&
          String(rpcCapture?.label || "").toLowerCase() === "streamgenerate"
        ) {
          state.cookieHeader = requestCookieHeader;
        } else if (String(rpcCapture?.label || "").toLowerCase() === "streamgenerate") {
          void captureCookieHeaderFromContext(page, url).then((cookieHeader) => {
            if (!stopped && cookieHeader) {
              state.cookieHeader = cookieHeader;
            }
          });
        }
      }
    };

    const onResponse = async (response) => {
      if (stopped) return;
      const url = response.url();
      if (!isInterestingNetworkUrl(url)) {
        return;
      }
      const request = response.request();
      const shouldCaptureHandleTraffic = shouldCaptureProgramHandleTraffic(
        url,
        request.method(),
        operation,
      );
      if (url.includes("/StreamGenerate")) {
        state.streamGenerateResponseAt = Date.now();
      }

      const contentType = response.headers()["content-type"] ?? "";
      if (shouldCaptureHandleTraffic) {
        let text = null;
        try {
          text = await readPlaywrightResponseText(
            response,
            BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
            "program response",
          );
        } catch (error) {
          text = `[[response text unavailable: ${error instanceof Error ? error.message : String(error)}]]`;
        }
        if (stopped) return;
        const handleHints = extractProgramHandleHintsFromText(`${url}\n${text || ""}`);
        mergeProgramHandleHints(state.handleHints, handleHints);
        mergeTransportHints(state.transportHints, extractTransportHintsFromNetworkUrl(url));
        mergeActionContract(
          state.actionContract,
          extractCanvasProgramActionContractFromText(`${url}\n${text || ""}`),
        );
        state.invokeContract = mergeInvokeContract(
          state.invokeContract,
          buildCanvasProgramInvokeContract(
            operation,
            state.actionContract,
            state.transportHints,
            { bodyText: text ?? "", buttons: [] },
            null,
            state,
          ),
        );
        state.handlePairs = dedupeProgramHandlePairs([
          ...state.handlePairs,
          ...extractProgramHandlePairs(`${url}\n${text || ""}`, {
            sourceKind: "response",
            sourceUrl: url,
            sourceRpc: readRpcIdFromRequestUrl(url),
            ...classifyHandlePairSurface(`${url}\n${text || ""}`),
            ts: new Date().toISOString(),
          }),
        ]);
        pushEvent({
          type: "response",
          status: response.status(),
          url,
          requestHeaders: summarizeHeaders(request.headers()),
          responseHeaders: summarizeHeaders(response.headers()),
          text:
            typeof text === "string" && text.length > 12000
              ? `${text.slice(0, 12000)}...[truncated]`
              : text,
        });
        const rpcCapture = classifyProgramRpcCapture(url, request.method());
        if (rpcCapture) {
          pushProgramRpcCapture(state, {
            type: "response",
            ...rpcCapture,
            url,
            status: response.status(),
            requestHeaders: summarizeHeaders(request.headers()),
            responseHeaders: summarizeHeaders(response.headers()),
            bodyText: trimProgramRpcCaptureText(text),
          });
        }
      }

      if (operation === "image") {
        if (
          (/image\//i.test(contentType) || /gg-dl|rd-gg-dl|googleusercontent/i.test(url)) &&
          !isLikelyAvatarUrl(url)
        ) {
          let bodyBase64 = null;
          if (/image\//i.test(contentType)) {
            try {
              const bytes = await readPlaywrightResponseBody(
                response,
                BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
                "image",
              );
              if (stopped) return;
              if (bytes?.length) {
                bodyBase64 = Buffer.from(bytes).toString("base64");
              }
            } catch {
              bodyBase64 = null;
            }
          }
          if (stopped) return;
          pushUniqueMediaUrl(state.imageUrls, {
            url,
            mimeType: inferMimeTypeFromUrl(url, contentType.split(";")[0] || "image/png"),
            bodyBase64,
          });
        }
        return;
      }

      if (operation === "tts" || operation === "music") {
        if (isAudioLikeMimeType(contentType) || isAudioLikeUrl(url)) {
          let bodyBase64 = null;
          if (isAudioLikeMimeType(contentType)) {
            try {
              const bytes = await readPlaywrightResponseBody(
                response,
                BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
                "audio",
              );
              if (stopped) return;
              if (bytes?.length) {
                bodyBase64 = Buffer.from(bytes).toString("base64");
              }
            } catch {
              bodyBase64 = null;
            }
          }
          if (stopped) return;
          pushUniqueMediaUrl(state.audioUrls, {
            url,
            mimeType: inferMimeTypeFromUrl(url, contentType.split(";")[0] || "audio/wav"),
            bodyBase64,
          });
        }
      }

      if (operation === "video" || operation === "music") {
        if (
          /video\//i.test(contentType) ||
          /contribution\.usercontent\.google\.com|googlevideo\.com|gvt1\.com|\.mp4(\?|$)|\.webm(\?|$)/i.test(url)
        ) {
          pushUniqueMediaUrl(state.videoUrls, {
            url,
            mimeType: inferMimeTypeFromUrl(url, contentType.split(";")[0] || "video/mp4"),
          });
        }
      }
    };

    const onWebSocket = (websocket) => {
      if (stopped) return;
      const url = websocket.url();
      mergeTransportHints(state.transportHints, extractTransportHintsFromNetworkUrl(url));
      pushEvent({
        type: "websocket",
        url,
      });
    };

    const listeners = [["request", onRequest], ["websocket", onWebSocket]];
    const stop = () => {
      stopped = true;
      const errors = [];
      for (const [event, listener] of listeners) {
        try { page.off(event, listener); } catch (error) { errors.push(error); }
      }
      let cleanup = Promise.resolve();
      try { cleanup = Promise.resolve(responseCapture?.stop()); } catch (error) { errors.push(error); }
      if (errors.length) {
        void cleanup.catch(() => undefined);
        throw errors[0];
      }
      return cleanup;
    };
    try {
      for (const [event, listener] of listeners) page.on(event, listener);
      responseCapture = createResponseCapture(page, {
        requestFilter: isInterestingNetworkUrl, onResponse, pageOnly: true,
        bodyBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
        binaryBodyBytes: BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
        onError: (error) => pushEvent({ type: "capture-error", code: error.code, message: error.message }),
      });
    } catch (error) {
      // Failed acquisition has no returned owner; revoke callbacks and retain its error.
      try { void stop().catch(() => undefined); } catch { /* Preserve the registration failure. */ }
      throw error;
    }

    return { state, stop, ready: responseCapture.ready };
  }

  return { startNetworkCapture };
}
