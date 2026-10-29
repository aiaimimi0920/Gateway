// Owns one capture lifetime and at most one compatibility generation; never logs credentials.
import {
  buildLegacyValidationRequest, isLegacyBootstrapSource,
  isProcessSessionValidationRequest, legacyResponseContainsParis, VALIDATION_BODY_LIMIT,
} from "./gemini-web-capture-compat.mjs";

async function validateLegacy(context, source, options) {
  const request = buildLegacyValidationRequest({
    url: source.url(), postData: source.postData(), headers: await source.allHeaders(),
  }, options.prompt);
  const cookies = await context.cookies("https://gemini.google.com/");
  const runtime = options.extractRuntime({
    requestUrl: request.url, postData: new URLSearchParams(request.form).toString(),
    headers: request.headers, cookies, currentUrl: request.headers.referer || options.targetUrl,
  });
  // Validate the exact bounded Google cookie material to be stored through secret patches.
  const cookie = runtime.cookieHeader;
  let response;
  try {
    response = await context.request.post(request.url, {
      headers: { ...request.headers, cookie }, form: request.form,
      timeout: 60_000, maxRedirects: 0, maxRetries: 0,
    });
    const body = await response.body();
    if (body.length > VALIDATION_BODY_LIMIT
      || !legacyResponseContainsParis(response.status(), body.toString("utf8"))) {
      throw new Error(`Gemini Web HTTP validation did not return a Paris assistant answer (HTTP ${response.status()}).`);
    }
    return { ...runtime, responseStatus: response.status(), responseContainsParis: true,
      captureMode: "legacy_http_validation" };
  } catch (error) {
    if (error.message.startsWith("Gemini Web HTTP validation")) throw error;
    // Playwright errors may contain query/session data; do not propagate their request log.
    const reason = /timeout|timed out/i.test(error.message) ? "timeout"
      : /closed|disposed/i.test(error.message) ? "closed" : "transport";
    throw new Error(`Gemini Web HTTP compatibility validation failed (${reason}); refresh the saved login session.`);
  } finally { await response?.dispose(); }
}

export function waitForGeminiWebRuntime(context, options) {
  return new Promise((resolve, reject) => {
    let settled = false;
    let validationStarted = false;
    let latestSource = null;
    const finish = (error, runtime) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      context.off("response", onResponse);
      context.off("close", onClose);
      if (error) reject(error); else resolve(runtime);
    };
    const validate = async () => {
      if (validationStarted || settled) return;
      validationStarted = true;
      if (!latestSource) {
        finish(new Error("Gemini Web uses processSession, but no authenticated legacy bootstrap was captured."));
        return;
      }
      try { finish(null, await validateLegacy(context, latestSource, options)); }
      catch (error) { finish(error); }
    };
    const onClose = () => finish(new Error("Gemini Web login window closed before HTTP validation completed."));
    const onResponse = async (response) => {
      const request = response.request();
      if (response.status() === 200 && isLegacyBootstrapSource(request.url(), request.postData())) {
        latestSource = request;
        // Explicit diagnostic recovery may restore a saved login, but must prove replay afresh.
        if (options.resumeValidation) void validate();
      }
      if (isProcessSessionValidationRequest(request.url(), request.postData(), options.prompt)) {
        void validate();
        return;
      }
      if (!options.isLegacyRequest(request.url()) || !options.containsPrompt(request.postData())) return;
      if (validationStarted || settled) return;
      validationStarted = true;
      try {
        const [headers, cookies, body] = await Promise.all([
          request.allHeaders(), context.cookies("https://gemini.google.com/"), response.body(),
        ]);
        if (body.length > VALIDATION_BODY_LIMIT) throw new Error("Gemini Web validation response exceeded its limit.");
        const runtime = options.extractRuntime({ requestUrl: request.url(), postData: request.postData(),
          headers, cookies, currentUrl: response.frame()?.url() || options.targetUrl });
        finish(null, { ...runtime, responseStatus: response.status(),
          responseContainsParis: legacyResponseContainsParis(response.status(), body.toString("utf8")) });
      } catch { finish(new Error("Gemini Web validation capture failed; no credentials were imported.")); }
    };
    const timeout = setTimeout(() => finish(new Error(
      "Timed out waiting for Gemini Web login and a validated HTTP reply; no credentials were imported.",
    )), options.timeoutMs);
    context.on("response", onResponse);
    context.on("close", onClose);
  });
}
