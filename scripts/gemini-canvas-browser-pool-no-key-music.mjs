import {
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_BODY_LIMIT_CODE,
} from "./gemini-canvas-browser-pool-body.mjs";

// Keep the browser callback closure-free so page and frame evaluate can serialize it.
export async function executeCanvasNoKeyMusic(target, request, timeoutMs, config) {
  const requestBody =
    request && typeof request === "object"
      ? request.jsonBody ??
        (typeof request.bodyText === "string"
          ? (() => {
              try {
                return JSON.parse(request.bodyText);
              } catch {
                return null;
              }
            })()
          : null)
      : null;
  return await target.evaluate(
    async ({
      url,
      requestBody,
      timeoutMs,
      errorNamePrefix,
      initialErrorMessage,
      timeoutErrorMessage,
      missingSetupMessage,
      maxAudioBytes,
      bodyLimitCode,
    }) => {
      const normalizeString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const appendEmptyKeyIfMissing = (rawUrl) => {
        const normalized = normalizeString(rawUrl);
        if (!normalized) {
          return null;
        }
        try {
          const parsed = new URL(normalized);
          if (
            /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
            !parsed.searchParams.has("key")
          ) {
            parsed.searchParams.set("key", "");
          }
          return parsed.toString();
        } catch {
          return normalized;
        }
      };
      const decodeBase64Chunk = (raw) => {
        const binary = atob(raw);
        const bytes = new Uint8Array(binary.length);
        for (let index = 0; index < binary.length; index += 1) {
          bytes[index] = binary.charCodeAt(index);
        }
        return bytes;
      };
      const encodedByteLength = (raw) => {
        let encodedLength = 0;
        let padding = 0;
        // Reject over-budget Base64 before atob allocates its decoded string and bytes.
        for (let index = 0; index < raw.length; index += 1) {
          const code = raw.charCodeAt(index);
          if (code === 9 || code === 10 || code === 12 || code === 13 || code === 32) {
            continue;
          }
          encodedLength += 1;
          if (code === 61) {
            padding += 1;
          }
        }
        return Math.max(0, Math.floor((encodedLength * 3) / 4) - padding);
      };
      const encodeBytesToBase64 = (chunks) => {
        let totalLength = 0;
        for (const chunk of chunks) {
          totalLength += chunk.length;
        }
        const merged = new Uint8Array(totalLength);
        let offset = 0;
        for (const chunk of chunks) {
          merged.set(chunk, offset);
          offset += chunk.length;
        }
        let binary = "";
        const chunkSize = 0x8000;
        for (let index = 0; index < merged.length; index += chunkSize) {
          const slice = merged.subarray(index, index + chunkSize);
          binary += String.fromCharCode(...slice);
        }
        return btoa(binary);
      };
      const requestPayload =
        requestBody && typeof requestBody === "object" ? requestBody : {};
      const setupFrame =
        requestPayload.setup && typeof requestPayload.setup === "object"
          ? requestPayload.setup
          : null;
      const clientContentFrame =
        requestPayload.client_content &&
        typeof requestPayload.client_content === "object"
          ? requestPayload.client_content
          : null;
      const musicGenerationConfigFrame =
        requestPayload.music_generation_config &&
        typeof requestPayload.music_generation_config === "object"
          ? requestPayload.music_generation_config
          : null;
      const playbackControlFrame =
        normalizeString(requestPayload.playback_control) ?? "PLAY";
      const candidateUrls = [];
      const requestedUrl = appendEmptyKeyIfMissing(url);
      if (requestedUrl) {
        candidateUrls.push(requestedUrl);
      }
      if (
        requestedUrl &&
        requestedUrl.includes("?key=") &&
        !candidateUrls.includes(requestedUrl.replace(/\?key=$/, ""))
      ) {
        candidateUrls.push(requestedUrl.replace(/\?key=$/, ""));
      }
      const errorName = (suffix) => errorNamePrefix + suffix;
      let lastFailure = {
        ok: false,
        status: 599,
        errorMessage: initialErrorMessage,
        errorName: errorName("Error"),
        events: [],
      };
      for (const requestUrl of candidateUrls) {
        const attemptResult = await new Promise((resolve) => {
          const events = [];
          let settled = false;
          let setupComplete = false;
          let sawAudio = false;
          let audioMimeType = "audio/L16;codec=pcm;rate=48000;channels=2";
          const audioChunks = [];
          let audioByteLength = 0;
          let idleTimer = null;
          let timeoutHandle = null;
          const finish = (value) => {
            if (settled) {
              return;
            }
            settled = true;
            if (idleTimer) {
              clearTimeout(idleTimer);
            }
            if (timeoutHandle) {
              clearTimeout(timeoutHandle);
            }
            try {
              socket.close();
            } catch {
              // ignore close errors
            }
            // Release retained audio buffers before the page operation settles.
            audioChunks.length = 0;
            audioByteLength = 0;
            resolve({
              setupComplete,
              events: events.slice(-20),
              ...value,
            });
          };
          const pushEvent = (value) => {
            events.push(String(value ?? "").slice(0, 1200));
            if (events.length > 40) {
              events.shift();
            }
          };
          const socket = new WebSocket(requestUrl);
          timeoutHandle = setTimeout(() => {
            finish({
              ok: false,
              status: 599,
              errorName: errorName("Timeout"),
              errorMessage: timeoutErrorMessage,
            });
          }, timeoutMs);
          const resetIdleTimer = () => {
            if (!sawAudio) {
              return;
            }
            if (idleTimer) {
              clearTimeout(idleTimer);
            }
            idleTimer = setTimeout(() => {
              finish({
                ok: audioChunks.length > 0,
                status: audioChunks.length > 0 ? 101 : 599,
                contentType: audioMimeType,
                bodyBase64:
                  audioChunks.length > 0 ? encodeBytesToBase64(audioChunks) : null,
                bodyText:
                  audioChunks.length > 0
                    ? JSON.stringify({
                        audioChunkCount: audioChunks.length,
                        mimeType: audioMimeType,
                      })
                    : null,
              });
            }, 1800);
          };

          socket.addEventListener("open", () => {
            if (settled) {
              return;
            }
            pushEvent("open " + requestUrl);
            if (setupFrame) {
              socket.send(JSON.stringify({ setup: setupFrame }));
            } else {
              finish({
                ok: false,
                status: 400,
                errorName: errorName("InvalidRequest"),
                errorMessage: missingSetupMessage,
              });
            }
          });

          socket.addEventListener("message", async (event) => {
            if (settled) {
              return;
            }
            let text;
            if (typeof event.data === "string") {
              text = event.data;
            } else if (event.data && typeof event.data.text === "function") {
              text = await event.data.text();
            } else {
              text = String(event.data);
            }
            if (settled) {
              return;
            }
            pushEvent(text);
            let parsed = null;
            try {
              parsed = JSON.parse(text);
            } catch {
              parsed = null;
            }
            if (!setupComplete && (parsed?.setupComplete || parsed?.setup_complete)) {
              setupComplete = true;
              if (clientContentFrame) {
                socket.send(JSON.stringify({ client_content: clientContentFrame }));
              }
              if (
                musicGenerationConfigFrame &&
                Object.keys(musicGenerationConfigFrame).length > 0
              ) {
                socket.send(
                  JSON.stringify({
                    music_generation_config: musicGenerationConfigFrame,
                  }),
                );
              }
              socket.send(
                JSON.stringify({
                  playback_control: playbackControlFrame,
                }),
              );
              return;
            }
            if (parsed?.filteredPrompt || parsed?.filtered_prompt) {
              finish({
                ok: false,
                status: 400,
                errorName: errorName("FilteredPrompt"),
                errorMessage: String(
                  parsed.filteredPrompt ?? parsed.filtered_prompt ?? "filtered prompt",
                ),
              });
              return;
            }
            const chunks =
              parsed?.serverContent?.audioChunks ??
              parsed?.server_content?.audio_chunks ??
              null;
            if (!Array.isArray(chunks) || !chunks.length) {
              return;
            }
            for (const chunk of chunks) {
              const raw = normalizeString(chunk?.data);
              if (!raw) {
                continue;
              }
              const decodedLength = encodedByteLength(raw);
              if (decodedLength > maxAudioBytes - audioByteLength) {
                const actualLength = audioByteLength + decodedLength;
                // Retrying another URL would retain the same oversized audio again.
                finish({
                  ok: false,
                  status: 413,
                  code: bodyLimitCode,
                  errorName: errorName("BodyTooLarge"),
                  errorMessage:
                    "Gemini Canvas browser binary body exceeded " +
                    maxAudioBytes +
                    " bytes (" +
                    actualLength +
                    " bytes).",
                });
                return;
              }
              const decoded = decodeBase64Chunk(raw);
              sawAudio = true;
              audioMimeType =
                normalizeString(chunk?.mimeType) ??
                normalizeString(chunk?.mime_type) ??
                audioMimeType;
              audioByteLength += decoded.length;
              audioChunks.push(decoded);
            }
            resetIdleTimer();
          });

          socket.addEventListener("error", (event) => {
            finish({
              ok: false,
              status: 599,
              errorName: errorName("WsError"),
              errorMessage:
                event?.message ?? event?.error?.message ?? "music websocket error",
            });
          });

          socket.addEventListener("close", (event) => {
            if (settled) {
              return;
            }
            if (audioChunks.length > 0) {
              finish({
                ok: true,
                status: 101,
                contentType: audioMimeType,
                bodyBase64: encodeBytesToBase64(audioChunks),
                bodyText: JSON.stringify({
                  audioChunkCount: audioChunks.length,
                  mimeType: audioMimeType,
                  closeCode: event.code,
                  closeReason: event.reason || "",
                }),
              });
              return;
            }
            finish({
              ok: false,
              status: 599,
              errorName: errorName("WsClosed"),
              errorMessage:
                "music websocket closed before audio: code=" +
                event.code +
                " reason=" +
                (event.reason || ""),
            });
          });
        });
        if (attemptResult.code === bodyLimitCode) {
          return attemptResult;
        }
        if (attemptResult.ok) {
          return attemptResult;
        }
        lastFailure = attemptResult;
      }
      return lastFailure;
    },
    {
      url: request?.url,
      requestBody,
      timeoutMs,
      errorNamePrefix: config.errorNamePrefix,
      initialErrorMessage: config.initialErrorMessage,
      timeoutErrorMessage: config.timeoutErrorMessage,
      missingSetupMessage: config.missingSetupMessage,
      maxAudioBytes: BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
      bodyLimitCode: BROWSER_POOL_BODY_LIMIT_CODE,
    },
  );
}
