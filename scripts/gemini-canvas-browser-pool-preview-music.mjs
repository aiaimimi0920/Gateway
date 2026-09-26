import { executeCanvasNoKeyMusic } from "./gemini-canvas-browser-pool-no-key-music.mjs";

export async function executeCanvasProxyPreviewNoKeyMusic(frame, request, timeoutMs) {
  return await executeCanvasNoKeyMusic(frame, request, timeoutMs, {
    errorNamePrefix: "PreviewMusicNoKey",
    initialErrorMessage: "canvas preview music websocket did not run",
    timeoutErrorMessage: "Canvas preview music websocket timed out.",
    missingSetupMessage: "music preview request is missing setup frame",
  });
}
