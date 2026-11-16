// Native decoded-byte capture is installed before the only Create click; no whole-body fallback.
import { createProgramResponseCapture } from "../gemini-canvas-program-handle-response-capture.mjs";
import { normalizeString } from "./request.mjs";
import { boundNativeModel, createNativeModelGate } from "./native-model.mjs";
const error = (code, message, status = 502) => Object.assign(new Error(message), { code, status });
export async function submitNativeGeneration(page, options, waitForButton, createCapture = createProgramResponseCapture) {
  const prompt = normalizeString(options.requestBody?.gen_params?.prompt ?? options.requestBody?.prompt);
  if (!prompt) throw error("udio_browser_missing_prompt", "Udio generation requires a non-empty prompt.", 400);
  const expectedModel = boundNativeModel(options.requestBody);
  const origin = new URL(options.baseUrl).origin;
  const matches = (value) => {
    try { const url = new URL(value); return url.origin === origin && url.pathname.replace(/\/+$/, "") === "/api/generate-proxy"; }
    catch { return false; }
  };
  let capture, gate, failure, settle, rejectResponse, seen = false, timer;
  const response = new Promise((resolve, reject) => { settle = resolve; rejectResponse = reject; });
  void response.catch(() => undefined);
  const fail = (reason) => { failure ??= reason; rejectResponse(failure); };
  const deadline = Date.now() + options.timeoutMs;
  const remaining = () => Math.max(1, deadline - Date.now());
  try {
    gate = createNativeModelGate(page, matches, expectedModel, fail);
    const captureOptions = {
      pageOnly: true, failOnBodyError: true, bodyBytes: 128 * 1024, binaryBodyBytes: 128 * 1024,
      requestFilter: matches,
      onError(reason) { fail(error(reason?.code === "gateway_program_handle_capture_limit_exceeded"
        ? "udio_browser_metadata_limit" : "udio_browser_capture_failed", "Udio native response admission failed; details omitted.")); },
      onResponse(value) {
        if (failure || value.request().method().toUpperCase() !== "POST" || !matches(value.url())) return;
        if (!gate.admitted()) { fail(error("udio_browser_model_mismatch", "Udio response has no admitted model-bound request; body omitted.", 409)); return; }
        if (seen || value.isRedirect?.()) {
          fail(error("udio_browser_ambiguous_generation", "Udio generation response is ambiguous or redirected; do not resubmit.")); return;
        }
        seen = true; settle(value);
      },
    };
    const work = async () => {
      await gate.ready;
      if (failure) throw failure;
      // Do not track a request while our model gate is still being installed. A pre-existing
      // response has no capture request identity and is rejected by the shared owner.
      capture = createCapture(page, captureOptions);
      await capture.ready;
      if (failure) throw failure;
      // Page routing cannot establish a pre-network model guarantee for Service Worker traffic.
      if (await page.evaluate(() => Boolean(navigator.serviceWorker?.controller))) {
        throw error("udio_browser_model_mismatch", "Udio native model binding requires a page not controlled by a Service Worker.", 409);
      }
      if (failure) throw failure;
      const input = page.locator("textarea").first();
      await input.waitFor({ state: "visible", timeout: remaining() });
      if (failure) throw failure;
      await input.fill(prompt, { timeout: remaining() });
      if (failure) throw failure;
      const button = await waitForButton(page, remaining());
      if (failure) throw failure;
      if (Date.now() >= deadline) throw error("udio_browser_wait_timeout", "Udio deadline expired before submission.", 504);
      gate.arm();
      await button.click({ timeout: remaining() }); // Exactly one click, even after a challenge cycle.
      const value = await response;
      if (failure) throw failure;
      const headers = value.headers(), status = value.status();
      const contentType = headers["content-type"] ?? "";
      if (status >= 200 && status < 300 && contentType.split(";")[0].trim().toLowerCase() !== "application/json") {
        throw error("udio_browser_content_type", "Udio response is not JSON; body omitted.");
      }
      const text = await value.text();
      if (failure) throw failure; // Shared native onError can precede empty-body settlement.
      return { ok: status >= 200 && status < 300, status, contentType, mitigated: headers["x-vercel-mitigated"] ?? "", text };
    };
    return await Promise.race([work(), new Promise((_, reject) => {
      timer = setTimeout(() => {
        const reason = error("udio_browser_wait_timeout", "Udio native response timed out; remote generation may still exist.", 504);
        fail(reason); reject(reason);
      }, Math.max(1, options.timeoutMs));
    })]);
  } catch (reason) {
    if (typeof reason?.code === "string" && reason.code.startsWith("udio_")) throw reason;
    throw failure ?? error("udio_browser_capture_failed", "Udio native generation failed; details omitted. Do not resubmit.");
  } finally {
    clearTimeout(timer);
    // Cancels capture ownership, not the page's original request or a remote generation.
    rejectResponse(error("udio_browser_capture_stopped", "Udio capture stopped."));
    try { try { await gate?.stop(); } finally { await capture?.stop(); } }
    catch { throw error("udio_browser_cleanup_failed", "Udio native capture cleanup failed."); }
    if (failure) throw failure; // Cleanup-time ambiguity/admission errors must not become success.
  }
}
