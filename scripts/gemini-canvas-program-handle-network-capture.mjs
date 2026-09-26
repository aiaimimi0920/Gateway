import { createProgramCaptureBudget } from "./gemini-canvas-program-handle-capture-budget.mjs";
import { createProgramResponseCapture } from "./gemini-canvas-program-handle-response-capture.mjs";

// Page adoption retains the budget and outstanding native reads with its state.
const stateBudgets = new WeakMap();

// The probe context owns request listeners and bounded CDP capture across page adoption.
export function createProgramNetworkCaptureOwner({
  operation,
  extractHandleHintsFromText,
  classifyHandlePairSurface,
  extractHandlePairsFromText,
  readRpcIdFromUrl,
  extractTransportHintsFromUrl,
  extractCanvasProgramActionContractFromText,
  mergeHints,
  mergeHandlePairs,
  mergeTransportHints,
  mergeActionContract,
  classifyProgramRpcCapture,
  pushProgramRpcCapture,
  trimProgramRpcCaptureText,
  extractRequestCookieHeader,
  captureCookieHeaderFromContext,
  textPreview,
  isAudioLikeMimeType,
  isAudioLikeUrl,
  isVideoLikeUrl,
  pushUniqueMediaUrl,
  captureBudgetLimits,
  createResponseCapture = createProgramResponseCapture,
}) {
  function startNetworkCapture(page, existingState = null) {
    const state = existingState ?? {
      requests: [],
      responses: [],
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
    };

    const context = page.context();
    let stopped = false, stopping = null, responseCapture = null;
    const budget = stateBudgets.get(state) ?? createProgramCaptureBudget(captureBudgetLimits);
    budget.assertHealthy();
    stateBudgets.set(state, budget);
    const fail = (error) => { budget.fail(error); void stop().catch(() => undefined); };
    const protect = (handler) => (...args) => {
      if (stopped) return;
      try {
        const result = handler(...args);
        return result?.then ? result.catch((error) => { if (!stopped) fail(error); }) : result;
      } catch (error) { fail(error); }
    };

    const requestFilter = (url) =>
      /StreamGenerate|batchexecute|assistant\.lamda\.BardFrontendService|predictLongRunning|BidiGenerateMusic|googleapis\.com|googleusercontent\.com|contribution\.usercontent\.google\.com|googlevideo\.com|gvt1\.com|\/share\/|\/app(?:\/|$)/i.test(
        String(url || ""),
      );

    const onRequest = protect((request) => {
      if (stopped) {
        return;
      }
      const url = request.url();
      if (!requestFilter(url)) {
        return;
      }
      const postData = request.postData() ?? "";
      const rawHeaders = request.headers() || {};
      const requestCookieHeader = extractRequestCookieHeader(rawHeaders);
      budget.inspectSource(url, postData, requestCookieHeader);
      const hints = extractHandleHintsFromText(`${url}\n${postData}`);
      const pairSurface = classifyHandlePairSurface(`${url}\n${postData}`);
      const handlePairs = extractHandlePairsFromText(`${url}\n${postData}`, {
        sourceKind: "request",
        sourceUrl: url,
        sourceRpc: readRpcIdFromUrl(url),
        ...pairSurface,
        ts: new Date().toISOString(),
      });
      const transportHints = extractTransportHintsFromUrl(url);
      const actionContract = extractCanvasProgramActionContractFromText(`${url}\n${postData}`);
      budget.accept({ url, postData, rawHeaders, hints, handlePairs, transportHints, actionContract });
      mergeHints(state.handleHints, hints);
      mergeHandlePairs(state.handlePairs, handlePairs);
      mergeTransportHints(state.transportHints, transportHints);
      mergeActionContract(state.actionContract, actionContract);
      const rpcCapture = classifyProgramRpcCapture(url, request.method());
      if (rpcCapture) {
        pushProgramRpcCapture(state, {
          type: "request",
          ...rpcCapture,
          url,
          cookieHeader: requestCookieHeader,
          bodyText: trimProgramRpcCaptureText(postData),
        });
      }
      if (
        requestCookieHeader &&
        String(rpcCapture?.label || "").toLowerCase() === "streamgenerate"
      ) {
        state.cookieHeader = requestCookieHeader;
      } else if (String(rpcCapture?.label || "").toLowerCase() === "streamgenerate") {
        const release = budget.beginRead();
        void captureCookieHeaderFromContext(page, url).then((cookieHeader) => {
          if (stopped || !cookieHeader) return;
          budget.inspectSource(cookieHeader);
          budget.accept({ cookieHeader });
          state.cookieHeader = cookieHeader;
        }).catch((error) => { if (!stopped) fail(error); }).finally(release);
      }
      state.requests.push({
        ts: new Date().toISOString(),
        method: request.method(),
        url,
        resourceType: request.resourceType(),
        postDataPreview: textPreview(postData, 1600),
        handleHints: hints,
        handlePairs,
        transportHints,
      });
    });

    const onResponse = protect(async (response) => {
      if (stopped) {
        return;
      }
      const url = response.url();
      if (!requestFilter(url)) {
        return;
      }
      const request = response.request();
      const contentType = response.headers()["content-type"] ?? "";
      budget.inspectSource(url, contentType);
      let bodyText = "";
      const release = budget.beginRead();
      try {
        bodyText = await response.text();
      } catch {
        bodyText = "";
      } finally {
        release();
      }
      if (stopped) {
        return;
      }
      budget.inspectSource(bodyText);
      const hints = extractHandleHintsFromText(`${url}\n${bodyText}`);
      const pairSurface = classifyHandlePairSurface(`${url}\n${bodyText}`);
      const handlePairs = extractHandlePairsFromText(`${url}\n${bodyText}`, {
        sourceKind: "response",
        sourceUrl: url,
        sourceRpc: readRpcIdFromUrl(url),
        ...pairSurface,
        ts: new Date().toISOString(),
      });
      const transportHints = extractTransportHintsFromUrl(url);
      const actionContract = extractCanvasProgramActionContractFromText(`${url}\n${bodyText}`);
      budget.accept({ url, contentType, bodyText, hints, handlePairs, transportHints, actionContract });
      mergeHints(state.handleHints, hints);
      mergeHandlePairs(state.handlePairs, handlePairs);
      mergeTransportHints(state.transportHints, transportHints);
      const rpcCapture = classifyProgramRpcCapture(url, request.method());
      if (rpcCapture) {
        pushProgramRpcCapture(state, {
          type: "response",
          ...rpcCapture,
          url,
          status: response.status(),
          contentType,
          bodyText: trimProgramRpcCaptureText(bodyText),
        });
      }
      state.responses.push({
        ts: new Date().toISOString(),
        status: response.status(),
        url,
        contentType: response.headers()["content-type"] ?? null,
        bodyPreview: textPreview(bodyText, 4000),
        ...(rpcCapture
          ? {
              bodyText: trimProgramRpcCaptureText(bodyText),
            }
          : {}),
        handleHints: hints,
        handlePairs,
        transportHints,
      });
      if (operation === "music" && (isAudioLikeMimeType(contentType) || isAudioLikeUrl(url))) {
        pushUniqueMediaUrl(state.audioUrls, {
          url,
          mimeType: contentType.split(";")[0] || null,
        });
      }
      if ((operation === "music" || operation === "video") && isVideoLikeUrl(url)) {
        pushUniqueMediaUrl(state.videoUrls, {
          url,
          mimeType: contentType.split(";")[0] || null,
        });
      }
      mergeActionContract(
        state.actionContract,
        actionContract,
      );
    });

    const onWebSocket = protect((websocket) => {
      if (stopped) {
        return;
      }
      const url = websocket.url();
      if (!requestFilter(url)) {
        return;
      }
      budget.inspectSource(url);
      const transportHints = extractTransportHintsFromUrl(url);
      budget.accept({ url, transportHints });
      mergeTransportHints(state.transportHints, transportHints);
      state.requests.push({
        ts: new Date().toISOString(),
        method: "WEBSOCKET",
        url,
        resourceType: "websocket",
        postDataPreview: null,
        handleHints: null,
        handlePairs: [],
        transportHints,
      });
    });

    context.on("request", onRequest);
    try {
      responseCapture = createResponseCapture(page, {
        requestFilter, onResponse, onWebSocket, onError: fail, bodyBytes: captureBudgetLimits?.inputBytes,
      });
    } catch (error) { fail(error); throw error; }

    function stop() {
      if (stopping) return stopping;
      stopped = true;
      stopping = (async () => {
        try { context.off("request", onRequest); }
        finally { await responseCapture?.stop(); }
      })();
      return stopping;
    }

    async function adoptPage(nextPage) {
      budget.assertHealthy();
      if (stopped || nextPage.context() !== context) throw new Error("Cannot adopt a page outside the active capture context.");
      await responseCapture.waitForTargets();
      budget.assertHealthy();
      page = nextPage;
    }

    return { state, stop, adoptPage, ready: responseCapture.ready, assertHealthy: budget.assertHealthy };
  }

  return { startNetworkCapture };
}
