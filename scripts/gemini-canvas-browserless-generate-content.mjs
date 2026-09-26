import path from "node:path";

export function createBrowserlessGenerateContentOperations({
  normalizeString,
  writeBuffer,
  buildTextRequestBody,
  buildTtsRequestBody,
  buildImageRequestBody,
  extractTextFromGenerateContentResponse,
  extractAudioFromGenerateContentResponse,
  extractInlineImageFromGenerateContentResponse,
  extractImagenImages,
  mimeToExt,
  pcmAudioToWavBytes,
  summarizeJsonBody,
  sendJsonWithAuthAttempts,
  sendExactMinimalApiKeyOnlyJson,
  sendJsonWithCustomAttempts,
  looksLikeQuotaOrPlanGate,
}) {
  async function probeText(context) {
    const requestUrl = `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:generateContent`;
    const requestBody = buildTextRequestBody(context.prompt);
    const sendResult = await sendJsonWithAuthAttempts(
      context,
      "text.generate-content",
      requestUrl,
      requestBody,
      context.timeoutMs,
    );
    const text = extractTextFromGenerateContentResponse(sendResult.responseJson);
    return {
      kind: "official_like_generate_content",
      requestUrl,
      requestBody,
      status: sendResult.response?.status ?? null,
      ok: sendResult.ok,
      responseText: text,
      bodySummary: summarizeJsonBody(sendResult.responseJson),
    };
  }

  async function probeTts(context) {
    const requestUrl = `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:generateContent`;
    const requestBody = buildTtsRequestBody(context.prompt, context.voiceName);
    const sendResult = await sendJsonWithAuthAttempts(
      context,
      "tts.generate-content",
      requestUrl,
      requestBody,
      context.timeoutMs,
    );
    const audio = extractAudioFromGenerateContentResponse(sendResult.responseJson);
    let audioAsset = null;
    if (audio) {
      const rawExt = mimeToExt(audio.mimeType);
      const rawPath = path.join(context.outDir, `tts-audio${rawExt}`);
      await writeBuffer(rawPath, audio.bytes);
      let wavPath = null;
      if (
        String(audio.mimeType).toLowerCase().startsWith("audio/l16") ||
        String(audio.mimeType).toLowerCase().startsWith("audio/pcm")
      ) {
        const wavBytes = pcmAudioToWavBytes(audio.bytes, audio.mimeType);
        wavPath = path.join(context.outDir, "tts-audio.wav");
        await writeBuffer(wavPath, wavBytes);
      }
      audioAsset = {
        mimeType: audio.mimeType,
        bytesLength: audio.bytes.length,
        rawPath,
        wavPath,
      };
    }
    return {
      kind: "official_like_generate_content_tts",
      requestUrl,
      requestBody,
      status: sendResult.response?.status ?? null,
      ok: sendResult.ok,
      bodySummary: summarizeJsonBody(sendResult.responseJson),
      audioAsset,
    };
  }

  async function probeImage(context) {
    const previewBaseUrl = "https://geminiweb-pa.clients6.google.com/v1beta";
    const previewModel = "gemini-2.5-flash-image-preview";
    const officialRequestUrl = `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:generateContent`;
    const previewRequestUrl = `${previewBaseUrl}/models/${previewModel}:generateContent`;
    const shareUrl = normalizeString(context.browserState.shareUrl) ?? context.pageReferer;
    const appUrl =
      normalizeString(context.browserState.canvasProgramUrl) ??
      `${context.baseUrl.replace(/\/+$/, "")}/app`;
    const directHttpTextImageBody = {
      contents: [
        {
          role: "user",
          parts: [{ text: context.prompt }],
        },
      ],
      generationConfig: {
        responseModalities: ["TEXT", "IMAGE"],
        imageConfig: {
          aspectRatio: context.aspectRatio,
        },
      },
    };
    const officialDirectResult = await sendJsonWithAuthAttempts(
      context,
      "image.google-api-official",
      officialRequestUrl,
      directHttpTextImageBody,
      context.timeoutMs,
    );
    const exactOfficialResult = await sendExactMinimalApiKeyOnlyJson(
      context,
      "image.google-api-exact",
      officialRequestUrl,
      directHttpTextImageBody,
      context.timeoutMs,
    );
    if (
      exactOfficialResult.response &&
      looksLikeQuotaOrPlanGate(
        exactOfficialResult.response.status,
        exactOfficialResult.response.text,
      )
    ) {
      return {
        kind: "official_like_generate_content_image",
        requestUrl: exactOfficialResult.request.url,
        requestBody: exactOfficialResult.request.body,
        status: exactOfficialResult.response.status,
        ok: false,
        bodySummary: summarizeJsonBody(exactOfficialResult.responseJson),
        imageAsset: null,
        error: "image_official_gate",
      };
    }
    let imageAsset = extractInlineImageFromGenerateContentResponse(officialDirectResult.responseJson);
    if (!imageAsset) {
      const images = extractImagenImages(officialDirectResult.responseJson);
      imageAsset = images[0] ?? null;
    }
    const officialErrorMessage = normalizeString(officialDirectResult.responseJson?.error?.message);
    const officialGateAccepted =
      (officialDirectResult.response?.status === 429 ||
        officialDirectResult.response?.status === 403 ||
        officialDirectResult.response?.status === 400) &&
      Boolean(officialErrorMessage) &&
      /(quota|paid plans|billing|resource_exhausted|rate limits?)/i.test(officialErrorMessage);
    if (imageAsset || officialGateAccepted) {
      let savedOfficialAsset = null;
      if (imageAsset) {
        const ext = mimeToExt(imageAsset.mimeType);
        const assetPath = path.join(context.outDir, `image-asset${ext}`);
        await writeBuffer(assetPath, imageAsset.bytes);
        savedOfficialAsset = {
          mimeType: imageAsset.mimeType,
          bytesLength: imageAsset.bytes.length,
          assetPath,
        };
      }
      return {
        kind: "official_like_generate_content_image",
        requestUrl: officialDirectResult.request?.url ?? officialRequestUrl,
        requestBody: officialDirectResult.request?.body ?? directHttpTextImageBody,
        status: officialDirectResult.response?.status ?? null,
        ok: Boolean(officialDirectResult.ok && imageAsset),
        bodySummary: summarizeJsonBody(officialDirectResult.responseJson),
        imageAsset: savedOfficialAsset,
        error: imageAsset ? null : "image_official_gate",
      };
    }
    const previewImageOnlyBody = buildImageRequestBody(context.prompt, context.aspectRatio);
    const plainMinimalImageBody = {
      contents: directHttpTextImageBody.contents,
      generationConfig: {
        imageConfig: {
          aspectRatio: context.aspectRatio,
        },
      },
    };
    const imagenPredictBody = {
      instances: [{ prompt: context.prompt }],
      parameters: {
        sampleCount: 1,
        aspectRatio: context.aspectRatio,
      },
    };
    const attempts = [
      {
        label: "clients6_signed_app_text_image",
        kind: "generateContent",
        requestVariant: "preview_text_image",
        requestUrl: previewRequestUrl,
        requestBody: directHttpTextImageBody,
        includeSignedHeaders: true,
        preserveCrossOriginOrigin: true,
        preserveCrossOriginReferer: true,
        signedOriginOverride: context.pageOrigin,
        refererOverride: appUrl,
      },
      {
        label: "clients6_signed_share_text_image",
        kind: "generateContent",
        requestVariant: "preview_text_image",
        requestUrl: previewRequestUrl,
        requestBody: directHttpTextImageBody,
        includeSignedHeaders: true,
        preserveCrossOriginOrigin: true,
        preserveCrossOriginReferer: true,
        signedOriginOverride: context.pageOrigin,
        refererOverride: shareUrl,
      },
      {
        label: "clients6_signed_share_image_only",
        kind: "generateContent",
        requestVariant: "preview_image_only",
        requestUrl: previewRequestUrl,
        requestBody: previewImageOnlyBody,
        includeSignedHeaders: true,
        preserveCrossOriginOrigin: true,
        preserveCrossOriginReferer: true,
        signedOriginOverride: context.pageOrigin,
        refererOverride: shareUrl,
      },
      {
        label: "clients6_plain_share_text_image",
        kind: "generateContent",
        requestVariant: "preview_text_image",
        requestUrl: previewRequestUrl,
        requestBody: directHttpTextImageBody,
        includeSignedHeaders: false,
        preserveCrossOriginOrigin: true,
        preserveCrossOriginReferer: true,
        signedOriginOverride: null,
        refererOverride: shareUrl,
      },
      {
        label: "clients6_plain_minimal_image_only",
        kind: "generateContent",
        requestVariant: "preview_minimal_image_only",
        requestUrl: previewRequestUrl,
        requestBody: plainMinimalImageBody,
        includeSignedHeaders: false,
        preserveCrossOriginOrigin: false,
        preserveCrossOriginReferer: false,
        signedOriginOverride: null,
        refererOverride: null,
      },
      {
        label: "google_api_official",
        kind: "generateContent",
        requestVariant: "official_image_only",
        requestUrl: officialRequestUrl,
        requestBody: buildImageRequestBody(context.prompt, context.aspectRatio),
        includeSignedHeaders: false,
        preserveCrossOriginOrigin: false,
        preserveCrossOriginReferer: true,
        signedOriginOverride: null,
        refererOverride: appUrl,
      },
      {
        label: "google_api_imagen4_predict",
        kind: "imagenPredict",
        requestVariant: "imagen_predict",
        requestUrl: `${context.apiBaseUrl.replace(/\/+$/, "")}/models/imagen-4.0-generate-001:predict`,
        requestBody: imagenPredictBody,
        includeSignedHeaders: false,
        preserveCrossOriginOrigin: false,
        preserveCrossOriginReferer: true,
        signedOriginOverride: null,
        refererOverride: appUrl,
      },
    ];

    let sendResult = null;
    imageAsset = null;
    let responseKind = null;
    for (const attempt of attempts) {
      sendResult = await sendJsonWithCustomAttempts(
        context,
        `image.${attempt.label}`,
        [attempt],
        context.timeoutMs,
      );
      if (!sendResult.ok) {
        continue;
      }
      if (attempt.kind === "generateContent") {
        imageAsset = extractInlineImageFromGenerateContentResponse(sendResult.responseJson);
        responseKind = attempt.kind;
      } else if (attempt.kind === "imagenPredict") {
        const images = extractImagenImages(sendResult.responseJson);
        imageAsset = images[0] ?? null;
        responseKind = attempt.kind;
      }
      if (imageAsset) {
        break;
      }
    }

    let savedAsset = null;
    if (imageAsset) {
      const ext = mimeToExt(imageAsset.mimeType);
      const assetPath = path.join(context.outDir, `image-asset${ext}`);
      await writeBuffer(assetPath, imageAsset.bytes);
      savedAsset = {
        mimeType: imageAsset.mimeType,
        bytesLength: imageAsset.bytes.length,
        assetPath,
      };
    }
    return {
      kind: responseKind === "imagenPredict" ? "imagen_predict_image" : "official_like_generate_content_image",
      requestUrl: sendResult?.request?.url ?? officialRequestUrl,
      requestBody: sendResult?.request?.body ?? buildImageRequestBody(context.prompt, context.aspectRatio),
      status: sendResult.response?.status ?? null,
      ok: Boolean(sendResult?.ok && imageAsset),
      bodySummary: summarizeJsonBody(sendResult.responseJson),
      imageAsset: savedAsset,
    };
  }

  return {
    probeText,
    probeTts,
    probeImage,
  };
}
