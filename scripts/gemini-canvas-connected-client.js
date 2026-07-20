(function () {
  if (window.__neuroGeminiCanvasConnectedClient) {
    return;
  }

  const DEFAULT_ENDPOINT = "wss://127.0.0.1:42322/ws";
  const FORBIDDEN_HEADERS = new Set([
    "host",
    "connection",
    "content-length",
    "origin",
    "referer",
    "user-agent",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-dest",
  ]);

  function normalizeString(value) {
    return typeof value === "string" && value.trim() ? value.trim() : null;
  }

  function normalizeHeaders(headers) {
    const source = headers && typeof headers === "object" ? headers : {};
    return Object.fromEntries(
      Object.entries(source).filter(([key]) => !FORBIDDEN_HEADERS.has(String(key).toLowerCase())),
    );
  }

  function toBase64FromBytes(bytes) {
    let binary = "";
    const chunkSize = 0x8000;
    for (let index = 0; index < bytes.length; index += chunkSize) {
      const chunk = bytes.subarray(index, index + chunkSize);
      binary += String.fromCharCode(...chunk);
    }
    return btoa(binary);
  }

  class ConnectedClient {
    constructor() {
      this.socket = null;
      this.pending = new Map();
      this.config = {
        endpoint: DEFAULT_ENDPOINT,
        apiKey: "",
        clientLabel: "",
      };
    }

    configure(nextConfig) {
      this.config = {
        ...this.config,
        ...(nextConfig && typeof nextConfig === "object" ? nextConfig : {}),
      };
      return { ...this.config };
    }

    async connect(nextConfig) {
      this.configure(nextConfig);
      if (this.socket && this.socket.readyState === WebSocket.OPEN) {
        return { connected: true, reused: true };
      }

      const endpoint = normalizeString(this.config.endpoint) || DEFAULT_ENDPOINT;
      const apiKey = normalizeString(this.config.apiKey) || "";
      const clientLabel =
        normalizeString(this.config.clientLabel) ||
        `gemini-canvas-${new Date().toISOString().slice(0, 10)}`;

      await new Promise((resolve, reject) => {
        const socket = new WebSocket(endpoint);
        let settled = false;
        const settle = (fn, value) => {
          if (settled) {
            return;
          }
          settled = true;
          fn(value);
        };

        socket.addEventListener("open", () => {
          socket.send(
            JSON.stringify({
              event_type: "authenticate",
              apiKey,
              clientLabel,
            }),
          );
        });

        socket.addEventListener("message", (event) => {
          let payload;
          try {
            payload = JSON.parse(event.data);
          } catch {
            return;
          }
          if (payload?.event_type === "auth_ack") {
            if (!payload.authorized) {
              settle(reject, new Error(payload.message || "Gemini Canvas client authentication failed."));
              socket.close();
              return;
            }
            this.socket = socket;
            settle(resolve);
            return;
          }
          this.handleMessage(payload);
        });

        socket.addEventListener("error", () => {
          settle(reject, new Error(`Gemini Canvas client failed to connect to ${endpoint}.`));
        });

        socket.addEventListener("close", () => {
          if (this.socket === socket) {
            this.socket = null;
          }
        });
      });

      return { connected: true, endpoint, clientLabel };
    }

    disconnect() {
      if (this.socket && this.socket.readyState <= WebSocket.OPEN) {
        this.socket.close();
      }
      this.socket = null;
    }

    async handleMessage(message) {
      if (!message || typeof message !== "object") {
        return;
      }
      switch (message.event_type) {
        case "proxy_request":
          await this.handleProxyRequest(message);
          return;
        case "cancel_request":
          this.cancelRequest(message.request_id);
          return;
        default:
          return;
      }
    }

    cancelRequest(requestId) {
      const pending = this.pending.get(requestId);
      if (!pending) {
        return;
      }
      pending.abortController.abort();
      this.pending.delete(requestId);
    }

    async handleProxyRequest(requestSpec) {
      const requestId = normalizeString(requestSpec.request_id);
      if (!requestId) {
        return;
      }
      const abortController = new AbortController();
      this.pending.set(requestId, { abortController });
      const requestAttemptId =
        normalizeString(requestSpec.request_attempt_id) || `${requestId}:attempt:legacy`;

      try {
        const requestUrl = this.constructUrl(requestSpec);
        const requestInit = this.buildRequestInit(requestSpec, abortController.signal);
        const response = await fetch(requestUrl, requestInit);

        this.transmit({
          event_type: "response_headers",
          request_id: requestId,
          request_attempt_id: requestAttemptId,
          status: response.status,
          headers: Object.fromEntries(response.headers.entries()),
        });

        const text = await response.text();
        if (text) {
          this.transmit({
            event_type: "chunk",
            request_id: requestId,
            request_attempt_id: requestAttemptId,
            data: text,
          });
        }

        this.transmit({
          event_type: "stream_close",
          request_id: requestId,
          request_attempt_id: requestAttemptId,
        });
      } catch (error) {
        this.transmit({
          event_type: "error",
          request_id: requestId,
          request_attempt_id: requestAttemptId,
          status: error?.name === "AbortError" ? 499 : Number(error?.status || 500),
          message: error instanceof Error ? error.message : String(error),
        });
      } finally {
        this.pending.delete(requestId);
      }
    }

    constructUrl(requestSpec) {
      const absolute = normalizeString(requestSpec.url);
      if (absolute) {
        return absolute;
      }
      const path = normalizeString(requestSpec.path);
      if (!path) {
        throw new Error("Gemini Canvas connected client requires requestSpec.url or requestSpec.path.");
      }
      if (/^https?:\/\//i.test(path)) {
        return path;
      }
      return `https://generativelanguage.googleapis.com/${path.replace(/^\/+/, "")}`;
    }

    buildRequestInit(requestSpec, signal) {
      const headers = normalizeHeaders(requestSpec.headers);
      const init = {
        method: normalizeString(requestSpec.method) || "POST",
        headers,
        signal,
      };
      if (typeof requestSpec.body === "string") {
        init.body = requestSpec.body;
      }
      const referrer = normalizeString(requestSpec.referrer);
      if (referrer) {
        init.referrer = referrer;
      }
      const referrerPolicy = normalizeString(requestSpec.referrerPolicy);
      if (referrerPolicy) {
        init.referrerPolicy = referrerPolicy;
      }
      return init;
    }

    transmit(payload) {
      if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
        return false;
      }
      this.socket.send(JSON.stringify(payload));
      return true;
    }

    async probeFetch(url, body) {
      const response = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
      const contentType = response.headers.get("content-type");
      const bytes = new Uint8Array(await response.arrayBuffer());
      return {
        status: response.status,
        ok: response.ok,
        contentType,
        bodyText:
          contentType && /(json|text|javascript|xml|html)/i.test(contentType)
            ? new TextDecoder().decode(bytes)
            : null,
        bodyBase64: toBase64FromBytes(bytes),
      };
    }
  }

  window.__neuroGeminiCanvasConnectedClient = new ConnectedClient();
  console.info(
    "[neuro-gemini-canvas] connected client installed. Call window.__neuroGeminiCanvasConnectedClient.connect({...}) to attach.",
  );
})();
