// Reuse the native decoded-byte owner. Ordinary Playwright response.text() is not a safe fallback.
import { createProgramResponseCapture } from "../gemini-canvas-program-handle-response-capture.mjs";
import { createError, classifyChallengeLikeBody } from "./pure.mjs";

const BODY_LIMIT = 128 * 1024;
const TOTAL_LIMIT = 1024 * 1024;
const QUEUE_LIMIT = 16;
const failure = (code, message, status = 502) => createError(status, code, message);
export function createSunoResponses(page, baseUrl, createCapture = createProgramResponseCapture, admitted = () => true) {
  const origin = new URL(baseUrl).origin;
  let stopped = false, error = null, waiter = null, capture = null, generationSeen = false, total = 0;
  const queue = [];
  const kindOf = (url) => {
    try {
      const parsed = new URL(url);
      if (parsed.origin !== origin) return null;
      const path = parsed.pathname.replace(/\/+$/, "");
      return path === "/api/generate/v2-web" ? "generate" : path === "/api/feed/v3" ? "feed" : null;
    } catch { return null; }
  };
  const fail = (reason) => {
    error ??= reason;
    waiter?.reject(error); waiter = null;
    queue.length = 0;
    void capture?.stop().catch(() => undefined);
  };
  capture = createCapture(page, {
    pageOnly: true, failOnBodyError: true, bodyBytes: BODY_LIMIT, binaryBodyBytes: BODY_LIMIT,
    requestFilter: (url) => Boolean(kindOf(url)),
    onError(reason) {
      fail(failure(reason?.code === "gateway_program_handle_capture_limit_exceeded"
        ? "suno_browser_body_limit" : "suno_browser_capture_failed", "Suno native response admission failed; details omitted."));
    },
    onResponse(response) {
      if (stopped || error || response.request().method().toUpperCase() !== "POST") return;
      const kind = kindOf(response.url());
      if (!kind) return;
      if (response.isRedirect?.()) { fail(failure("suno_browser_redirect", "Suno response redirected; generation is not replayed.")); return; }
      if (kind === "generate") {
        if (!admitted()) { fail(failure("suno_browser_request_mismatch", "Suno response has no admitted model-bound submission.")); return; }
        if (generationSeen) { fail(failure("suno_browser_ambiguous_generation", "More than one Suno generation was observed; do not resubmit.")); return; }
        generationSeen = true;
      } else if (!generationSeen) return; // Ignore account feed observed before this submission response.
      if (waiter?.kind === kind) {
        const waiting = waiter; waiter = null; waiting.resolve(response);
      } else if (queue.length < QUEUE_LIMIT) queue.push({ kind, response });
      else fail(failure("suno_browser_capture_queue_limit", "Suno response queue exceeded its bound."));
    },
  });
  const ready = capture.ready.catch(() => { throw error ?? failure("suno_browser_capture_failed", "Unable to prepare Suno response capture."); });
  // A failed capture may happen before the worker awaits ready.
  void ready.catch(() => undefined);
  async function read(kind) {
    if (error) throw error;
    if (stopped) throw failure("suno_browser_capture_stopped", "Suno response capture stopped.");
    const index = queue.findIndex((entry) => entry.kind === kind);
    const response = index >= 0 ? queue.splice(index, 1)[0].response : await new Promise((resolve, reject) => {
      if (waiter) { reject(failure("suno_browser_capture_failed", "Concurrent Suno response wait is not permitted.")); return; }
      waiter = { kind, resolve, reject };
    });
    if (error) throw error;
    const status = response.status();
    if (status >= 200 && status < 300 &&
        String(response.headers()["content-type"] ?? "").split(";")[0].trim().toLowerCase() !== "application/json") {
      throw failure("suno_browser_content_type", "Suno response is not JSON; body omitted.");
    }
    if (total >= TOTAL_LIMIT) throw failure("suno_browser_body_limit", "Suno cumulative metadata budget exceeded.");
    const text = await response.text(); // Only the native-admitted, <=128KiB response supplied above.
    if (error) throw error;
    const bytes = Buffer.byteLength(text);
    if (bytes > TOTAL_LIMIT - total) {
      throw failure("suno_browser_body_limit", "Suno cumulative metadata budget exceeded.");
    }
    total += bytes; // Admit before JSON.parse; one native response can be in flight at a time.
    if (status < 200 || status >= 300) {
      throw failure(classifyChallengeLikeBody(text) ? "suno_browser_challenge_required" : "suno_browser_generation_failed",
        `Suno request returned HTTP ${status}; body omitted.`, status >= 400 && status <= 599 ? status : 502);
    }
    try { return JSON.parse(text); }
    catch { throw failure("suno_browser_invalid_generation_json", "Suno response JSON is invalid; body omitted."); }
  }
  return {
    ready,
    fail,
    async next(kind, timeoutMs) {
      let timer;
      try {
        return await Promise.race([read(kind), new Promise((_, reject) => {
          timer = setTimeout(() => {
            const reason = failure("suno_browser_wait_timeout", "Suno response wait timed out; remote generation may still exist.", 504);
            fail(reason); reject(reason);
          }, Math.max(1, timeoutMs));
        })]);
      } finally { clearTimeout(timer); }
    },
    async stop() {
      if (!stopped) {
        stopped = true;
        waiter?.reject(failure("suno_browser_capture_stopped", "Suno response capture stopped."));
        waiter = null; queue.length = 0;
      }
      await capture.stop();
    },
  };
}
