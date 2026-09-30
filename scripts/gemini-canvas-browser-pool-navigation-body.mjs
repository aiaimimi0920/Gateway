import { createReadStream } from "node:fs";
import { createProgramResponseCapture } from "./gemini-canvas-program-handle-response-capture.mjs";
import {
  assertByteLengthWithinLimit, BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
} from "./gemini-canvas-browser-pool-body.mjs";

export async function readNavigationDownload(path) {
  const limit = BROWSER_POOL_BINARY_BODY_LIMIT_BYTES;
  // Read one overflow byte, never the entire untrusted file or a stat-only snapshot.
  const stream = createReadStream(path, { highWaterMark: 64 * 1024, end: limit });
  const chunks = [];
  let total = 0;
  try {
    for await (const chunk of stream) {
      total = assertByteLengthWithinLimit(total + chunk.length, limit, "download");
      chunks.push(chunk);
    }
    return Buffer.concat(chunks, total);
  } finally {
    stream.destroy();
  }
}

export function createNavigationBodyCapture(page, timeoutMs, createCapture = createProgramResponseCapture) {
  let resolve, reject, timer, settled = false;
  const result = new Promise((yes, no) => { resolve = yes; reject = no; });
  // A download can win while the document request aborts; defer reporting until consumed.
  void result.catch(() => undefined);
  const finish = (error, value) => {
    if (settled) return;
    settled = true;
    clearTimeout(timer);
    if (error) reject(error); else resolve(value);
  };
  const capture = createCapture(page, {
    pageOnly: true, mainDocumentOnly: true, failOnBodyError: true,
    bodyBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
    binaryBodyBytes: BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
    requestFilter: () => true,
    onError: (error) => finish(error),
    async onResponse(response) {
      // Only CDP's redirect transition proves another document request follows.
      if (response.isRedirect()) return;
      try { finish(null, await response.body()); } catch (error) { finish(error); }
    },
  });
  timer = setTimeout(() => {
    finish(Object.assign(new Error("Gemini Canvas navigation body capture timed out."), {
      status: 504, code: "gemini_canvas_navigation_body_timeout",
    }));
    void capture.stop().catch(() => undefined);
  }, Math.max(1, Math.min(timeoutMs, 30_000)));
  if (settled) clearTimeout(timer);
  return {
    ready: capture.ready, result,
    async stop() {
      finish(new Error("Gemini Canvas navigation body capture stopped."));
      await capture.stop();
    },
  };
}
