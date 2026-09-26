export function decodeEscapedCanvasProxyHtml(rawText) {
  const source = String(rawText || "");
  if (!source) {
    return null;
  }
  let decoded = source;
  for (let iteration = 0; iteration < 4; iteration += 1) {
    const next = decoded
      .replace(/\\\\u003c/g, "<")
      .replace(/\\\\u003e/g, ">")
      .replace(/\\\\u003d/g, "=")
      .replace(/\\\\u0026/g, "&")
      .replace(/\\\\u0027/g, "'")
      .replace(/\\\\u0022/g, "\"")
      .replace(/\\\\u005c/g, "\\")
      .replace(/\\\\n/g, "\n")
      .replace(/\\\\r/g, "\r")
      .replace(/\\\\t/g, "\t")
      .replace(/\\\\\"/g, "\"")
      .replace(/\\"/g, "\"")
      .replace(/\\\\'/g, "'")
      .replace(/\\'/g, "'")
      .replace(/\\\\\//g, "\\/");
    if (next === decoded) {
      break;
    }
    decoded = next;
  }
  const start = decoded.indexOf("<!DOCTYPE html>");
  const htmlStart = start >= 0 ? start : decoded.indexOf("<html");
  const htmlEnd = decoded.indexOf("</html>", htmlStart);
  if (htmlStart < 0 || htmlEnd <= htmlStart) {
    return null;
  }
  return decoded.slice(htmlStart, htmlEnd + "</html>".length);
}

export function injectForcedCanvasProxyAuthIndex(html, authIndex = "0") {
  const source = String(html || "");
  if (!source) {
    return null;
  }
  const numericAuthIndex = /^\d+$/.test(String(authIndex ?? "").trim())
    ? Number(String(authIndex).trim())
    : 0;
  const bootstrapScript =
    `<script>` +
    `(function(){` +
    `window.chrome=window.chrome||{};` +
    `window.chrome._contextId=${numericAuthIndex};` +
    `window.__NEURO_FORCED_AUTH_INDEX__=${numericAuthIndex};` +
    `})();` +
    `</script>`;
  let patched = source
    .replace(
      /window\.__authIndexReady\s*=\s*new\s+Promise\(function\(resolve\)\s*\{\s*resolveAuthIndex\s*=\s*resolve;\s*\}\);/i,
      (matched) =>
        `${matched}\n` +
        `            if (Number.isInteger(window.__NEURO_FORCED_AUTH_INDEX__) && window.__NEURO_FORCED_AUTH_INDEX__ >= 0) {\n` +
        `                if (!window.chrome) window.chrome = {};\n` +
        `                window.chrome._contextId = window.__NEURO_FORCED_AUTH_INDEX__;\n` +
        `                resolveAuthIndex(window.__NEURO_FORCED_AUTH_INDEX__);\n` +
        `            }`,
    )
    .replace(/\bws:\/\/127\.0\.0\.1:9998\b/g, "wss://127.0.0.1:9998")
    .replace(
      /const response = await responsePromise;/m,
      `Logger.output("Response promise awaiting completed");\n` +
        `                const response = await responsePromise;\n` +
        `                Logger.output(\`Response promise resolved: status=\${response?.status ?? "unknown"} hasBody=\${Boolean(response?.body)}\`);`,
    )
    .replace(
      /reader = response\.body\.getReader\(\);/m,
      `if (!response.body) {\n` +
        `                    Logger.output("Response body missing before reader acquisition");\n` +
        `                    throw new Error("Response body missing before reader acquisition");\n` +
        `                }\n` +
        `                reader = response.body.getReader();\n` +
        `                Logger.output("Response reader acquired");`,
    )
    .replace(
      /this\._sendErrorResponse\(error, operationId, requestAttemptId\);/m,
      `Logger.output(\`Sending error response back to relay: \${error?.name || "Error"} \${error?.message || error}\`);\n` +
        `                    this._sendErrorResponse(error, operationId, requestAttemptId);`,
    )
    .replace(
      /const config = \{\s*headers: this\._sanitizeHeaders\(requestSpec\.headers\),\s*method: requestSpec\.method,\s*signal,\s*\};/m,
      `const config = {\n` +
        `                headers: this._sanitizeHeaders(requestSpec.headers),\n` +
        `                method: requestSpec.method,\n` +
        `                signal,\n` +
        `                credentials: "include",\n` +
        `            };`,
    )
    .replace(
      /const response = await fetch\(requestUrl, requestConfig\);/m,
      `Logger.output(\`Fetch start: \${requestUrl}\`);\n` +
        `                    Logger.output(\`Fetch config: method=\${requestConfig.method || "GET"} credentials=\${requestConfig.credentials || "default"} hasBody=\${Boolean(requestConfig.body)}\`);\n` +
        `                    const proxyFetchAbortController = new AbortController();\n` +
        `                    proxyFetchAbortController.signal.addEventListener("abort", () => {\n` +
        `                        const reason = proxyFetchAbortController.signal.reason;\n` +
        `                        const reasonText = reason && typeof reason === "object" && "message" in reason\n` +
        `                            ? String(reason.message)\n` +
        `                            : String(reason || "unknown");\n` +
        `                        Logger.output(\`Fetch abort signaled: \${reasonText}\`);\n` +
        `                    }, { once: true });\n` +
        `                    const forwardAbort = () => proxyFetchAbortController.abort(new DOMException("Proxy fetch aborted", "AbortError"));\n` +
        `                    if (requestConfig.signal) {\n` +
        `                        if (requestConfig.signal.aborted) {\n` +
        `                            forwardAbort();\n` +
        `                        } else {\n` +
        `                            requestConfig.signal.addEventListener("abort", forwardAbort, { once: true });\n` +
        `                        }\n` +
        `                    }\n` +
        `                    const fetchTimeoutId = setTimeout(() => {\n` +
        `                        Logger.output("Fetch timeout reached; aborting request");\n` +
        `                        proxyFetchAbortController.abort(new DOMException("Proxy fetch timeout (30s)", "AbortError"));\n` +
        `                    }, 30000);\n` +
        `                    requestConfig.signal = proxyFetchAbortController.signal;\n` +
        `                    let response;\n` +
        `                    try {\n` +
        `                        response = await fetch(requestUrl, requestConfig);\n` +
        `                    } catch (fetchError) {\n` +
        `                        Logger.output(\`Fetch threw: \${fetchError?.name || "Error"} \${fetchError?.message || fetchError}\`);\n` +
        `                        throw fetchError;\n` +
        `                    } finally {\n` +
        `                        clearTimeout(fetchTimeoutId);\n` +
        `                    }\n` +
        `                    Logger.output(\`Fetch done: \${response.status} \${response.url}\`);\n` +
        `                    Logger.output(\`Fetch body present: \${Boolean(response.body)}\`);`,
    );
  if (/<head[^>]*>/i.test(patched)) {
    patched = patched.replace(/<head[^>]*>/i, (matched) => `${matched}\n${bootstrapScript}`);
  } else {
    patched = `${bootstrapScript}\n${patched}`;
  }
  return patched;
}

export function extractCanvasProxyClientHtmlFromTexts(values) {
  for (const value of values || []) {
    const decoded = decodeEscapedCanvasProxyHtml(value);
    if (decoded && /Browser (?:API )?Proxy Client/i.test(decoded)) {
      return decoded;
    }
  }
  return null;
}
