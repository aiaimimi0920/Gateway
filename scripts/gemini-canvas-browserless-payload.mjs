
export function createBrowserlessPayloadOwner({
  normalizeString,
}) {
  function mimeToExt(mimeType) {
    const normalized = String(mimeType || "").toLowerCase();
    if (normalized.includes("png")) {
      return ".png";
    }
    if (normalized.includes("jpeg") || normalized.includes("jpg")) {
      return ".jpg";
    }
    if (normalized.includes("webp")) {
      return ".webp";
    }
    if (normalized.includes("gif")) {
      return ".gif";
    }
    if (normalized.includes("wav")) {
      return ".wav";
    }
    if (normalized.includes("ogg")) {
      return ".ogg";
    }
    if (normalized.includes("mpeg") || normalized.includes("mp3")) {
      return ".mp3";
    }
    if (normalized.includes("mp4")) {
      return ".mp4";
    }
    if (normalized.includes("webm")) {
      return ".webm";
    }
    if (normalized.startsWith("audio/l16") || normalized.startsWith("audio/pcm")) {
      return ".pcm";
    }
    return ".bin";
  }

  function buildTextRequestBody(prompt) {
    return {
      contents: [
        {
          role: "user",
          parts: [{ text: prompt }],
        },
      ],
    };
  }

  function buildTtsRequestBody(prompt, voiceName = null) {
    const body = buildTextRequestBody(prompt);
    body.generationConfig = {
      responseModalities: ["AUDIO"],
    };
    if (normalizeString(voiceName)) {
      body.generationConfig.speechConfig = {
        voiceConfig: {
          prebuiltVoiceConfig: {
            voiceName: voiceName.trim(),
          },
        },
      };
    }
    return body;
  }

  function buildImageRequestBody(prompt, aspectRatio = "1:1") {
    return {
      contents: [
        {
          role: "user",
          parts: [{ text: prompt }],
        },
      ],
      generationConfig: {
        responseModalities: ["IMAGE"],
        imageConfig: {
          aspectRatio,
        },
      },
    };
  }

  function buildVideoCreateRequestBody(prompt, aspectRatio = "16:9", durationSeconds = null) {
    const parameters = {
      aspectRatio,
    };
    if (Number.isFinite(durationSeconds) && durationSeconds > 0) {
      parameters.durationSeconds = durationSeconds;
    }
    return {
      instances: [{ prompt }],
      parameters,
    };
  }

  function extractTextFromGenerateContentResponse(body) {
    const parts = body?.candidates?.[0]?.content?.parts;
    if (!Array.isArray(parts)) {
      return null;
    }
    const texts = parts
      .map((part) => normalizeString(part?.text))
      .filter(Boolean);
    return texts.length ? texts.join("\n") : null;
  }

  function extractAudioFromGenerateContentResponse(body) {
    const parts = body?.candidates?.[0]?.content?.parts;
    if (!Array.isArray(parts)) {
      return null;
    }
    for (const part of parts) {
      const inlineData = part?.inlineData ?? part?.inline_data;
      const mimeType =
        normalizeString(inlineData?.mimeType) ??
        normalizeString(inlineData?.mime_type) ??
        "audio/L16;codec=pcm;rate=24000";
      const raw = normalizeString(inlineData?.data);
      if (!raw) {
        continue;
      }
      return {
        mimeType,
        bytes: Buffer.from(raw, "base64"),
      };
    }
    return null;
  }

  function extractInlineImageFromGenerateContentResponse(body) {
    const candidates = Array.isArray(body?.candidates) ? body.candidates : [];
    for (const candidate of candidates) {
      const parts = candidate?.content?.parts;
      if (!Array.isArray(parts)) {
        continue;
      }
      for (const part of parts) {
        const inlineData = part?.inlineData ?? part?.inline_data;
        const mimeType =
          normalizeString(inlineData?.mimeType) ??
          normalizeString(inlineData?.mime_type) ??
          "image/png";
        const raw = normalizeString(inlineData?.data);
        if (!raw || !mimeType.startsWith("image/")) {
          continue;
        }
        return {
          mimeType,
          bytes: Buffer.from(raw, "base64"),
        };
      }
    }
    return null;
  }

  function extractImagenImages(body) {
    const images = [];
    const collections = [];
    if (Array.isArray(body?.generatedImages)) {
      collections.push(body.generatedImages);
    }
    if (Array.isArray(body?.predictions)) {
      collections.push(body.predictions);
    }
    for (const collection of collections) {
      for (const candidate of collection) {
        const imageRecord = candidate?.image ?? candidate;
        const raw =
          normalizeString(imageRecord?.imageBytes) ??
          normalizeString(imageRecord?.bytesBase64Encoded) ??
          normalizeString(imageRecord?.b64_json) ??
          normalizeString(imageRecord?.b64Json);
        if (!raw) {
          continue;
        }
        const mimeType =
          normalizeString(imageRecord?.mimeType) ??
          normalizeString(imageRecord?.mime_type) ??
          "image/png";
        images.push({
          mimeType,
          bytes: Buffer.from(raw, "base64"),
        });
      }
    }
    return images;
  }

  function parsePcmSampleRate(mimeType) {
    for (const segment of String(mimeType || "").split(";")) {
      const trimmed = segment.trim();
      if (trimmed.startsWith("rate=")) {
        const parsed = Number(trimmed.slice("rate=".length));
        if (Number.isFinite(parsed) && parsed > 0) {
          return parsed;
        }
      }
    }
    return 24_000;
  }

  function parsePcmChannels(mimeType) {
    for (const segment of String(mimeType || "").split(";")) {
      const trimmed = segment.trim();
      if (trimmed.startsWith("channels=")) {
        const parsed = Number(trimmed.slice("channels=".length));
        if (Number.isFinite(parsed) && parsed > 0) {
          return parsed;
        }
      }
    }
    return 1;
  }

  function pcmAudioToWavBytes(rawPcm, mimeType) {
    const sampleRate = parsePcmSampleRate(mimeType);
    const channels = parsePcmChannels(mimeType);
    const bitsPerSample = 16;
    const pcmLe = Buffer.from(rawPcm);
    if (String(mimeType || "").toLowerCase().startsWith("audio/l16")) {
      for (let index = 0; index + 1 < pcmLe.length; index += 2) {
        const first = pcmLe[index];
        pcmLe[index] = pcmLe[index + 1];
        pcmLe[index + 1] = first;
      }
    }

    const byteRate = sampleRate * channels * (bitsPerSample / 8);
    const blockAlign = channels * (bitsPerSample / 8);
    const dataLen = pcmLe.length;
    const riffLen = 36 + dataLen;

    const header = Buffer.alloc(44);
    header.write("RIFF", 0, "ascii");
    header.writeUInt32LE(riffLen, 4);
    header.write("WAVE", 8, "ascii");
    header.write("fmt ", 12, "ascii");
    header.writeUInt32LE(16, 16);
    header.writeUInt16LE(1, 20);
    header.writeUInt16LE(channels, 22);
    header.writeUInt32LE(sampleRate, 24);
    header.writeUInt32LE(byteRate, 28);
    header.writeUInt16LE(blockAlign, 32);
    header.writeUInt16LE(bitsPerSample, 34);
    header.write("data", 36, "ascii");
    header.writeUInt32LE(dataLen, 40);
    return Buffer.concat([header, pcmLe]);
  }

  function extractVideoUriFromOperation(body) {
    return (
      normalizeString(body?.response?.generateVideoResponse?.generatedSamples?.[0]?.video?.uri) ??
      normalizeString(body?.response?.generatedVideos?.[0]?.video?.uri) ??
      normalizeString(body?.response?.generated_videos?.[0]?.video?.uri)
    );
  }

  function summarizeJsonBody(body) {
    if (!body || typeof body !== "object") {
      return null;
    }
    return {
      name: normalizeString(body?.name),
      done: body?.done === true,
      error: body?.error ?? null,
      hasCandidates: Array.isArray(body?.candidates) ? body.candidates.length : 0,
      hasGeneratedImages: Array.isArray(body?.generatedImages) ? body.generatedImages.length : 0,
      hasPredictions: Array.isArray(body?.predictions) ? body.predictions.length : 0,
      videoUri: extractVideoUriFromOperation(body),
    };
  }

  return {
    mimeToExt,
    buildTextRequestBody,
    buildTtsRequestBody,
    buildImageRequestBody,
    buildVideoCreateRequestBody,
    extractTextFromGenerateContentResponse,
    extractAudioFromGenerateContentResponse,
    extractInlineImageFromGenerateContentResponse,
    extractImagenImages,
    parsePcmSampleRate,
    parsePcmChannels,
    pcmAudioToWavBytes,
    extractVideoUriFromOperation,
    summarizeJsonBody,
  };
}
