import {
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
} from "./gemini-canvas-browser-pool-body.mjs";

export async function executeCanvasProxyPreviewNoKeyFetch(frame, request, timeoutMs) {
  return await frame.evaluate(
    async ({ url, method, headers, bodyText, timeoutMs, maxBodyBytes, maxTextBytes, canvasHostIdentity }) => {
      const normalizeString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const appendEmptyKeyIfMissing = (rawUrl) => {
        const normalized = normalizeString(rawUrl);
        if (!normalized) {
          return null;
        }
        try {
          const parsed = new URL(normalized);
          if (
            /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
            !parsed.searchParams.has("key")
          ) {
            parsed.searchParams.set("key", "");
          }
          return parsed.toString();
        } catch {
          return normalized;
        }
      };
      const sanitizeHeaders = (rawHeaders) => {
        const normalized =
          rawHeaders && typeof rawHeaders === "object" ? { ...rawHeaders } : {};
        const forbiddenHeaders = [
          "host",
          "connection",
          "content-length",
          "origin",
          "referer",
          "user-agent",
          "sec-fetch-mode",
          "sec-fetch-site",
          "sec-fetch-dest",
        ];
        for (const header of forbiddenHeaders) {
          delete normalized[header];
          delete normalized[header.toLowerCase()];
          delete normalized[header.toUpperCase()];
        }
        return normalized;
      };
      const requestUrl = appendEmptyKeyIfMissing(url);
      const controller = new AbortController();
      const timer = setTimeout(() => {
        controller.abort(new DOMException("Canvas preview no-key fetch timeout", "AbortError"));
      }, timeoutMs);
      const bodyLimitError = (actual = null, kind = "response", limit = maxBodyBytes) => Object.assign(
        new Error(`Gemini Canvas browser ${kind} body exceeded ${limit} bytes${Number.isSafeInteger(actual) ? ` (${actual} bytes)` : ""}.`),
        { status: 413, code: "gemini_canvas_browser_body_too_large" },
      );
      const bodySizeUnknownError = () => Object.assign(
        new Error("Gemini Canvas browser response body has no declared size; refusing a whole-body read."),
        { status: 502, code: "gemini_canvas_browser_body_size_unknown" },
      );
      const utf8Length = (value) => {
        let bytes = 0;
        for (const character of value) {
          const code = character.codePointAt(0);
          bytes += code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
        }
        return bytes;
      };
      const readBody = async (response) => {
        if (response.body === null || [204, 205, 304].includes(response.status)) return new Uint8Array(0);
        const header = response.headers?.get?.("content-length");
        const declared = /^\d+$/.test(String(header ?? "").trim()) ? Number(header) : null;
        if (Number.isSafeInteger(declared) && declared > maxBodyBytes) throw bodyLimitError(declared);
        const reader = response.body?.getReader?.();
        if (!reader) {
          if (!Number.isSafeInteger(declared)) throw bodySizeUnknownError();
          const bytes = new Uint8Array(await response.arrayBuffer());
          if (bytes.byteLength > maxBodyBytes) throw bodyLimitError(bytes.byteLength);
          return bytes;
        }
        const chunks = [];
        let total = 0;
        try {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            const chunk = value instanceof Uint8Array ? value : new Uint8Array(value);
            total += chunk.byteLength;
            if (total > maxBodyBytes) {
              await reader.cancel().catch(() => undefined);
              throw bodyLimitError(total);
            }
            chunks.push(chunk);
          }
        } finally {
          reader.releaseLock?.();
        }
        const bytes = new Uint8Array(total);
        let offset = 0;
        for (const chunk of chunks) {
          bytes.set(chunk, offset);
          offset += chunk.byteLength;
        }
        return bytes;
      };
      try {
        const hostFetch = canvasHostIdentity ? window.fetch : fetch;
        // Atomic document/wrapper admission: no await may separate this guard from fetch.
        if (canvasHostIdentity && (location.origin !== canvasHostIdentity.origin
          || location.href !== canvasHostIdentity.href
          || !Object.hasOwn(document, canvasHostIdentity.documentTokenKey)
          || document[canvasHostIdentity.documentTokenKey] !== canvasHostIdentity.documentToken
          || !document.querySelector("script[data-bard-client-injected]")
          || typeof hostFetch !== "function"
          || Function.prototype.toString.call(hostFetch).includes("[native code]"))) {
          throw Object.assign(new Error("Canvas preview identity changed; no generation request was sent."),
            { status: 503, code: "gemini_canvas_host_identity_changed" });
        }
        if (canvasHostIdentity) {
          // Google only intercepts its advertised model/operation prefixes. A wrapped
          // fetch alone does not prove this request will avoid the native/API path.
          const operations = { textModelName: ["generateContent", "streamGenerateContent"],
            imageModelName: ["predict", "predictLongRunning"], imageEditModelName: ["generateContent"],
            imageTransformModelName: ["generateContent"], videoModelName: ["predict", "predictLongRunning"],
            ttsModelName: ["generateContent"],
            deprecatedTextModelNames: ["generateContent", "streamGenerateContent"],
            deprecatedImageModelNames: ["predict", "predictLongRunning", "generateContent", "streamGenerateContent"],
            deprecatedGenerateImageModelNames: ["generateContent", "streamGenerateContent"],
            deprecatedImageTransformModelNames: ["generateContent", "streamGenerateContent"] };
          const scripts = document.querySelectorAll("script[data-bard-client-injected]");
          const models = new Map();
          let sourceBytes = 0, validMetadata = true;
          const remember = (field, names) => {
            if (!Array.isArray(names) || names.length > 64
              || names.some(name => typeof name !== "string" || !/^[a-zA-Z0-9._-]{1,120}$/.test(name))) {
              validMetadata = false;
              return;
            }
            const canonical = [...new Set(names)].sort();
            if (!models.has(field)) models.set(field, { signatures: new Set(), names: canonical });
            models.get(field).signatures.add(JSON.stringify(canonical));
          };
          if (scripts.length <= 32) for (const script of scripts) {
            const source = script.textContent ?? "";
            sourceBytes += source.length;
            if (sourceBytes > 512 * 1024) break;
            if (!source.includes("googleLlmBaseApiUrls")) continue;
            for (const match of source.matchAll(/["']?((?:text|image|imageEdit|imageTransform|video|tts)ModelName)["']?\s*:\s*["']([a-zA-Z0-9._-]{1,120})["']/g)) {
              remember(match[1], [match[2]]);
            }
            // These exact alias buckets are also proxied by Google's current wrapper.
            // Read bounded JSON literals only, never evaluate injected source or invent models.
            for (const match of source.matchAll(/["']?(deprecated(?:Text|Image|GenerateImage|ImageTransform)ModelNames)["']?\s*:\s*(\[[^\]]{0,8192}\])/g)) {
              try { remember(match[1], JSON.parse(match[2])); }
              catch { validMetadata = false; }
            }
          }
          const target = new URL(requestUrl);
          const admitted = validMetadata && sourceBytes <= 512 * 1024 && scripts.length <= 32
            && !target.username && !target.password
            && String(method).toUpperCase() === "POST" && target.origin === "https://generativelanguage.googleapis.com"
            && [...models].some(([field, values]) => values.signatures.size === 1
              && values.names.some(model => operations[field].some(operation =>
                target.pathname === `/v1beta/models/${model}:${operation}`)));
          if (!admitted) throw Object.assign(new Error("Canvas host does not advertise this model and operation; no request was sent."),
            { status: 503, code: "gemini_canvas_host_model_unavailable" });
        }
        const response = await hostFetch.call(globalThis, requestUrl, {
          method,
          headers: sanitizeHeaders(headers),
          body: typeof bodyText === "string" ? bodyText : undefined,
          credentials: "include",
          mode: "cors",
          signal: controller.signal,
        });
        const bodyBuffer = await readBody(response);
        const responseHeaders = {};
        response.headers?.forEach?.((value, key) => {
          responseHeaders[key] = value;
        });
        const contentType = response.headers?.get?.("content-type") ?? null;
        const bodyTextResult =
          contentType && /(json|text|javascript|xml|html)/i.test(contentType)
            ? (() => {
                const text = new TextDecoder().decode(bodyBuffer);
                const textBytes = utf8Length(text);
                if (textBytes > maxTextBytes) throw bodyLimitError(textBytes, "text", maxTextBytes);
                return text;
              })()
            : null;
        const toBase64 = (bytes) => {
          let binary = "";
          const chunkSize = 0x8000;
          for (let index = 0; index < bytes.length; index += chunkSize) {
            const chunk = bytes.subarray(index, index + chunkSize);
            binary += String.fromCharCode(...chunk);
          }
          return btoa(binary);
        };
        return {
          status: response.status,
          ok: response.ok,
          finalUrl: response.url || null,
          contentType,
          headers: responseHeaders,
          bodyText: bodyTextResult,
          bodyBase64: toBase64(bodyBuffer),
        };
      } catch (error) {
        const hostAdmissionError = ["gemini_canvas_host_identity_changed", "gemini_canvas_host_model_unavailable"].includes(error?.code);
        return {
          status: hostAdmissionError ? 503 : 599,
          ...(hostAdmissionError ? { errorCode: error.code } : {}),
          ok: false,
          finalUrl: null,
          contentType: null,
          headers: {},
          bodyText: null,
          bodyBase64: null,
          errorMessage: error instanceof Error ? error.message : String(error),
          errorName: error instanceof Error ? error.name : "Error",
        };
      } finally {
        clearTimeout(timer);
      }
    },
    {
      url: request.url,
      method: request.method,
      headers: request.headers,
      bodyText: request.bodyText,
      canvasHostIdentity: request.canvasHostIdentity,
      timeoutMs,
      maxBodyBytes: BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
      maxTextBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
    },
  );
}
