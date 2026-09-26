export async function installCanvasProxyPreviewAuthIndexBridge(page, authIndex = "0") {
  const normalizedAuthIndex = /^\d+$/.test(String(authIndex ?? "").trim())
    ? String(authIndex).trim()
    : "0";

  const installSource = ({ authIndexValue }) => {
    const installBridge = () => {
      if (window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__) {
        return window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__;
      }
      window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__ = {
        installedAt: Date.now(),
        authIndex: authIndexValue,
        events: [],
      };
      const pushEvent = (event) => {
        try {
          window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.events.push({
            t: Date.now(),
            ...event,
          });
        } catch {
          // ignore event capture failures
        }
      };
      if (!window.chrome) {
        window.chrome = {};
      }
      window.chrome._contextId = Number(authIndexValue);
      pushEvent({ kind: "install", authIndex: authIndexValue });

      if (!window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__) {
        const NativeWebSocket = window.WebSocket;
        if (typeof NativeWebSocket === "function") {
          const rewriteUrl = (value) => {
            const source = typeof value === "string" ? value : String(value ?? "");
            if (!/^ws:\/\/127\.0\.0\.1:9998(?:\/|\?|$)/i.test(source)) {
              return source;
            }
            try {
              const parsed = new URL(source);
              parsed.protocol = "wss:";
              pushEvent({
                kind: "ws_rewrite",
                from: source,
                to: parsed.toString(),
              });
              return parsed.toString();
            } catch {
              const rewritten = source.replace(/^ws:/i, "wss:");
              pushEvent({
                kind: "ws_rewrite",
                from: source,
                to: rewritten,
              });
              return rewritten;
            }
          };
          const WrappedWebSocket = function(url, protocols) {
            const rewrittenUrl = rewriteUrl(url);
            return protocols === undefined
              ? new NativeWebSocket(rewrittenUrl)
              : new NativeWebSocket(rewrittenUrl, protocols);
          };
          WrappedWebSocket.prototype = NativeWebSocket.prototype;
          Object.setPrototypeOf(WrappedWebSocket, NativeWebSocket);
          window.WebSocket = WrappedWebSocket;
          window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__ = true;
        }
      }

      window.addEventListener("message", (event) => {
        const data = event?.data;
        let messagePreview = null;
        try {
          messagePreview =
            typeof data === "string"
              ? data.slice(0, 1200)
              : JSON.stringify(data ?? null).slice(0, 1200);
        } catch {
          messagePreview = "[[message preview unavailable]]";
        }
        pushEvent({
          kind: "message",
          origin: String(event?.origin ?? ""),
          type:
            data && typeof data === "object" && "type" in data
              ? String(data.type)
              : typeof data,
          endpoint:
            data && typeof data === "object" && "endpoint" in data
              ? String(data.endpoint ?? "")
              : null,
          errorMessage:
            data && typeof data === "object" && "message" in data
              ? String(data.message ?? "")
              : null,
          messagePreview,
        });
        if (data && typeof data === "object" && data.type === "requestAuthIndex") {
          try {
            event.source?.postMessage(
              {
                type: "authIndexResponse",
                authIndex: authIndexValue,
              },
              "*",
            );
            pushEvent({
              kind: "reply",
              type: "authIndexResponse",
              authIndex: authIndexValue,
            });
          } catch (error) {
            pushEvent({
              kind: "reply_error",
              errorMessage: error instanceof Error ? error.message : String(error),
            });
          }
        }
      });
      return window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__;
    };

    const bridge = installBridge();
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", installBridge, { once: true });
    }
    return {
      authIndex: String(window.chrome?._contextId ?? authIndexValue),
      installedAt: bridge?.installedAt ?? null,
      eventCount: Array.isArray(bridge?.events) ? bridge.events.length : 0,
    };
  };

  await page
    .addInitScript(installSource, { authIndexValue: normalizedAuthIndex })
    .catch(() => undefined);

  return await page
    .evaluate(installSource, { authIndexValue: normalizedAuthIndex })
    .catch(() => ({
      authIndex: normalizedAuthIndex,
      installedAt: null,
      eventCount: 0,
    }));
}
