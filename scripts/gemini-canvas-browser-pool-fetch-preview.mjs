import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { executeCanvasProxyPreviewNoKeyFetch } from "./gemini-canvas-browser-pool-no-key-fetch.mjs";
import { executeCanvasProxyPreviewNoKeyMusic } from "./gemini-canvas-browser-pool-preview-music.mjs";

export function createFetchPreviewOwner({
  isFixtureCanvasBaseUrl,
  ensureProgramPage,
  tryOpenCanvasProxyPreview,
  isCanvasProxyPreviewFrameUrl,
  ensureCanvasProxyPreviewFrame,
  buildProgramHandleState,
}) {
  async function runFetchPreviewOperation({
    entry,
    args,
    url,
    method,
    requestedHeaders,
    requestBodyText,
    timeoutMs,
    preferredProgramPageUrl,
    fixtureBaseUrl,
    useCanvasPreviewMusicNoKeyMode,
    fetchRequest,
    capture,
  }) {
    const previewFetchRequest = {
      url,
      method,
      headers: requestedHeaders,
      bodyText: requestBodyText,
    };
    let preview;
    if (preferredProgramPageUrl && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      await ensureProgramPage(
        entry,
        fixtureBaseUrl ?? "https://gemini.google.com",
        preferredProgramPageUrl,
        timeoutMs,
      );
      const previewResult = await tryOpenCanvasProxyPreview(entry.page, timeoutMs);
      preview = {
        page: entry.page,
        frame: entry.page.frames().find((frame, index) => index > 0 && isCanvasProxyPreviewFrameUrl(frame.url())) ?? entry.page,
        preview: previewResult,
        stampedFrames: [],
      };
    } else {
      preview = await ensureCanvasProxyPreviewFrame(
        entry,
        fixtureBaseUrl ?? "https://gemini.google.com",
        normalizeString(args.shareId),
        timeoutMs,
        useCanvasPreviewMusicNoKeyMode ? null : previewFetchRequest,
      );
    }
    const probeFetchResult =
      !useCanvasPreviewMusicNoKeyMode
        ? preview.stampedFrames.find((entry) => entry?.stamped && entry?.probeFetchResult)
            ?.probeFetchResult
        : null
      ?? null;
    const result = probeFetchResult
      ? {
          status: probeFetchResult.status ?? 599,
          ok: probeFetchResult.ok === true,
          finalUrl: normalizeString(probeFetchResult.url) ?? previewFetchRequest.url,
          contentType: normalizeString(probeFetchResult.contentType),
          headers: {},
          bodyText:
            typeof probeFetchResult.bodyText === "string"
              ? probeFetchResult.bodyText
              : normalizeString(probeFetchResult.bodyPreview) ?? null,
          bodyBase64:
            typeof probeFetchResult.bodyText === "string"
              ? Buffer.from(probeFetchResult.bodyText, "utf8").toString("base64")
              : typeof probeFetchResult.bodyPreview === "string"
                ? Buffer.from(probeFetchResult.bodyPreview, "utf8").toString("base64")
              : null,
          errorMessage: probeFetchResult.ok ? null : normalizeString(probeFetchResult.errorMessage),
          errorName: probeFetchResult.ok ? null : "PreviewProbeFetchError",
        }
      : useCanvasPreviewMusicNoKeyMode
      ? await executeCanvasProxyPreviewNoKeyMusic(
          preview.frame,
          {
            url: previewFetchRequest.url,
            jsonBody:
              fetchRequest.jsonBody !== undefined
                ? fetchRequest.jsonBody
                : typeof previewFetchRequest.bodyText === "string"
                ? (() => {
                    try {
                      return JSON.parse(previewFetchRequest.bodyText);
                    } catch {
                      return null;
                    }
                  })()
                : null,
          },
          timeoutMs,
        )
      : await executeCanvasProxyPreviewNoKeyFetch(
          preview.frame,
          previewFetchRequest,
          timeoutMs,
        );
    if (!result.ok && result.errorMessage) {
      const diagnostics = {
        previewFrameUrl: preview.frame?.url?.() ?? null,
        probeResult: result,
        networkEvents: capture.state.events.slice(-40),
        rpcCaptures: capture.state.rpcCaptures.slice(-20),
      };
      throw Object.assign(
        new Error(`Gemini Canvas preview-frame no-key fetch failed: ${result.errorName || "Error"} ${result.errorMessage}`),
        {
          status: result.status || 599,
          code:
            result.code ??
            (useCanvasPreviewMusicNoKeyMode
              ? "gemini_canvas_preview_music_no_key_fetch_failed"
              : "gemini_canvas_preview_no_key_fetch_failed"),
          bodyText:
            typeof result.bodyText === "string" && result.bodyText.trim()
              ? result.bodyText
              : JSON.stringify(diagnostics),
        },
      );
    }
    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        preview.page.url(),
        capture.state,
      ),
      ...result,
      previewFrameUrl: preview.frame.url(),
      previewProbe: preview.stampedFrames,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  return { runFetchPreviewOperation };
}
