import path from "node:path";

export function createBrowserlessMusicOperation({
  DEFAULT_MUSIC_WS_URL,
  normalizeString,
  writeJson,
  writeBuffer,
  mimeToExt,
  pcmAudioToWavBytes,
  textPreview,
}) {
  function normalizeRemoteMusicWsUrl(rawUrl) {
    const normalized = normalizeString(rawUrl);
    if (!normalized) {
      return null;
    }
    try {
      const parsed = new URL(normalized);
      const host = parsed.hostname.toLowerCase();
      if (host === "127.0.0.1" || host === "localhost" || host === "::1") {
        return null;
      }
      return parsed.toString();
    } catch {
      return null;
    }
  }

  async function probeMusic(context) {
    const browserStateWsUrl =
      normalizeRemoteMusicWsUrl(context.browserState?.musicWsUrl) ??
      normalizeRemoteMusicWsUrl(context.browserState?.canvasProgramInvokeContract?.musicWsUrl) ??
      null;
    const requestUrl =
      normalizeString(context.args?.["music-ws-url"]) ??
      browserStateWsUrl ??
      `${DEFAULT_MUSIC_WS_URL}?key=${encodeURIComponent(context.apiKey)}`;

    const requestBody = {
      setup: { model: `models/${context.model}` },
      client_content: {
        weightedPrompts: [{ text: context.prompt, weight: 1.0 }],
      },
      playback_control: "PLAY",
      note: "Browserless music probe uses only key query and does not send authuser query or durationSeconds music_generation_config.",
    };

    await writeJson(path.join(context.outDir, "music.ws.request.json"), {
      requestUrl,
      requestBody,
    });

    const result = await new Promise((resolve) => {
      const events = [];
      const ws = new WebSocket(requestUrl);
      let settled = false;
      let receivingAudio = false;
      const listeners = [];
      const timeout = setTimeout(() => {
        settle({
          ok: false,
          error: "music_ws_timeout",
          events,
        });
      }, Math.min(context.timeoutMs, 60_000));
      let setupComplete = false;
      let audioMimeType = "audio/l16;rate=48000;channels=2";
      const chunks = [];

      const settle = (value) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        // Detach before close so terminal transport events cannot re-enter cleanup.
        for (const [type, listener] of listeners) {
          try { ws.removeEventListener(type, listener); } catch {}
        }
        resolve(value);
        try {
          ws.close();
        } catch {}
      };
      const finish = (value) => settle({ setupComplete, events, ...value });
      const fail = (error) => finish({
        ok: false,
        error: "music_ws_error",
        message: error instanceof Error ? error.message : String(error),
      });
      const listen = (type, handler) => {
        const listener = (event) => {
          if (settled) return;
          try {
            Promise.resolve(handler(event)).catch(fail);
          } catch (error) {
            fail(error);
          }
        };
        listeners.push([type, listener]);
        ws.addEventListener(type, listener);
      };

      listen("open", () => {
        ws.send(JSON.stringify({ setup: requestBody.setup }));
      });

      listen("message", async (event) => {
        if (receivingAudio) return;
        let text;
        if (typeof event.data === "string") {
          text = event.data;
        } else if (event.data && typeof event.data.text === "function") {
          text = await event.data.text();
        } else {
          text = String(event.data);
        }
        if (settled || receivingAudio) return;
        events.push(textPreview(text, 1200));

        let parsed = null;
        try {
          parsed = JSON.parse(text);
        } catch {
          parsed = null;
        }
        if (!setupComplete && (parsed?.setupComplete || parsed?.setup_complete)) {
          setupComplete = true;
          ws.send(JSON.stringify({ client_content: requestBody.client_content }));
          ws.send(JSON.stringify({ playback_control: requestBody.playback_control }));
        }

        const audioChunks =
          parsed?.serverContent?.audioChunks ?? parsed?.server_content?.audio_chunks ?? null;
        if (!Array.isArray(audioChunks) || !audioChunks.length) {
          return;
        }
        for (const chunk of audioChunks) {
          const raw = normalizeString(chunk?.data);
          if (!raw) {
            continue;
          }
          chunks.push(Buffer.from(raw, "base64"));
          audioMimeType =
            normalizeString(chunk?.mimeType) ??
            normalizeString(chunk?.mime_type) ??
            audioMimeType;
        }
        if (!chunks.length) {
          return;
        }
        // Claim one payload before writes yield to another audio callback.
        receivingAudio = true;
        const pcm = Buffer.concat(chunks);
        const rawExt = mimeToExt(audioMimeType) || ".bin";
        const rawPath = path.join(context.outDir, `music-audio${rawExt}`);
        await writeBuffer(rawPath, pcm);
        // An issued write cannot be undone; do not start more work after settlement.
        if (settled) return;
        let wavPath = null;
        if (String(audioMimeType).toLowerCase().startsWith("audio/l16")) {
          const wavBytes = pcmAudioToWavBytes(pcm, audioMimeType);
          wavPath = path.join(context.outDir, "music-audio.wav");
          await writeBuffer(wavPath, wavBytes);
        }
        if (settled) return;
        finish({
          ok: true,
          audioAsset: {
            mimeType: audioMimeType,
            bytesLength: pcm.length,
            rawPath,
            wavPath,
          },
          bodySummary: {
            audioChunkCount: audioChunks.length,
            audioBytes: pcm.length,
            mimeType: audioMimeType,
          },
        });
      });

      listen("error", (event) => {
        finish({
          ok: false,
          error: "music_ws_error",
          message: event?.message ?? String(event?.error ?? event),
        });
      });

      listen("close", (event) => {
        if (chunks.length) {
          return;
        }
        finish({
          ok: false,
          error: "music_ws_closed_without_audio",
          message: `close code=${event.code} reason=${event.reason || ""}`.trim(),
        });
      });
    });

    await writeJson(path.join(context.outDir, "music.ws.response.json"), result);
    return {
      kind: "official_music_ws_browserless",
      requestUrl,
      requestBody,
      status: result.ok ? 101 : null,
      ok: result.ok,
      bodySummary: result.bodySummary ?? null,
      audioAsset: result.audioAsset ?? null,
      error: result.error ?? null,
      responseText: result.message ?? null,
    };
  }

  return {
    probeMusic,
    normalizeRemoteMusicWsUrl,
  };
}
