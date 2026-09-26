import { previewText } from "./payload.mjs";
import { writeWorkerDebugSnapshot } from "./diagnostics.mjs";
import { ensureAuthenticatedStudioPage } from "./ui-session.mjs";
import { DEFAULT_TIMEOUT_MS } from "./settings.mjs";

export async function fetchInsidePage(context, page, requestSpec, timeoutMs = DEFAULT_TIMEOUT_MS) {
  await ensureAuthenticatedStudioPage(page, Math.min(timeoutMs, 25_000)).catch(() => false);
  await writeWorkerDebugSnapshot(page, "raw_fetch_request", {
    requestSpec: {
      method: requestSpec?.method ?? null,
      url: requestSpec?.url ?? null,
      headers: requestSpec?.headers ?? null,
      bodyLength: typeof requestSpec?.body === "string" ? requestSpec.body.length : null,
      bodyPreview:
        typeof requestSpec?.body === "string"
          ? previewText(requestSpec.body, 400)
          : null,
    },
  }).catch(() => undefined);
  try {
    const response = await context.request.fetch(requestSpec.url, {
      method: requestSpec.method || "POST",
      headers: requestSpec.headers || {},
      data: typeof requestSpec.body === "string" ? requestSpec.body : undefined,
      failOnStatusCode: false,
      timeout: timeoutMs,
    });
    const headers = response.headers();
    const contentType = headers["content-type"] || headers["Content-Type"] || "";
    const buffer = await response.body();
    const isTextLike =
      contentType.includes("json") ||
      contentType.startsWith("text/") ||
      contentType.includes("javascript") ||
      contentType.includes("xml");
    let bodyText = isTextLike ? buffer.toString("utf8") : null;
    if (bodyText && contentType.includes("json")) {
      try {
        bodyText = JSON.stringify(JSON.parse(bodyText));
      } catch (_) {}
    }
    return {
      ok: response.ok(),
      status: response.status(),
      contentType,
      bodyText,
      bodyBase64: isTextLike ? null : buffer.toString("base64"),
      finalUrl: response.url(),
      bodyTextLength: typeof bodyText === "string" ? bodyText.length : null,
      bodyBase64Length: isTextLike ? null : buffer.length,
      transportOwner: "aistudio_browser_context_request",
    };
  } catch (nodeSideError) {
    await writeWorkerDebugSnapshot(page, "raw_fetch_context_request_failed", {
      requestSpecUrl: requestSpec?.url ?? null,
      requestSpecMethod: requestSpec?.method ?? null,
      requestSpecBodyLength:
        typeof requestSpec?.body === "string" ? requestSpec.body.length : null,
      error:
        nodeSideError instanceof Error ? nodeSideError.message : String(nodeSideError),
    }).catch(() => undefined);
  }

  return page.evaluate(async (spec) => {
    const compactJsonText = (value) => {
      if (typeof value !== "string" || !value.trim()) {
        return value;
      }
      try {
        return JSON.stringify(JSON.parse(value));
      } catch (_) {
        return value;
      }
    };
    const response = await fetch(spec.url, {
      method: spec.method || "POST",
      headers: spec.headers || {},
      body: typeof spec.body === "string" ? spec.body : undefined,
      credentials: "include",
    });
    const contentType = response.headers.get("content-type") || "";
    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    const isTextLike =
      contentType.includes("json") ||
      contentType.startsWith("text/") ||
      contentType.includes("javascript") ||
      contentType.includes("xml");
    let bodyText = isTextLike ? new TextDecoder().decode(bytes) : null;
    if (bodyText && contentType.includes("json")) {
      bodyText = compactJsonText(bodyText);
    }
    const bodyBase64 = isTextLike
      ? null
      : btoa(String.fromCharCode(...bytes));
    return {
      ok: response.ok,
      status: response.status,
      contentType,
      bodyText,
      bodyBase64,
      finalUrl: response.url,
      bodyTextLength: typeof bodyText === "string" ? bodyText.length : null,
      bodyBase64Length: typeof bodyBase64 === "string" ? bodyBase64.length : null,
      transportOwner: "aistudio_page_evaluate_fetch",
    };
  }, requestSpec);
}
