import { normalizeString, normalizeObject } from "./gemini-canvas-browser-pool-input.mjs";
import { BROWSER_POOL_TEXT_BODY_LIMIT_BYTES } from "./gemini-canvas-browser-pool-body.mjs";

export function createPreviewOwner({ installCanvasProxyPreviewAuthIndexBridge, inferGoogleAuthUser }) {
  async function tryOpenCanvasProxyPreview(page, timeoutMs) {
    const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    const bridge = await installCanvasProxyPreviewAuthIndexBridge(
      page,
      inferGoogleAuthUser(page.url()),
    ).catch(() => null);
    let stampedFrames = [];

    const candidates = [
      page.getByRole("button", { name: /预览(?:应用)?|Preview(?: app)?|运行|Run/i }).first(),
      page.getByRole("tab", { name: /预览(?:应用)?|Preview(?: app)?|运行|Run/i }).first(),
      page
        .locator(
          'button[aria-label*="预览"], button[aria-label*="Preview"], button[aria-label*="运行"], button[aria-label*="Run"]',
        )
        .first(),
      page
        .locator("button,[role=\"button\"],[role=\"tab\"]")
        .filter({ hasText: /预览(?:应用)?|Preview(?: app)?|运行|Run/i })
        .first(),
    ];

    for (const candidate of candidates) {
      try {
        if ((await candidate.count()) === 0) {
          continue;
        }
        await candidate.waitFor({ state: "visible", timeout: 4_000 });
        await candidate.click({ timeout: Math.min(timeoutMs, 12_000), force: true });
        const previewDeadline = Date.now() + Math.min(timeoutMs, 20_000);
        while (Date.now() < previewDeadline) {
          await page.waitForTimeout(1200);
          const bridgeEventCount = await page
            .evaluate(() => window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__?.events?.length ?? 0)
            .catch(() => 0);
          const iframeCount = await page.locator("iframe").count().catch(() => 0);
          if (iframeCount > 0 && bridgeEventCount <= 1) {
            stampedFrames = await stampCanvasProxyPreviewFrames(
              page,
              inferGoogleAuthUser(page.url()),
            ).catch(() => stampedFrames);
          }
          const bodyPreview = await page
            .evaluate(() => (document.body?.innerText ?? "").slice(0, 1200))
            .catch(() => "");
          if (
            bridgeEventCount > 1 ||
            /显示控制台|System Logs Output|Connecting\.\.\.|Connected|Disconnected|出了点问题|修正错误/i.test(bodyPreview)
          ) {
            break;
          }
        }
        const afterBodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
        const bridgeEvents = await page
          .evaluate(() => window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__?.events ?? [])
          .catch(() => []);
        const iframeNodes = await page
          .evaluate(() =>
            Array.from(document.querySelectorAll("iframe")).map((node, index) => ({
              index,
              id: node.id || null,
              name: node.getAttribute("name") || null,
              title: node.getAttribute("title") || null,
              src: node.getAttribute("src") || null,
            })),
          )
          .catch(() => []);
        const frameUrls = page.frames().map((frame, index) => ({
          index,
          name: frame.name() || null,
          url: frame.url(),
        }));
        return {
          clicked: true,
          reason: "preview_clicked",
          pageUrl: page.url(),
          bodyPreview: String(afterBodyText || "").slice(0, 600),
          bridge,
          bridgeEvents: Array.isArray(bridgeEvents) ? bridgeEvents.slice(-12) : [],
          stampedFrames,
          iframeNodes,
          frameUrls,
        };
      } catch {
        // try next candidate
      }
    }

    return {
      clicked: false,
      reason: "preview_button_not_clickable",
      pageUrl: page.url(),
      bodyPreview: String(bodyText || "").slice(0, 600),
      bridge,
      stampedFrames,
    };
  }

  async function stampCanvasProxyPreviewFrames(page, authIndex = "0", previewFetchRequest = null) {
    const numericAuthIndex = /^\d+$/.test(String(authIndex ?? "").trim())
      ? Number(String(authIndex).trim())
      : 0;
    const normalizedPreviewFetchRequest =
      previewFetchRequest && typeof previewFetchRequest === "object" && normalizeString(previewFetchRequest.url)
        ? {
            url: normalizeString(previewFetchRequest.url),
            method: normalizeString(previewFetchRequest.method)?.toUpperCase() ?? "POST",
            headers: normalizeObject(previewFetchRequest.headers),
            bodyText:
              typeof previewFetchRequest.bodyText === "string" ? previewFetchRequest.bodyText : null,
          }
        : null;
    const results = [];
    for (const [index, frame] of page.frames().entries()) {
      const frameUrl = frame.url();
      if (index === 0) {
        continue;
      }
      if (!/scf\.usercontent\.goog|^blob:/i.test(String(frameUrl || ""))) {
        continue;
      }
      try {
          const stamped = await frame.evaluate(({ authValue, previewFetchRequest, maxTextBytes }) => {
          window.chrome = window.chrome || {};
          window.chrome._contextId = authValue;
          if (!window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__) {
            const NativeWebSocket = window.WebSocket;
            if (typeof NativeWebSocket === "function") {
              const WrappedWebSocket = function(url, protocols) {
                const source = typeof url === "string" ? url : String(url ?? "");
                const rewritten = /^ws:\/\/127\.0\.0\.1:9998(?:\/|\?|$)/i.test(source)
                  ? source.replace(/^ws:/i, "wss:")
                  : source;
                return protocols === undefined
                  ? new NativeWebSocket(rewritten)
                  : new NativeWebSocket(rewritten, protocols);
              };
              WrappedWebSocket.prototype = NativeWebSocket.prototype;
              Object.setPrototypeOf(WrappedWebSocket, NativeWebSocket);
              window.WebSocket = WrappedWebSocket;
              window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__ = true;
            }
          }
          if (
            typeof ConnectionManager === "function" &&
            !window.__NEURO_CANVAS_PROXY_CONNECTION_MANAGER_PATCHED__
          ) {
            const originalEstablish = ConnectionManager.prototype.establish;
            ConnectionManager.prototype.establish = async function(...args) {
              if (typeof this.endpoint === "string" && /^ws:\/\/127\.0\.0\.1:9998(?:\/|$|\?)/i.test(this.endpoint)) {
                this.endpoint = this.endpoint.replace(/^ws:/i, "wss:");
              }
              return await originalEstablish.apply(this, args);
            };
            window.__NEURO_CANVAS_PROXY_CONNECTION_MANAGER_PATCHED__ = true;
          }
          try {
            window.postMessage({ type: "authIndexResponse", authIndex: authValue }, "*");
          } catch {
            // ignore self-post failures
          }
          if (
            typeof initializeProxySystem === "function" &&
            !window.__NEURO_CANVAS_PROXY_FORCE_REINITIALIZED__
          ) {
            window.__NEURO_CANVAS_PROXY_FORCE_REINITIALIZED__ = true;
            try {
              initializeProxySystem();
            } catch {
              // ignore reinit failures; page logs will capture them
            }
          }
          let probeFetch = null;
          let timer = null;
          const bodyLimitError = (actual = null) => Object.assign(
            new Error(`Gemini Canvas browser preview text body exceeded ${maxTextBytes} bytes${Number.isSafeInteger(actual) ? ` (${actual} bytes)` : ""}.`),
            { status: 413, code: "gemini_canvas_browser_body_too_large" },
          );
          const bodySizeUnknownError = () => Object.assign(
            new Error("Gemini Canvas browser preview response has no declared size; refusing a whole-body read."),
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
          const readResponseText = async (response) => {
            if (response.body === null || [204, 205, 304].includes(response.status)) return "";
            const header = response.headers?.get?.("content-length");
            const declared = /^\d+$/.test(String(header ?? "").trim()) ? Number(header) : null;
            if (Number.isSafeInteger(declared) && declared > maxTextBytes) throw bodyLimitError(declared);
            const reader = response.body?.getReader?.();
            if (!reader) {
              if (!Number.isSafeInteger(declared)) throw bodySizeUnknownError();
              const text = await response.text();
              if (utf8Length(text) > maxTextBytes) throw bodyLimitError(utf8Length(text));
              return text;
            }
            const chunks = [];
            let total = 0;
            try {
              while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                const chunk = value instanceof Uint8Array ? value : new Uint8Array(value);
                total += chunk.byteLength;
                if (total > maxTextBytes) {
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
            return new TextDecoder().decode(bytes);
          };
          try {
            const controller = new AbortController();
            timer = setTimeout(() => controller.abort(), 15000);
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
            const defaultProbeRequest = {
              url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key=",
              method: "POST",
              headers: { "content-type": "application/json" },
              bodyText: JSON.stringify({
                contents: [
                  {
                    role: "user",
                    parts: [{ text: "Reply with exactly: ok" }],
                  },
                ],
              }),
            };
            const effectiveProbeRequest = previewFetchRequest?.url
              ? previewFetchRequest
              : defaultProbeRequest;
            let requestUrl = String(effectiveProbeRequest.url || defaultProbeRequest.url);
            try {
              const parsed = new URL(requestUrl);
              if (
                /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
                !parsed.searchParams.has("key")
              ) {
                parsed.searchParams.set("key", "");
                requestUrl = parsed.toString();
              }
            } catch {
              // keep original request URL
            }
            probeFetch = fetch(
              requestUrl,
              {
                method: String(effectiveProbeRequest.method || "POST"),
                headers: sanitizeHeaders(
                  effectiveProbeRequest.headers && typeof effectiveProbeRequest.headers === "object"
                    ? effectiveProbeRequest.headers
                    : defaultProbeRequest.headers,
                ),
                body:
                  typeof effectiveProbeRequest.bodyText === "string"
                    ? effectiveProbeRequest.bodyText
                    : defaultProbeRequest.bodyText,
                credentials: "include",
                mode: "cors",
                signal: controller.signal,
              },
            )
              .then(async (response) => {
                clearTimeout(timer);
                const responseText = await readResponseText(response);
                return {
                  ok: response.ok,
                  status: response.status,
                  url: response.url,
                  contentType: response.headers.get("content-type"),
                  bodyText: responseText,
                  bodyPreview: responseText.slice(0, 800),
                };
              })
              .catch((error) => ({
                ok: false,
                errorMessage: error instanceof Error ? error.message : String(error),
              }));
          } catch (error) {
            probeFetch = Promise.resolve({
              ok: false,
              errorMessage: error instanceof Error ? error.message : String(error),
            });
          }
          return Promise.resolve(probeFetch).finally(() => clearTimeout(timer)).then((probeFetchResult) => ({
            href: location.href,
            readyState: document.readyState,
            bodyPreview: (document.body?.innerText ?? "").slice(0, 400),
            chromeContextId: window.chrome?._contextId ?? null,
            hasAuthIndexReady: typeof window.__authIndexReady !== "undefined",
            hasInitializeProxySystem: typeof initializeProxySystem === "function",
            wsRewriteInstalled: window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__ === true,
            connectionManagerPatched: window.__NEURO_CANVAS_PROXY_CONNECTION_MANAGER_PATCHED__ === true,
            probeFetchResult,
          }));
        }, {
          authValue: numericAuthIndex,
          previewFetchRequest: normalizedPreviewFetchRequest,
          maxTextBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
        });
        results.push({
          index,
          url: frameUrl,
          stamped: true,
          ...stamped,
        });
      } catch (error) {
        results.push({
          index,
          url: frameUrl,
          stamped: false,
          errorMessage: error instanceof Error ? error.message : String(error),
        });
      }
    }
    return results;
  }

  return { tryOpenCanvasProxyPreview, stampCanvasProxyPreviewFrames };
}
