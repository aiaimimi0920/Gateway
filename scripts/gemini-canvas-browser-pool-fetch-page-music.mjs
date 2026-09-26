import { executeCanvasProgramPageNoKeyMusic } from "./gemini-canvas-browser-pool-page-music.mjs";

export function createFetchPageMusicOwner({ buildProgramHandleState }) {
  async function runFetchPageMusicOperation({
    entry,
    args,
    url,
    fetchRequest,
    timeoutMs,
    fixtureBaseUrl,
    capture,
  }) {
    const result = await executeCanvasProgramPageNoKeyMusic(
      entry.page,
      {
        url,
        jsonBody:
          fetchRequest.jsonBody !== undefined
            ? fetchRequest.jsonBody
            : typeof fetchRequest.bodyText === "string"
            ? (() => {
                try {
                  return JSON.parse(fetchRequest.bodyText);
                } catch {
                  return null;
                }
              })()
            : null,
      },
      timeoutMs,
    );
    if (!result.ok && result.errorMessage) {
      throw Object.assign(
        new Error(`Gemini Canvas page-context no-key music websocket failed: ${result.errorName || "Error"} ${result.errorMessage}`),
        {
          status: result.status || 599,
          code: result.code ?? "gemini_canvas_page_music_no_key_fetch_failed",
          bodyText: result.bodyText,
        },
      );
    }
    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        entry.page.url(),
        capture.state,
      ),
      ...result,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  return { runFetchPageMusicOperation };
}
