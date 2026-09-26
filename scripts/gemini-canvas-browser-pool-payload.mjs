import { readFileSync } from "node:fs";
import {
  normalizeGeminiBrowserAssetUrl, isAudioLikeMimeType, isAudioLikeUrl, inferMimeTypeFromUrl,
} from "./gemini-canvas-browser-pool-media-urls.mjs";
import {
  assertBytesWithinLimit,
  assertTextWithinLimit,
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
  readPlaywrightResponseBody,
} from "./gemini-canvas-browser-pool-body.mjs";

async function fetchAssetBytes(page, url) {
  return page.evaluate(async ({ url, maxBodyBytes }) => {
    const bodyLimitError = (actual = null) => Object.assign(
      new Error(`Gemini Canvas browser asset body exceeded ${maxBodyBytes} bytes${Number.isSafeInteger(actual) ? ` (${actual} bytes)` : ""}.`),
      { status: 413, code: "gemini_canvas_browser_body_too_large" },
    );
    const bodySizeUnknownError = () => Object.assign(
      new Error("Gemini Canvas browser asset body has no declared size; refusing a whole-body read."),
      { status: 502, code: "gemini_canvas_browser_body_size_unknown" },
    );
    const readBody = async (response) => {
      if (response.body === null || [204, 205, 304].includes(response.status)) return new Uint8Array(0);
      const header = response.headers.get("content-length");
      const declared = /^\d+$/.test(String(header ?? "").trim()) ? Number(header) : null;
      if (Number.isSafeInteger(declared) && declared > maxBodyBytes) throw bodyLimitError(declared);
      const reader = response.body?.getReader?.();
      if (!reader) {
        if (!Number.isSafeInteger(declared)) throw bodySizeUnknownError();
        const bytes = new Uint8Array(await response.arrayBuffer());
        if (bytes.byteLength > maxBodyBytes) throw bodyLimitError(bytes.byteLength);
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
          if (total > maxBodyBytes) {
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
      return bytes;
    };
    const response = await fetch(url, { credentials: "include" });
    const bytes = await readBody(response);
    let binary = "";
    const chunkSize = 0x8000;
    for (let index = 0; index < bytes.length; index += chunkSize) {
      const chunk = bytes.subarray(index, index + chunkSize);
      binary += String.fromCharCode(...chunk);
    }
    return {
      status: response.status,
      ok: response.ok,
      contentType: response.headers.get("content-type"),
      bodyBase64: btoa(binary),
    };
  }, { url, maxBodyBytes: BROWSER_POOL_BINARY_BODY_LIMIT_BYTES });
}

export async function extractAudioBytes(page, asset) {
  if (asset?.bodyBase64) {
    return {
      mimeType: asset.mimeType || "audio/wav",
      bodyBase64: asset.bodyBase64,
    };
  }

  const normalizedUrl = normalizeGeminiBrowserAssetUrl(asset?.url) ?? asset?.url;
  let result;
  try {
    result = await fetchAssetBytes(page, normalizedUrl);
  } catch (error) {
    throw Object.assign(
      new Error("Failed to download Gemini Canvas TTS audio payload from the browser context."),
      {
        status: Number(error?.status ?? 500),
        code: "gemini_canvas_tts_audio_fetch_failed",
        url: normalizedUrl ?? null,
        cause: error instanceof Error ? error.message : String(error),
      },
    );
  }

  if (!result?.ok || !result.bodyBase64) {
    throw Object.assign(new Error("Failed to download Gemini Canvas TTS audio payload from the browser context."), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_tts_audio_fetch_failed",
      url: normalizedUrl ?? null,
    });
  }

  const resolvedMimeType = result.contentType || asset.mimeType || "audio/wav";
  if (!isAudioLikeMimeType(resolvedMimeType) && !isAudioLikeUrl(normalizedUrl)) {
    throw Object.assign(new Error("Gemini Canvas TTS candidate asset was not an audio resource."), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_tts_non_audio_asset",
      mimeType: resolvedMimeType,
      url: normalizedUrl,
    });
  }

  return {
    mimeType: resolvedMimeType,
    bodyBase64: result.bodyBase64,
  };
}

