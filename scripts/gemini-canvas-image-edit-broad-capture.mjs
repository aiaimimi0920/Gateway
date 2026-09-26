import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import path from "node:path";

const MAX_CAPTURED_RESPONSE_BODY_BYTES = 4 * 1024 * 1024;
const MAX_CAPTURED_BASE64_BODY_CHARS = 4 * Math.ceil(MAX_CAPTURED_RESPONSE_BODY_BYTES / 3);
const MAX_CONCURRENT_CAPTURED_BODY_READS = 8;

function contentLength(headers) {
  const raw = Object.entries(headers ?? {}).find(([name]) => name.toLowerCase() === "content-length")?.[1];
  if (typeof raw === "number") return Number.isSafeInteger(raw) && raw >= 0 ? raw : null;
  if (typeof raw !== "string" || !/^\d+$/.test(raw.trim())) return null;
  const parsed = Number(raw.trim());
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function hasNoResponseBody(method, status) {
  return method?.toUpperCase() === "HEAD" || [204, 205, 304].includes(status);
}

function hasNativeBodySizeAdmission(request) {
  if (request.bodySizeInvalid || request.decodedBodyBytes > MAX_CAPTURED_RESPONSE_BODY_BYTES) return false;
  if (request.hasDecodedBodySize && request.decodedBodyBytes > 0) return true;
  const declaredLength = contentLength(request.responseHeaders);
  return declaredLength !== null && declaredLength <= MAX_CAPTURED_RESPONSE_BODY_BYTES;
}

function boundedCdpBodyText(response) {
  if (typeof response?.body !== "string") return null;
  if (response.base64Encoded === true) {
    if (response.body.length > MAX_CAPTURED_BASE64_BODY_CHARS) return null;
    const body = Buffer.from(response.body, "base64");
    return body.length <= MAX_CAPTURED_RESPONSE_BODY_BYTES ? body.toString("utf8") : null;
  }
  if (response.body.length > MAX_CAPTURED_RESPONSE_BODY_BYTES) return null;
  return Buffer.byteLength(response.body, "utf8") <= MAX_CAPTURED_RESPONSE_BODY_BYTES ? response.body : null;
}

export function createImageEditBroadCapture({ marker, outDir }) {
  const events = [];
  const MAX_EVENTS = 1600;
  let markerTransport = null;
  let markerResponseSeenAt = null;
  let markerRequestSeenAt = null;
  let secondarySignalerPollUrl = null;
  let secondarySignalerPollSeenAt = null;
  let secondarySignalerPollCompletedAt = null;
  const cdpRequests = new Map();
  let activeBodyReads = 0;
  let finalizeUploadCapture = null;
  let stopped = false;
  const listeners = [];

  function listen(emitter, event, handler) {
    const listener = (...args) => {
      if (!stopped) return handler(...args);
    };
    emitter.on(event, listener);
    listeners.push([emitter, event, listener]);
  }

  function stop() {
    if (stopped) return;
    // Invalidate pending callbacks before detaching any listener.
    stopped = true;
    cdpRequests.clear();
    for (const [emitter, event, listener] of listeners) {
      try {
        emitter.off(event, listener);
      } catch {
        // A failed detach must not skip the remaining owned listeners.
      }
    }
    listeners.length = 0;
  }

  function beginBodyRead() {
    if (activeBodyReads >= MAX_CONCURRENT_CAPTURED_BODY_READS) return false;
    activeBodyReads += 1;
    return true;
  }

  function endBodyRead() {
    activeBodyReads -= 1;
  }

  const interestingUrl = (url) => {
    const lowered = url.toLowerCase();
    return (
      lowered.includes("gemini.google.com") ||
      lowered.includes("googleapis.com") ||
      lowered.includes("clients6.google.com") ||
      lowered.includes("googleusercontent.com") ||
      lowered.includes("gstatic.com") ||
      lowered.includes("content.googleapis.com")
    );
  };

  const shouldCaptureResponseBody = (contentType, url) => {
    if (/json|text|javascript|html/i.test(contentType)) {
      return true;
    }
    return /gg-dl|rd-gg-dl|googleusercontent|content\.googleapis/i.test(url);
  };

  const pushEvent = (event) => {
    if (stopped) return;
    events.push(event);
    if (events.length > MAX_EVENTS) {
      events.shift();
    }

    const haystack = `${event.url ?? ""}\n${event.postData ?? ""}\n${event.bodyText ?? ""}\n${event.payload ?? ""}`;
    if (!markerTransport && haystack.includes(marker)) {
      markerTransport = event;
      if (event.kind === "request") {
        markerRequestSeenAt = Date.now();
      }
    }
    if (
      markerTransport &&
      !markerResponseSeenAt &&
      event.kind === "response" &&
      event.url === markerTransport.url
    ) {
      markerResponseSeenAt = Date.now();
    }
    if (
      event.kind === "request" &&
      typeof event.url === "string" &&
      event.url.includes("signaler-pa.clients6.google.com/punctual/multi-watch/channel") &&
      /[?&]AID=(?:[1-9]\d*)\b/i.test(event.url)
    ) {
      secondarySignalerPollUrl = event.url;
      secondarySignalerPollSeenAt = Date.now();
    }
    if (
      secondarySignalerPollUrl &&
      (event.kind === "response" || event.kind === "cdp-response-body") &&
      event.url === secondarySignalerPollUrl
    ) {
      secondarySignalerPollCompletedAt = Date.now();
    }
  };

  function attach(cdp, page) {
    if (stopped) return;
    listen(cdp, "Network.requestWillBeSent", (event) => {
      const requestUrl = event.request?.url ?? "";
      if (!interestingUrl(requestUrl)) {
        return;
      }
      cdpRequests.set(event.requestId, {
        requestId: event.requestId,
        url: requestUrl,
        method: event.request?.method ?? null,
        headers: event.request?.headers ?? null,
        postData: event.request?.postData ?? null,
        responseStatus: null,
        responseHeaders: null,
        decodedBodyBytes: 0,
        hasDecodedBodySize: false,
        bodySizeInvalid: false,
      });
    });

    listen(cdp, "Network.responseReceived", (event) => {
      const request = cdpRequests.get(event.requestId);
      if (!request) return;
      request.responseStatus = event.response?.status ?? null;
      request.responseHeaders = event.response?.headers ?? null;
    });

    listen(cdp, "Network.dataReceived", (event) => {
      const request = cdpRequests.get(event.requestId);
      if (!request || request.bodySizeInvalid) return;
      const bytes = event.dataLength;
      if (!Number.isSafeInteger(bytes) || bytes < 0 || bytes > MAX_CAPTURED_RESPONSE_BODY_BYTES - request.decodedBodyBytes) {
        request.bodySizeInvalid = true;
        return;
      }
      request.decodedBodyBytes += bytes;
      request.hasDecodedBodySize = true;
    });

    listen(cdp, "Network.loadingFailed", (event) => {
      cdpRequests.delete(event.requestId);
    });

    listen(cdp, "Network.loadingFinished", async (event) => {
      const request = cdpRequests.get(event.requestId);
      if (!request) {
        return;
      }
      cdpRequests.delete(event.requestId);
      if (
        !finalizeUploadCapture &&
        typeof request.url === "string" &&
        request.url.includes("push.clients6.google.com/upload/") &&
        typeof request.headers?.["x-goog-upload-command"] === "string" &&
        /upload,\s*finalize/i.test(request.headers["x-goog-upload-command"])
      ) {
        try {
          const postDataResponse = await cdp.send("Network.getRequestPostData", {
            requestId: event.requestId,
          });
          if (stopped) return;
          const postData = postDataResponse?.postData ?? null;
          if (typeof postData === "string" && postData.length > 0) {
            const buffer = Buffer.from(postData, "latin1");
            const sha256 = createHash("sha256").update(buffer).digest("hex");
            const fileName = `upload-finalize-body-${sha256.slice(0, 12)}.bin`;
            const outputPath = path.join(outDir, fileName);
            writeFileSync(outputPath, buffer);
            finalizeUploadCapture = {
              byteLength: buffer.length,
              sha256,
              fileName,
              outputPath,
              uploadUrl: request.url,
              headers: request.headers,
              source: "Network.getRequestPostData",
            };
          }
        } catch {
          // continue with normal body capture
        }
      }
      if (stopped) return;
      if (
        !/StreamGenerate|batchexecute|chooseServer|multi-watch\/channel|refreshCreds|content-push|push\.clients6/i.test(
          request.url,
        )
      ) {
        return;
      }
      const noBody = hasNoResponseBody(request.method, request.responseStatus);
      if (!noBody && (!hasNativeBodySizeAdmission(request) || !beginBodyRead())) return;
      try {
        const response = noBody
          ? { body: "", base64Encoded: false }
          : await cdp.send("Network.getResponseBody", { requestId: event.requestId });
        if (stopped) return;
        const bodyText = boundedCdpBodyText(response);
        if (bodyText === null) return;
        pushEvent({
          kind: "cdp-response-body",
          ts: new Date().toISOString(),
          method: request.method,
          resourceType: "cdp",
          url: request.url,
          status: null,
          headers: request.headers,
          postData: request.postData,
          bodyText: bodyText.slice(0, 120000),
        });
      } catch {
        // Some streaming requests do not expose a retrievable body.
      } finally {
        if (!noBody) endBodyRead();
      }
    });

    listen(page, "request", async (request) => {
      if (!interestingUrl(request.url())) return;
      const method = request.method().toUpperCase();
      const resourceType = request.resourceType();
      if (!["POST", "PUT", "PATCH", "GET"].includes(method)) return;
      if (method === "GET" && !["image", "media", "fetch", "xhr", "document"].includes(resourceType)) {
        return;
      }
      let headers = request.headers();
      try {
        headers = await request.allHeaders();
      } catch {
        if (stopped) return;
        headers = request.headers();
      }
      if (stopped) return;
      let postData = request.postData() ?? null;
      let binaryBodyMeta = null;
      const uploadCommandHeader =
        headers["x-goog-upload-command"] ??
        headers["X-Goog-Upload-Command"] ??
        null;
      if (
        method === "POST" &&
        typeof request.url() === "string" &&
        request.url().includes("push.clients6.google.com/upload/") &&
        typeof uploadCommandHeader === "string" &&
        /upload,\s*finalize/i.test(uploadCommandHeader)
      ) {
        try {
          const bodyBuffer = await request.postDataBuffer();
          if (stopped) return;
          if (bodyBuffer?.length) {
            const sha256 = createHash("sha256").update(bodyBuffer).digest("hex");
            const fileName = `upload-finalize-body-${sha256.slice(0, 12)}.bin`;
            const outputPath = path.join(outDir, fileName);
            writeFileSync(outputPath, bodyBuffer);
            binaryBodyMeta = {
              byteLength: bodyBuffer.length,
              sha256,
              fileName,
              outputPath,
            };
            finalizeUploadCapture = {
              byteLength: bodyBuffer.length,
              sha256,
              fileName,
              outputPath,
              uploadUrl: request.url(),
              headers,
            };
            postData = null;
          }
        } catch {
          // ignore binary capture failures and keep textual metadata only
        }
      }
      if (stopped) return;
      pushEvent({
        kind: "request",
        ts: new Date().toISOString(),
        method,
        resourceType,
        url: request.url(),
        headers,
        postData,
        binaryBodyMeta,
      });
    });

    listen(page, "response", async (response) => {
      const request = response.request();
      if (!interestingUrl(response.url())) return;
      const method = request.method().toUpperCase();
      const resourceType = request.resourceType();
      if (!["POST", "PUT", "PATCH", "GET"].includes(method)) return;
      if (method === "GET" && !["image", "media", "fetch", "xhr", "document"].includes(resourceType)) {
        return;
      }

      let responseHeaders = response.headers();
      try {
        responseHeaders = await response.allHeaders();
      } catch {
        if (stopped) return;
        responseHeaders = response.headers();
      }
      if (stopped) return;
      const contentType = responseHeaders["content-type"] ?? "";
      const event = {
        kind: "response",
        ts: new Date().toISOString(),
        method,
        resourceType,
        url: response.url(),
        status: response.status(),
        contentType,
        headers: responseHeaders,
        bodyText: null,
      };
      if (shouldCaptureResponseBody(contentType, response.url())) {
        try {
          const status = response.status();
          if (hasNoResponseBody(method, status)) {
            event.bodyText = "";
          } else {
            const declaredLength = contentLength(responseHeaders);
            if (declaredLength !== null && declaredLength <= MAX_CAPTURED_RESPONSE_BODY_BYTES && beginBodyRead()) {
              try {
                const bodyText = await response.text();
                if (typeof bodyText === "string" && Buffer.byteLength(bodyText, "utf8") <= MAX_CAPTURED_RESPONSE_BODY_BYTES) {
                  event.bodyText = bodyText;
                }
              } finally {
                endBodyRead();
              }
            }
          }
        } catch {
          event.bodyText = null;
        }
      }
      if (stopped) return;
      pushEvent(event);
    });

    listen(page, "websocket", (websocket) => {
      const base = {
        kind: "websocket",
        ts: new Date().toISOString(),
        url: websocket.url(),
      };
      pushEvent({ ...base, phase: "open" });
      listen(websocket, "framesent", (frame) => {
        pushEvent({
          ...base,
          phase: "framesent",
          payload: typeof frame.payload === "string" ? frame.payload.slice(0, 6000) : String(frame.payload),
        });
      });
      listen(websocket, "framereceived", (frame) => {
        pushEvent({
          ...base,
          phase: "framereceived",
          payload: typeof frame.payload === "string" ? frame.payload.slice(0, 6000) : String(frame.payload),
        });
      });
      listen(websocket, "close", () => {
        pushEvent({ ...base, phase: "close" });
      });
    });
  }

  return {
    attach,
    stop,
    get events() { return events; },
    get markerTransport() { return markerTransport; },
    get markerResponseSeenAt() { return markerResponseSeenAt; },
    get markerRequestSeenAt() { return markerRequestSeenAt; },
    get secondarySignalerPollUrl() { return secondarySignalerPollUrl; },
    get secondarySignalerPollSeenAt() { return secondarySignalerPollSeenAt; },
    get secondarySignalerPollCompletedAt() { return secondarySignalerPollCompletedAt; },
    get finalizeUploadCapture() { return finalizeUploadCapture; },
  };
}
