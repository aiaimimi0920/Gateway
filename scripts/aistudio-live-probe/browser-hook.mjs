const addHookScript = `
      (() => {
        if (Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)) {
          return;
        }
        const maxEvents = 200;
        const messagePreview = (message) => {
          let remaining = 64;
          const seen = new WeakSet();
          const project = (value, depth) => {
            if (--remaining < 0) return "[limit]";
            if (typeof value === "string") return value.slice(0, 2048);
            if (value == null || typeof value === "boolean" || typeof value === "number") return value;
            if (typeof value !== "object") return "[" + typeof value + "]";
            if (seen.has(value)) return "[circular]";
            if (depth >= 4) return "[depth]";
            seen.add(value);
            if (Array.isArray(value)) {
              const result = [];
              for (let index = 0; index < Math.min(value.length, 16) && remaining > 0; index++) {
                const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
                result.push(!descriptor ? null : "value" in descriptor
                  ? project(descriptor.value, depth + 1) : "[accessor]");
              }
              seen.delete(value);
              return result;
            }
            const result = Object.create(null);
            let count = 0;
            for (const key in value) {
              if (!Object.prototype.hasOwnProperty.call(value, key)) continue;
              if (count++ >= 16 || remaining <= 0) break;
              const descriptor = Object.getOwnPropertyDescriptor(value, key);
              const entry = descriptor && "value" in descriptor
                ? project(descriptor.value, depth + 1) : "[accessor]";
              result[key.slice(0, 120)] = entry;
            }
            seen.delete(value);
            return result;
          };
          try {
            if (typeof message === "string") return message.slice(0, 2048);
            return JSON.stringify(project(message ?? null, 0)).slice(0, 2048);
          } catch (_) { return "[unavailable]"; }
        };
        const pushEvent = (event) => {
          try {
            const target = window.__AISTUDIO_LIVE_CAPTURE__;
            target.push({
              time: new Date().toISOString(),
              ...event,
            });
            if (target.length > maxEvents) {
              target.splice(0, target.length - maxEvents);
            }
          } catch (_) {}
        };
        window.__AISTUDIO_LIVE_CAPTURE__ = [];

        const originalPostMessage = window.postMessage?.bind(window);
        if (originalPostMessage) {
          window.postMessage = function(message, targetOrigin, transfer) {
            try { pushEvent({
              kind: "window.postMessage",
              targetOrigin: String(targetOrigin ?? "*"),
              messagePreview: messagePreview(message),
            }); } catch (_) {}
            return originalPostMessage(message, targetOrigin, transfer);
          };
        }

        window.addEventListener("message", (event) => {
          const data = event.data;
          if (data && typeof data === "object" && data.type === "requestAuthIndex") {
            try {
              event.source?.postMessage(
                {
                  type: "authIndexResponse",
                  authIndex: globalThis.__AISTUDIO_AUTH_INDEX__ ?? 0,
                },
                "*",
              );
              pushEvent({
                kind: "window.message.reply",
                replyType: "authIndexResponse",
                authIndex: globalThis.__AISTUDIO_AUTH_INDEX__ ?? 0,
              });
            } catch (error) {
              pushEvent({
                kind: "window.message.reply_error",
                error: String(error),
              });
            }
          }
          pushEvent({
            kind: "window.message",
            origin: String(event.origin ?? ""),
            messagePreview: messagePreview(event.data),
          });
        });

        const originalFetch = window.fetch?.bind(window);
        let activePreviews = 0;
        const captureFetchResponse = async (response, method, url) => {
          if (activePreviews >= 8) return;
          activePreviews += 1;
          let reader;
          let timer;
          try {
            reader = response.clone().body?.getReader();
            const bytes = new Uint8Array(4096);
            let length = 0;
            const deadline = new Promise((_, reject) => {
              timer = setTimeout(() => reject(new Error("preview deadline")), 5000);
            });
            while (reader && length < bytes.length) {
              const chunk = await Promise.race([reader.read(), deadline]);
              if (chunk.done) break;
              const part = chunk.value.subarray(0, bytes.length - length);
              bytes.set(part, length);
              length += part.length;
            }
            pushEvent({ kind: "fetch.response", method, url: response.url || url,
              status: response.status, contentType: response.headers.get("content-type"),
              bodyPreview: new TextDecoder().decode(bytes.subarray(0, length)) });
          } catch (_) {
            pushEvent({ kind: "fetch.response", method, url, status: response.status,
              bodyPreview: null });
          } finally {
            clearTimeout(timer);
            // A tee branch cancel may wait for the consumer; never await it here.
            // Keep the admission slot until native cancellation settles.
            try {
              if (reader) {
                void reader.cancel().catch(() => {}).then(() => { activePreviews -= 1; });
              } else { activePreviews -= 1; }
            } catch (_) { activePreviews -= 1; }
            try { reader?.releaseLock(); } catch (_) {}
          }
        };
        if (originalFetch) {
          window.fetch = async (...args) => {
            const [resource, init] = args;
            const method = (init && init.method) || "GET";
            const url = typeof resource === "string" ? resource : resource?.url || String(resource);
            const body =
              typeof init?.body === "string"
                ? init.body.slice(0, 4096)
                : init?.body == null
                  ? null
                  : String(init.body).slice(0, 4096);
            pushEvent({ kind: "fetch.request", method, url, body });
            try {
              const response = await originalFetch(...args);
              void captureFetchResponse(response, method, url);
              return response;
            } catch (error) {
              pushEvent({
                kind: "fetch.error",
                method,
                url,
                error: String(error),
              });
              throw error;
            }
          };
        }

        const OriginalXHR = window.XMLHttpRequest;
        if (OriginalXHR) {
          const open = OriginalXHR.prototype.open;
          const send = OriginalXHR.prototype.send;
          OriginalXHR.prototype.open = function(method, url, ...rest) {
            this.__codexMethod = method;
            this.__codexUrl = url;
            return open.call(this, method, url, ...rest);
          };
          OriginalXHR.prototype.send = function(body) {
            pushEvent({
              kind: "xhr.request",
              method: this.__codexMethod || "GET",
              url: this.__codexUrl || "",
              body:
                typeof body === "string"
                  ? body.slice(0, 4096)
                  : body == null
                    ? null
                    : String(body).slice(0, 4096),
            });
            this.addEventListener("loadend", () => {
              let responseText = null;
              try {
                responseText = String(this.responseText || "").slice(0, 4096);
              } catch (_) {}
              pushEvent({
                kind: "xhr.response",
                method: this.__codexMethod || "GET",
                url: this.__codexUrl || "",
                status: this.status,
                bodyPreview: responseText,
              });
            });
            return send.call(this, body);
          };
        }
      })();
    `;

export { addHookScript };
