// Canvas identity belongs to the Google-injected preview fetch, not a pool-global WS client.
import { randomUUID } from "node:crypto";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { executeCanvasProxyPreviewNoKeyFetch } from "./gemini-canvas-browser-pool-no-key-fetch.mjs";

// One non-enumerable property per document; navigation discards it, even on the same Frame/URL.
const documentTokenKey = randomUUID();

export function canvasPreviewOrigin(rawUrl) {
  try {
    const url = new URL(String(rawUrl).startsWith("blob:") ? String(rawUrl).slice(5) : rawUrl);
    return url.protocol === "https:" && url.hostname.endsWith(".scf.usercontent.goog") ? url.origin : null;
  } catch { return null; }
}

function unavailable(code = "gemini_canvas_host_identity_unavailable", status = 503) {
  return Object.assign(new Error(status === 401
    ? "Gemini Canvas browser session is not signed in; refresh the account browser state."
    : "Gemini Canvas authenticated preview host is unavailable; no generation request was sent."), { status, code });
}

export async function selectCanvasHostFrame(page) {
  for (const frame of page.frames()) {
    const origin = canvasPreviewOrigin(frame.url());
    if (!origin || frame.isDetached()) continue;
    const element = await frame.frameElement().catch(() => null);
    try {
      if (!element || !await element.isVisible()) continue;
      const identity = await frame.evaluate(({ origin, href, key, proposedToken }) => {
        if (location.origin !== origin || location.href !== href
          || !document.querySelector("script[data-bard-client-injected]")
          || typeof window.fetch !== "function"
          || Function.prototype.toString.call(window.fetch).includes("[native code]")) return null;
        if (!Object.hasOwn(document, key)) Object.defineProperty(document, key, { value: proposedToken });
        return { origin, href, documentTokenKey: key, documentToken: document[key] };
      }, { origin, href: frame.url(), key: documentTokenKey, proposedToken: randomUUID() });
      if (identity) return { frame, identity };
    } catch { /* Detached/loading frames are not a fallback execution surface. */ }
    finally { await element?.dispose().catch(() => undefined); }
  }
  return null;
}

export function createCanvasHostFetchOwner({
  ensureSharePage, ensureProgramPage, buildProgramHandleState, log,
  selectHostFrame = selectCanvasHostFrame, executeHostFetch = executeCanvasProxyPreviewNoKeyFetch,
}) {
  async function runCanvasHostFetchOperation({
    entry, args, url, method, requestedHeaders, requestBodyText,
    timeoutMs, preferredProgramPageUrl, fixtureBaseUrl, capture,
  }) {
    const baseUrl = fixtureBaseUrl ?? "https://gemini.google.com";
    const target = new URL(url);
    if (new URL(baseUrl).origin !== "https://gemini.google.com"
      || target.origin !== "https://generativelanguage.googleapis.com") {
      throw Object.assign(new Error("Canvas host fetch only accepts the Google Canvas API origin."),
        { status: 400, code: "gemini_canvas_host_target_rejected" });
    }
    let explicitBodyKey = false;
    try { explicitBodyKey = Boolean(JSON.parse(requestBodyText)?.apiKey); }
    catch { /* Non-JSON bodies cannot contain the wrapper's top-level apiKey switch. */ }
    if (explicitBodyKey
      || [...target.searchParams].some(([name, value]) => /^(?:key|api_key|access_token)$/i.test(name) && value)
      || Object.entries(requestedHeaders ?? {}).some(([name, value]) => value
        && /^(?:authorization|x-goog-api-key|x-api-key)$/i.test(name))) {
      throw Object.assign(new Error("Canvas host uses the selected browser account, not an explicit API identity."),
        { status: 400, code: "gemini_canvas_host_api_identity_forbidden" });
    }
    // Never follow another account's connected client or submit prompts in the chat composer.
    if (preferredProgramPageUrl) {
      if (new URL(preferredProgramPageUrl).origin !== "https://gemini.google.com") {
        throw Object.assign(new Error("Canvas program must belong to the Gemini origin."),
          { status: 400, code: "gemini_canvas_host_target_rejected" });
      }
      await ensureProgramPage(entry, baseUrl, preferredProgramPageUrl, timeoutMs);
    } else {
      await ensureSharePage(entry, baseUrl, normalizeString(args.shareId), timeoutMs);
    }
    const page = entry.page;
    const sourceUrl = page.url();
    if (new URL(sourceUrl).origin !== "https://gemini.google.com") throw unavailable();
    const deadline = Date.now() + Math.min(Math.max(timeoutMs, 1000), 30_000);
    const clicked = new Set();
    let selected;
    while (Date.now() < deadline) {
      if (page.isClosed() || entry.contextClosed || page.url() !== sourceUrl) throw unavailable();
      selected = await selectHostFrame(page);
      if (selected) break;
      // Unlike the legacy preview helper, readiness never sends a probe generation.
      for (const [role, name, key] of [
        ["button", /^(?:继续|Continue)$/i, "continue"],
        ["tab", /^(?:预览(?:应用)?|Preview(?: app)?)$/i, "preview-tab"],
        ["button", /^(?:预览(?:应用)?|Preview(?: app)?)$/i, "preview-button"],
      ]) {
        if (clicked.has(key)) continue;
        const control = page.getByRole(role, { name }).first();
        if (await control.isVisible().catch(() => false)) {
          clicked.add(key);
          await control.click({ timeout: Math.min(timeoutMs, 5000) });
          break;
        }
      }
      await page.waitForTimeout(250);
    }
    if (!selected) {
      const guest = await page.getByRole("button", { name: /^(?:登录|Sign in)$/i }).first()
        .isVisible().catch(() => false);
      throw guest ? unavailable("gemini_canvas_browser_session_required", 401) : unavailable();
    }
    const { frame, identity } = selected;
    if (entry.page !== page || page.url() !== sourceUrl || frame.isDetached()) throw unavailable();
    // The selected frame stays owned by this entry; a detach or fetch error is never replayed elsewhere.
    const result = await executeHostFetch(frame, { url, method, headers: requestedHeaders,
      bodyText: requestBodyText, canvasHostIdentity: identity }, timeoutMs);
    if (result.errorCode === "gemini_canvas_host_identity_changed") throw unavailable(result.errorCode);
    if (result.errorCode === "gemini_canvas_host_model_unavailable") {
      throw Object.assign(new Error("Canvas host does not advertise this model and operation; no generation request was sent."),
        { status: 503, code: result.errorCode });
    }
    if (result.errorMessage) {
      throw Object.assign(new Error("Gemini Canvas preview host request failed; the request was not retried."),
        { status: result.status || 502, code: "gemini_canvas_host_fetch_failed" });
    }
    // The parent can synthesize an empty 401 for an internal RPC error, even
    // while this same account can generate text/media. It is not proof of bad credentials.
    if (result.status === 401 && !result.bodyText?.trim() && !result.bodyBase64) {
      throw Object.assign(new Error("Canvas preview host rejected this operation (host status 401, empty response); the request was not retried."),
        { status: 502, code: "gemini_canvas_host_operation_rejected" });
    }
    log("canvas host fetch completed", JSON.stringify({ status: result.status,
      executionSurface: "canvas_preview_host", hostWrappedFetch: true }));
    return { operation: "fetch", ...buildProgramHandleState(baseUrl, args, sourceUrl, capture.state),
      ...result, executionSurface: "canvas_preview_host", canvasHostFrameOrigin: canvasPreviewOrigin(frame.url()),
      networkEvents: capture.state.events, rpcCaptures: capture.state.rpcCaptures };
  }
  return { runCanvasHostFetchOperation };
}
