// Own the browser-context fetch adapter so request orchestration stays focused
// on page ownership, fallback selection, and capture cleanup.

export function runFetchPageRequest(page, input) {
  return page.evaluate(
    async ({
      url,
      method,
      headers,
      bodyText,
      timeoutMs,
      referrer,
      referrerPolicy,
      useCanvasProxyMode,
      maxBodyBytes,
      maxTextBytes,
    }) => {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), timeoutMs);
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      const toBase64 = (bytes) => {
        let binary = "";
        const chunkSize = 0x8000;
        for (let index = 0; index < bytes.length; index += chunkSize) {
          const chunk = bytes.subarray(index, index + chunkSize);
          binary += String.fromCharCode(...chunk);
        }
        return btoa(binary);
      };
      const bodyLimitError = (actual = null, kind = "response", limit = maxBodyBytes) =>
        Object.assign(
          new Error(
            `Gemini Canvas browser ${kind} body exceeded ${limit} bytes${
              Number.isSafeInteger(actual) ? ` (${actual} bytes)` : ""
            }.`,
          ),
          { status: 413, code: "gemini_canvas_browser_body_too_large" },
        );
      const bodySizeUnknownError = (kind, limit) => Object.assign(
        new Error(`Gemini Canvas browser ${kind} body has no declared size; refusing a whole-body read.`),
        { status: 502, code: "gemini_canvas_browser_body_size_unknown", limit },
      );
      const utf8Length = (value) => {
        let bytes = 0;
        for (const character of value) {
          const code = character.codePointAt(0);
          bytes += code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
        }
        return bytes;
      };
      const readBody = async (response, limit, kind) => {
        if (response.body === null || [204, 205, 304].includes(response.status)) return new Uint8Array(0);
        const header = response.headers?.get?.("content-length");
        const declared = /^\d+$/.test(String(header ?? "").trim()) ? Number(header) : null;
        if (Number.isSafeInteger(declared) && declared > limit) {
          throw bodyLimitError(declared, kind, limit);
        }
        const reader = response.body?.getReader?.();
        if (!reader) {
          if (!Number.isSafeInteger(declared)) throw bodySizeUnknownError(kind, limit);
          const bytes = new Uint8Array(await response.arrayBuffer());
          if (bytes.byteLength > limit) throw bodyLimitError(bytes.byteLength, kind, limit);
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
            if (total > limit) {
              await reader.cancel().catch(() => undefined);
              throw bodyLimitError(total, kind, limit);
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
        console.debug?.("[gemini-canvas-fetch] start", url);
        const response = await fetch(url, {
          method,
          headers,
          body: typeof bodyText === "string" ? encoder.encode(bodyText) : undefined,
          credentials: useCanvasProxyMode ? "same-origin" : "include",
          mode: "cors",
          referrer:
            useCanvasProxyMode || !(typeof referrer === "string" && referrer)
              ? undefined
              : referrer,
          referrerPolicy:
            useCanvasProxyMode || !(typeof referrerPolicy === "string" && referrerPolicy)
              ? undefined
              : referrerPolicy,
          signal: controller.signal,
        });
        const responseHeaders = {};
        response.headers.forEach((value, key) => {
          responseHeaders[key] = value;
        });
        const contentType = response.headers.get("content-type");
        const isTextResponse =
          contentType && /(json|text|javascript|xml|html)/i.test(contentType);
        const bodyBytes = await readBody(
          response,
          isTextResponse ? maxTextBytes : maxBodyBytes,
          isTextResponse ? "text" : "response",
        );
        const bodyTextResult =
          isTextResponse
            ? (() => {
                const text = decoder.decode(bodyBytes);
                if (utf8Length(text) > maxTextBytes) {
                  throw bodyLimitError(utf8Length(text), "text", maxTextBytes);
                }
                return text;
              })()
            : null;
        console.debug?.("[gemini-canvas-fetch] done", response.status, response.url);

        return {
          status: response.status,
          ok: response.ok,
          finalUrl: response.url,
          contentType,
          headers: responseHeaders,
          bodyText: bodyTextResult,
          bodyBase64: toBase64(bodyBytes),
        };
      } finally {
        clearTimeout(timeout);
      }
    },
    input,
  );
}
