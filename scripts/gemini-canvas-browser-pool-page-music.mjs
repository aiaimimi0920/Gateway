import { executeCanvasNoKeyMusic } from "./gemini-canvas-browser-pool-no-key-music.mjs";

export async function executeCanvasProgramPageNoKeyMusic(page, request, timeoutMs) {
  return await executeCanvasNoKeyMusic(page, request, timeoutMs, {
    errorNamePrefix: "PageMusicNoKey",
    initialErrorMessage: "canvas program page no-key music websocket did not run",
    timeoutErrorMessage:
      "Canvas program page no-key music websocket timed out.",
    missingSetupMessage: "music page no-key request is missing setup frame",
  });
}