export async function extractImageBytes(page, asset) {
  if (asset?.bodyBase64) {
    return {
      mimeType: asset.mimeType || "image/png",
      bodyBase64: asset.bodyBase64,
    };
  }

  const normalizedUrl = normalizeGeminiBrowserAssetUrl(asset?.url) ?? asset?.url;
  let result;
  try {
    result = await fetchAssetBytes(page, normalizedUrl);
  } catch (error) {
    throw Object.assign(
      new Error("Failed to download Gemini Canvas image payload from the browser context."),
      {
        status: Number(error?.status ?? 500),
        code: "gemini_canvas_image_fetch_failed",
        url: normalizedUrl ?? null,
        cause: error instanceof Error ? error.message : String(error),
      },
    );
  }

  if (!result?.ok || !result.bodyBase64) {
    throw Object.assign(new Error("Failed to download Gemini Canvas image payload from the browser context."), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_image_fetch_failed",
      url: normalizedUrl ?? null,
    });
  }

  const resolvedMimeType = result.contentType || asset.mimeType || "image/png";
  if (!/^image\//i.test(resolvedMimeType)) {
    throw Object.assign(new Error(`Gemini Canvas image candidate asset was not an image resource. url=${normalizedUrl} contentType=${resolvedMimeType}`), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_non_image_asset",
      mimeType: resolvedMimeType,
      url: normalizedUrl,
    });
  }

  return {
    mimeType: resolvedMimeType,
    bodyBase64: result.bodyBase64,
  };
}

export async function downloadBinaryViaNavigation(entry, url, timeoutMs) {
  const page = await entry.context.newPage();
  try {
    const downloadPromise = page
      .waitForEvent("download", { timeout: Math.min(timeoutMs, 30_000) })
      .catch(() => null);
    let response = null;
    let navigationError = null;
    try {
      response = await page.goto(url, {
        waitUntil: "commit",
        timeout: timeoutMs,
      });
    } catch (error) {
      navigationError = error;
    }
    const download = await downloadPromise;
    if (download) {
      const downloadPath = await download.path();
      const bodyBuffer = assertBytesWithinLimit(
        readFileSync(downloadPath),
        BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
        "download",
      );
      const suggestedFilename = download.suggestedFilename();
      return {
        status: 200,
        ok: true,
        finalUrl: url,
        contentType: inferMimeTypeFromUrl(suggestedFilename || url, "application/octet-stream"),
        headers: {
          "content-disposition": suggestedFilename
            ? `attachment; filename="${suggestedFilename.replaceAll('"', "")}"`
            : "attachment",
        },
        bodyText: null,
        bodyBase64: bodyBuffer.toString("base64"),
      };
    }
    if (navigationError) {
      throw navigationError;
    }
    if (!response) {
      throw Object.assign(
        new Error("Gemini Canvas browser navigation download returned no response."),
        {
          status: 599,
          code: "gemini_canvas_navigation_fetch_missing_response",
        },
      );
    }
    const bodyBuffer = Buffer.from(
      await readPlaywrightResponseBody(
        response,
        BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
        "navigation",
      ),
    );
    const responseHeaders = response.headers();
    const contentType = responseHeaders["content-type"] ?? null;
    const bodyText =
      contentType && /(json|text|javascript|xml|html)/i.test(contentType)
        ? assertTextWithinLimit(
            bodyBuffer.toString("utf8"),
            BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
            "navigation text",
          )
        : null;
    return {
      status: response.status(),
      ok: response.ok(),
      finalUrl: response.url(),
      contentType,
      headers: responseHeaders,
      bodyText,
      bodyBase64: bodyBuffer.toString("base64"),
    };
  } finally {
    await page.close().catch(() => undefined);
  }
}
