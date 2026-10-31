import { readNavigationDownload } from "./gemini-canvas-browser-pool-navigation-body.mjs";
import { assertBytesWithinLimit, BROWSER_POOL_BINARY_BODY_LIMIT_BYTES } from "./gemini-canvas-browser-pool-body.mjs";

// Read only a Playwright-owned download, not a caller-supplied path or URL.
export async function downloadWebMusic(page, timeoutMs, readBytes = readNavigationDownload) {
  const timeoutError = () => Object.assign(new Error("Music download timed out; generation was not resubmitted."), {
    status: 504, code: "gemini_web_music_download_timeout",
  });
  const timeout = Math.min(Number(timeoutMs ?? 30_000), 30_000);
  if (!Number.isFinite(timeout) || timeout <= 0) throw timeoutError();
  const deadline = performance.now() + timeout;
  const remaining = () => {
    const budget = deadline - performance.now();
    if (budget <= 0) throw timeoutError();
    return Math.max(1, Math.ceil(budget));
  };
  const button = page.getByRole("button", { name: /^(?:下载音乐作品|Download music)$/i }).last();
  if (!await button.isVisible()) return null;
  let download, timer, listener, finished = false;
  const timedOut = new Promise((_, reject) => {
    timer = setTimeout(() => reject(timeoutError()), remaining());
  });
  // Catch immediately: a failed menu click must not leave an unhandled timeout.
  void timedOut.catch(() => undefined);
  const event = new Promise(resolve => { listener = value => { download = value; resolve(value); }; page.on("download", listener); });
  // All active steps share the result deadline; never open a fresh 30s budget
  // or start the audio click after a slow menu has exhausted the request.
  const withinBudget = async work => {
    const result = await Promise.race([work(remaining()), timedOut]);
    remaining();
    return result;
  };
  try {
    await withinBudget(timeout => button.click({ timeout }));
    const audio = page.getByText(/^(?:纯音频|Audio only)$/i).first();
    await withinBudget(timeout => audio.click({ timeout }));
    download = await withinBudget(() => event);
    const path = await withinBudget(() => download.path());
    if (!path) throw new Error("Music download has no local payload.");
    const bytes = await withinBudget(() => readBytes(path));
    assertBytesWithinLimit(bytes, BROWSER_POOL_BINARY_BODY_LIMIT_BYTES, "music download");
    // Container admission only; real acceptance separately decodes the complete track.
    const id3 = bytes.length >= 10 && bytes.subarray(0, 3).toString("ascii") === "ID3";
    const frame = bytes.length >= 4 && bytes[0] === 0xff && (bytes[1] & 0xe6) === 0xe2;
    if (!id3 && !frame) throw Object.assign(new Error("Music download is not an MP3 audio payload."), {
      status: 502, code: "gemini_web_music_download_invalid_audio",
    });
    finished = true;
    return { kind: "audio", url: download.url(), mimeType: "audio/mpeg", bodyBase64: bytes.toString("base64"), durationSeconds: null };
  } finally {
    clearTimeout(timer);
    page.off("download", listener);
    if (download) {
      if (!finished) await download.cancel().catch(() => undefined);
      await download.delete().catch(() => undefined);
    }
  }
}
