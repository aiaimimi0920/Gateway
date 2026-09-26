import { readFileSync } from "node:fs";
import path from "node:path";
import {
  assertByteLengthWithinLimit,
  BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
} from "./gemini-canvas-browser-pool-body.mjs";
import { normalizeString, normalizeObject } from "./gemini-canvas-browser-pool-input.mjs";

const CONNECTED_CLIENT_RESPONSE_CHUNK_LIMIT = 1024;

export function createConnectedClientOwner({ log, SCRIPT_DIR, DEFAULT_HOST, DEFAULT_TLS_PORT }) {
  const connectedClients = new Map();
  const pendingConnectedProxyRequests = new Map();
  let connectedClientBootstrapSource = null;

  function geminiCanvasConnectedClientScriptPath() {
    return path.resolve(SCRIPT_DIR, "gemini-canvas-connected-client.js");
  }

  function loadConnectedClientBootstrapSource() {
    if (connectedClientBootstrapSource) {
      return connectedClientBootstrapSource;
    }
    const scriptPath = geminiCanvasConnectedClientScriptPath();
    connectedClientBootstrapSource = readFileSync(scriptPath, "utf8");
    return connectedClientBootstrapSource;
  }

  function normalizeClientLabel(value) {
    return normalizeString(value)?.slice(0, 128) ?? null;
  }

  function geminiCanvasBrowserClientApiKey() {
    return (
      normalizeString(process.env.GEMINI_CANVAS_BROWSER_CLIENT_API_KEY) ??
      normalizeString(process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN) ??
      null
    );
  }

  function nextConnectedRequestId() {
    return `gemini_canvas_req_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
  }

  function connectedProxyRequestTimeoutMs() {
    return Math.max(
      Number(process.env.GEMINI_CANVAS_CONNECTED_REQUEST_TIMEOUT_MS || "120000"),
      10_000,
    );
  }

  function releasePendingConnectedProxyRequest(pending) {
    if (!pending) {
      return;
    }
    clearTimeout(pending.timeoutHandle);
    pendingConnectedProxyRequests.delete(pending.requestId);
    pending.chunks.length = 0;
    pending.bodyBytes = 0;
  }

  function connectedClientChunkLimitError(actual) {
    return Object.assign(
      new Error(`Gemini Canvas connected browser response exceeded ${CONNECTED_CLIENT_RESPONSE_CHUNK_LIMIT} chunks.`),
      {
        status: 413,
        code: "gemini_canvas_connected_client_chunk_limit",
        limit: CONNECTED_CLIENT_RESPONSE_CHUNK_LIMIT,
        actual,
      },
    );
  }

  function listConnectedClients() {
    return [...connectedClients.values()].filter((entry) => entry.authenticated);
  }

  function pickConnectedClient() {
    return listConnectedClients()[0] ?? null;
  }

  function cleanupConnectedClient(connectionId) {
    const entry = connectedClients.get(connectionId);
    if (!entry) {
      return;
    }
    connectedClients.delete(connectionId);
    for (const pending of pendingConnectedProxyRequests.values()) {
      if (pending.connectionId !== connectionId) {
        continue;
      }
      releasePendingConnectedProxyRequest(pending);
      pending.reject(
        Object.assign(new Error("Gemini Canvas connected browser client disconnected."), {
          status: 503,
          code: "gemini_canvas_connected_client_disconnected",
        }),
      );
    }
  }

  function handleConnectedClientMessage(connectionId, rawMessage) {
    const entry = connectedClients.get(connectionId);
    if (!entry) {
      return;
    }
    let rawMessageBytes;
    if (typeof rawMessage === "string") {
      rawMessageBytes = Buffer.byteLength(rawMessage, "utf8");
    } else if (Buffer.isBuffer(rawMessage)) {
      rawMessageBytes = rawMessage.byteLength;
    } else {
      return;
    }
    if (rawMessageBytes > BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES) {
      entry.ws.close(1009, "message_too_large");
      return;
    }
    let payload;
    try {
      const messageText = typeof rawMessage === "string" ? rawMessage : rawMessage.toString("utf8");
      payload = JSON.parse(messageText);
    } catch {
      return;
    }

    if (payload?.event_type === "authenticate") {
      const expectedApiKey = geminiCanvasBrowserClientApiKey();
      const providedApiKey = normalizeString(payload.apiKey);
      const clientLabel = normalizeClientLabel(payload.clientLabel) ?? `gemini-canvas-${connectionId}`;
      const authorized = !expectedApiKey || providedApiKey === expectedApiKey;
      entry.clientLabel = clientLabel;
      entry.authenticated = authorized;
      entry.ws.send(
        JSON.stringify({
          event_type: "auth_ack",
          authorized,
          message: authorized ? "" : "Invalid or missing API key",
        }),
      );
      if (!authorized) {
        entry.ws.close(4001, "invalid_api_key");
        return;
      }
      log("connected browser client authenticated", clientLabel);
      return;
    }

    const requestId = normalizeString(payload?.request_id);
    if (!requestId) {
      return;
    }
    const pending = pendingConnectedProxyRequests.get(requestId);
    if (!pending || !entry.authenticated || pending.connectionId !== connectionId) {
      return;
    }

    switch (payload?.event_type) {
      case "response_headers":
        pending.status = Number(payload.status || 200);
        pending.headers = normalizeObject(payload.headers);
        return;
      case "chunk":
        if (typeof payload.data !== "string") {
          return;
        }
        if (payload.data.length === 0) {
          return;
        }
        if (pending.chunks.length >= CONNECTED_CLIENT_RESPONSE_CHUNK_LIMIT) {
          releasePendingConnectedProxyRequest(pending);
          pending.reject(connectedClientChunkLimitError(CONNECTED_CLIENT_RESPONSE_CHUNK_LIMIT + 1));
          return;
        }
        try {
          pending.bodyBytes = assertByteLengthWithinLimit(
            pending.bodyBytes + Buffer.byteLength(payload.data, "utf8"),
            BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
            "connected client response",
          );
        } catch (error) {
          releasePendingConnectedProxyRequest(pending);
          pending.reject(error);
          return;
        }
        pending.chunks.push(payload.data);
        return;
      case "stream_close": {
        const bodyText = pending.chunks.join("");
        releasePendingConnectedProxyRequest(pending);
        pending.resolve({
          status: pending.status ?? 200,
          headers: pending.headers,
          bodyText,
        });
        return;
      }
      case "error":
        releasePendingConnectedProxyRequest(pending);
        pending.reject(
          Object.assign(new Error(normalizeString(payload.message) ?? "Connected browser client failed."), {
            status: Number(payload.status || 500),
            code: "gemini_canvas_connected_client_error",
            bodyText: normalizeString(payload.message) ?? null,
          }),
        );
        return;
      default:
        return;
    }
  }

  async function dispatchConnectedProxyRequest(requestSpec) {
    const client = pickConnectedClient();
    if (!client) {
      throw Object.assign(new Error("No authenticated Gemini Canvas connected browser client is available."), {
        status: 503,
        code: "gemini_canvas_connected_client_unavailable",
      });
    }

    const requestId = nextConnectedRequestId();
    const requestAttemptId = `${requestId}_attempt_1`;
    return await new Promise((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        const pending = pendingConnectedProxyRequests.get(requestId);
        if (!pending) return;
        releasePendingConnectedProxyRequest(pending);
        pending.reject(
          Object.assign(new Error("Timed out waiting for Gemini Canvas connected browser client response."), {
            status: 504,
            code: "gemini_canvas_connected_client_timeout",
          }),
        );
      }, connectedProxyRequestTimeoutMs());

      pendingConnectedProxyRequests.set(requestId, {
        requestId,
        connectionId: client.connectionId,
        status: null,
        headers: {},
        chunks: [],
        bodyBytes: 0,
        resolve,
        reject,
        timeoutHandle,
      });

      try {
        client.ws.send(
          JSON.stringify({
            event_type: "proxy_request",
            request_id: requestId,
            request_attempt_id: requestAttemptId,
            request_attempt_number: 1,
            streaming_mode: "fake",
            is_generative: true,
            ...requestSpec,
          }),
        );
      } catch (error) {
        releasePendingConnectedProxyRequest(pendingConnectedProxyRequests.get(requestId));
        reject(error);
      }
    });
  }

  async function ensureLoopbackConnectedClient(entry) {
    const host = normalizeString(process.env.GEMINI_CANVAS_BROWSER_HOST) ?? DEFAULT_HOST;
    const tlsPort = Number(process.env.GEMINI_CANVAS_BROWSER_POOL_TLS_PORT || DEFAULT_TLS_PORT);
    const endpoint = `wss://${host}:${tlsPort}/ws`;
    const apiKey = geminiCanvasBrowserClientApiKey() ?? "";
    const clientLabel = `gemini-canvas-loopback-${process.pid}`;

    await entry.page.addScriptTag({
      content: loadConnectedClientBootstrapSource(),
    });

    return await entry.page.evaluate(
      async ({ endpoint, apiKey, clientLabel }) => {
        const client = window.__neuroGeminiCanvasConnectedClient;
        if (!client) {
          throw new Error("Gemini Canvas connected client bootstrap did not install.");
        }
        return await client.connect({
          endpoint,
          apiKey,
          clientLabel,
        });
      },
      {
        endpoint,
        apiKey,
        clientLabel,
      },
    );
  }

  return { connectedClients, nextConnectedRequestId, listConnectedClients, cleanupConnectedClient,
    handleConnectedClientMessage, dispatchConnectedProxyRequest, ensureLoopbackConnectedClient };
}
