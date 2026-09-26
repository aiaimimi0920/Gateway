import { previewText } from "./input-text.mjs";
import { createCaptureTasks } from "./capture-tasks.mjs";
import { createCaptureBudget } from "./capture-budget.mjs";
import { requestMatches, createCaptureRequestIdFactory, createResponseRequestAttributor,
  detectAistudioTargetRpcKind } from "./rpc-attribution.mjs";

export function createBrowserCapture({ capture, requestUrlIncludes, responseBodyLimit, persistCapture }) {
  const append = createCaptureBudget();
  const listeners = [];
  const tasks = createCaptureTasks(() => {
    for (const [target, event, listener] of listeners) target.off(event, listener);
    listeners.length = 0;
  });
  const on = (target, event, listener) => {
    if (!tasks.open) return;
    const guarded = (...args) => {
      if (!tasks.open) return;
      try { listener(...args); } catch (error) { tasks.reportFailure(error); }
    };
    listeners.push([target, event, guarded]);
    target.on(event, guarded);
  };
  const nextCaptureRequestId = createCaptureRequestIdFactory();
  const makeRequestEntry = async (request) => {
    let headers = {};
    try {
      headers = await request.allHeaders();
    } catch (_) {}
    return {
      id: nextCaptureRequestId(),
      time: new Date().toISOString(),
      method: request.method(),
      url: request.url(),
      resourceType: request.resourceType(),
      headers,
      postDataPreview: previewText(request.postData(), 12_000),
    };
  };

  const responseRequestAttributor = createResponseRequestAttributor();
  let captureActive = false;
  let firstMatchAt = null;
  let lastMatchAt = null;

  const markMatch = () => {
    const now = Date.now();
    if (!firstMatchAt) {
      firstMatchAt = now;
    }
    lastMatchAt = now;
  };

  function attach(context, page) {
    if (!tasks.open) return;
    on(page, "pageerror", (error) => {
      append(capture.pageErrors, {
        time: new Date().toISOString(),
        message: String(error),
      });
    });

    on(page, "console", (message) => {
      try {
        append(capture.console, {
          time: new Date().toISOString(),
          type: message.type(),
          text: message.text(),
        });
      } catch (error) {
        if (error?.code === "aistudio_probe_capture_budget_exceeded") throw error;
      }
    });

    on(page, "framenavigated", (frame) => {
      append(capture.console, {
        time: new Date().toISOString(),
        type: "frame",
        text: `${frame.url()} <- ${frame.name() || "<unnamed>"}`,
      });
    });

    const handleCapturedRequest = async (request, signal, source = "context") => {
      if (!captureActive) {
        return;
      }
      const url = request.url();
      if (!requestMatches(url, requestUrlIncludes)) {
        return;
      }
      markMatch();
      const entry = await makeRequestEntry(request);
      signal.throwIfAborted();
      entry.source = source;
      entry.frameUrl = request.frame()?.url() ?? null;
      entry.targetRpcKind = detectAistudioTargetRpcKind(url);
      responseRequestAttributor.trackRequest(request, entry.id);
      append(capture.requests, entry);
      await persistCapture();
    };

    const handleCapturedResponse = async (response, signal, source = "context") => {
      if (!captureActive) {
        return;
      }
      const url = response.url();
      if (!requestMatches(url, requestUrlIncludes)) {
        return;
      }
      markMatch();
      const headers = await response.allHeaders().catch(() => ({}));
      signal.throwIfAborted();
      const contentType = headers["content-type"] || "";
      let bodyPreview = null;
      try {
        if (
          contentType.includes("json") ||
          contentType.startsWith("text/") ||
          contentType.includes("javascript") ||
          contentType.includes("xml")
        ) {
          bodyPreview = previewText(await response.text(), responseBodyLimit);
        }
      } catch (error) {
        bodyPreview = `[read-error] ${String(error)}`;
      }
      signal.throwIfAborted();
      append(capture.responses, {
        time: new Date().toISOString(),
        requestId: responseRequestAttributor.resolveResponse(response),
        url,
        source,
        frameUrl: response.request().frame()?.url() ?? null,
        status: response.status(),
        headers,
        bodyPreview,
        targetRpcKind: detectAistudioTargetRpcKind(url),
      });
      await persistCapture();
    };

    on(context, "request", (request) => {
      if (captureActive && requestMatches(request.url(), requestUrlIncludes)) {
        tasks.run((signal) => handleCapturedRequest(request, signal));
      }
    });

    on(context, "response", (response) => {
      if (captureActive && requestMatches(response.url(), requestUrlIncludes)) {
        tasks.run((signal) => handleCapturedResponse(response, signal));
      }
    });

    on(page, "websocket", (ws) => {
      if (!captureActive) {
        return;
      }
      const entry = {
        time: new Date().toISOString(),
        url: ws.url(),
        framesSent: [],
        framesReceived: [],
        closed: false,
        errors: [],
      };
      append(capture.websockets, entry);
      markMatch();
      on(ws, "framesent", (event) => {
        append(entry.framesSent, previewText(event.payload, 4096), entry);
      });
      on(ws, "framereceived", (event) => {
        append(entry.framesReceived, previewText(event.payload, 4096), entry);
      });
      on(ws, "close", () => {
        entry.closed = true;
      });
      on(ws, "socketerror", (error) => {
        append(entry.errors, String(error), entry);
      });
    });

  }
  return {
    attach,
    activate() { tasks.check(); captureActive = true; },
    getMatchTimes() { tasks.check(); return { firstMatchAt, lastMatchAt }; },
    stop: tasks.stop,
  };
}
