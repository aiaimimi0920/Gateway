import { createProgramCdpSessions } from "./gemini-canvas-program-handle-cdp-sessions.mjs";
import { BOUNDED_NATIVE_RESPONSE } from "./gemini-canvas-browser-pool-body.mjs";

function captureError(reason, limit = false) {
  return Object.assign(new Error(`Program-handle response capture ${reason}.`), {
    code: limit ? "gateway_program_handle_capture_limit_exceeded" : "gateway_program_handle_capture_failed",
  });
}

// Track completed decoded bytes before requesting any native response body.
export function createProgramResponseCapture(page, {
  requestFilter, onResponse, onWebSocket, onError, bodyBytes = 4 * 1024 * 1024,
  createSessions = createProgramCdpSessions, binaryBodyBytes = bodyBytes, pageOnly = false,
}) {
  if (!Number.isSafeInteger(bodyBytes) || bodyBytes < 1 || bodyBytes > 4 * 1024 * 1024) {
    throw new TypeError("Response body limit must lower the positive default limit.");
  }
  if (!Number.isSafeInteger(binaryBodyBytes) || binaryBodyBytes < bodyBytes || binaryBodyBytes > 16 * 1024 * 1024) {
    throw new TypeError("Binary response limit must fit the positive 16 MiB budget.");
  }
  const pending = new Set();
  const requestsBySession = new Map();
  let stopped = false, failed = false, sessions = null, nativeReads = 0, nativeEnvelopeBytes = 0;
  const fail = (error) => {
    if (stopped || failed) return;
    failed = true;
    void stop().catch(() => undefined);
    onError(error);
  };
  const settle = (entry, readable = false) => {
    entry.resolve?.(readable);
    entry.resolve = null;
  };
  const readBody = async (entry, completed) => {
    if (!(await completed) || stopped || failed || entry.isDisposed()) return "";
    if (nativeReads >= 8) {
      fail(captureError("exceeded its pending body read limit", true)); return "";
    }
    // Nested CDP messages escape the body repeatedly. Reserve their conservative
    // wire size before asking Chromium to serialize it; this is not a heap cap.
    const envelopeBytes = 1024 + entry.bytes * 8 * 2 ** (entry.session.nestingDepth ?? 0);
    const envelopeLimit = Math.max(4 * 1024 * 1024, binaryBodyBytes) * 8 + 1024;
    if (envelopeBytes > envelopeLimit || envelopeBytes > 2 * envelopeLimit - nativeEnvelopeBytes) {
      fail(captureError("exceeded its protocol envelope limit", true)); return "";
    }
    nativeReads++;
    nativeEnvelopeBytes += envelopeBytes;
    const timer = setTimeout(() => fail(captureError("body read timed out")), 5000);
    try {
      const body = await entry.session.send("Network.getResponseBody", { requestId: entry.requestId });
      if (stopped || failed || entry.isDisposed()) return "";
      const bytes = Buffer.from(body.body, body.base64Encoded ? "base64" : "utf8");
      if (bytes.length > binaryBodyBytes) {
        fail(captureError("exceeded its decoded body limit", true)); return "";
      }
      return bytes;
    } catch { return ""; }
    finally { clearTimeout(timer); nativeReads--; nativeEnvelopeBytes -= envelopeBytes; }
  };
  const publish = (entry, response, empty = false) => {
    entry.response = response;
    const headers = Object.fromEntries(Object.entries(response.headers ?? {}).map(([key, value]) => [key.toLowerCase(), String(value)]));
    const completed = new Promise((resolve) => { entry.resolve = resolve; });
    let text = null, bytes = null;
    const body = () => {
      entry.bodyRequested = true;
      if (entry.bodyRejected) fail(captureError("exceeded its decoded body limit", true));
      return (bytes ??= readBody(entry, completed).then((value) => Buffer.isBuffer(value) ? value : Buffer.alloc(0)));
    };
    const readText = async () => {
      const value = (await body()).toString("utf8");
      if (Buffer.byteLength(value) > bodyBytes) {
        fail(captureError("exceeded its decoded text limit", true)); return "";
      }
      return value;
    };
    const result = onResponse({ url: () => response.url, status: () => response.status,
      request: () => ({ method: () => entry.method, headers: () => entry.headers }), headers: () => headers,
      [BOUNDED_NATIVE_RESPONSE]: true, body, text: () => (text ??= readText()) });
    void Promise.resolve(result).catch(() => fail(captureError("consumer failed")));
    if (empty) settle(entry);
  };
  const guard = (handler) => (event) => {
    if (stopped || failed) return;
    try {
      const result = handler(event);
      if (result?.then) void result.catch(() => fail(captureError("failed")));
    } catch { fail(captureError("failed")); }
  };
  const configure = async (session, signal) => {
    let disposed = false;
    const requests = new Map();
    requestsBySession.set(session, requests);
    const find = (requestId) => {
      const local = requests.get(requestId);
      if (local) return local;
      // OOPIF navigation can move body events from the parent to its child target.
      for (let parent = session.parent; parent; parent = parent.parent) {
        const previous = requestsBySession.get(parent);
        const entry = previous?.get(requestId);
        if (!entry) continue;
        previous.delete(requestId);
        entry.session = session;
        entry.isDisposed = () => disposed;
        requests.set(requestId, entry);
        return entry;
      }
      return null;
    };
    const remove = (requestId) => {
      const entry = requests.get(requestId);
      requests.delete(requestId);
      pending.delete(entry);
      return entry;
    };
    const request = guard((event) => {
      const prior = requests.get(event.requestId);
      if (prior) {
        if (event.redirectResponse && !prior.response) publish(prior, event.redirectResponse, true);
        else settle(prior);
        remove(event.requestId);
      }
      if (stopped || failed) return;
      if (!requestFilter(event.request.url)) return;
      if (typeof event.requestId !== "string" || !event.requestId) {
        fail(captureError("has no request identity")); return;
      }
      if (pending.size >= 128) { fail(captureError("exceeded its pending request limit", true)); return; }
      const entry = { session, requestId: event.requestId, method: event.request.method,
        headers: event.request.headers ?? {}, bytes: 0,
        resolve: null, response: null, isDisposed: () => disposed };
      requests.set(event.requestId, entry);
      pending.add(entry);
    });
    const response = guard((event) => {
      const entry = find(event.requestId);
      if (!entry && requestFilter(event.response.url)) {
        fail(captureError("has no request identity")); return;
      }
      if (entry && !entry.response) {
        const type = Object.entries(event.response.headers ?? {}).find(([key]) => key.toLowerCase() === "content-type")?.[1] ?? "";
        entry.bodyLimit = /^(image\/|audio\/|application\/octet-stream)/i.test(type) ? binaryBodyBytes : bodyBytes;
        publish(entry, event.response);
      }
    });
    const data = guard((event) => {
      const entry = find(event.requestId);
      if (!entry) return;
      if (!Number.isSafeInteger(event.dataLength) || event.dataLength < 0) {
        fail(captureError("has invalid decoded byte metadata")); return;
      }
      if (event.dataLength > (entry.bodyLimit ?? bodyBytes) - entry.bytes) {
        entry.bodyRejected = true;
        if (entry.bodyRequested) fail(captureError("exceeded its decoded body limit", true));
        return;
      }
      entry.bytes += event.dataLength;
    });
    const finished = guard(({ requestId }) => {
      const entry = find(requestId);
      if (!entry) return;
      remove(requestId);
      if (!entry.response) { settle(entry); return; }
      const { status, headers } = entry.response;
      const declared = Object.entries(headers ?? {}).find(([key]) => key.toLowerCase() === "content-length")?.[1];
      const empty = entry.method === "HEAD" || [204, 205, 304].includes(status) || String(declared) === "0";
      if (!entry.bytes) {
        if (!empty) {
          entry.bodyRejected = true;
          if (entry.bodyRequested) fail(captureError("has no decoded body size"));
        }
        settle(entry);
        return;
      }
      settle(entry, true);
    });
    const loadingFailed = guard(({ requestId }) => {
      const entry = find(requestId);
      if (entry) { remove(requestId); settle(entry); }
    });
    const listeners = { "Network.requestWillBeSent": request, "Network.responseReceived": response,
      "Network.dataReceived": data, "Network.loadingFinished": finished, "Network.loadingFailed": loadingFailed,
      "Network.webSocketCreated": guard(({ url }) => onWebSocket?.({ url: () => url })) };
    for (const [event, listener] of Object.entries(listeners)) session.on(event, listener);
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      requestsBySession.delete(session);
      signal?.removeEventListener("abort", dispose);
      for (const [event, listener] of Object.entries(listeners)) session.off(event, listener);
      for (const [id, entry] of requests) {
        settle(entry);
        remove(id);
      }
    };
    signal?.addEventListener("abort", dispose, { once: true });
    if (signal?.aborted) { dispose(); return dispose; }
    try {
      await session.send("Network.enable", { maxResourceBufferSize: binaryBodyBytes, maxTotalBufferSize: binaryBodyBytes,
        maxPostDataSize: 0 });
      return dispose;
    } catch { dispose(); throw captureError("could not initialize"); }
  };
  function stop() {
    stopped = true;
    for (const entry of pending) settle(entry);
    pending.clear();
    requestsBySession.clear();
    return Promise.resolve(sessions?.stop());
  }
  sessions = createSessions(page, { configure, onError: fail, pageOnly });
  if (stopped) void sessions.stop().catch(() => undefined);
  return { ready: sessions.ready, stop, waitForTargets: () => sessions.waitForTargets() };
}
