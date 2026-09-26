export function createBrowserlessInvocationOptions({
  normalizeString,
  DEFAULT_BASE_URL,
  DEFAULT_API_BASE_URL,
  DEFAULT_TEXT_MODEL,
  DEFAULT_TTS_MODEL,
  DEFAULT_IMAGE_MODEL,
  DEFAULT_MUSIC_MODEL,
  DEFAULT_VIDEO_MODEL,
  DEFAULT_LOCALE,
  DEFAULT_TIMEOUT_MS,
  DEFAULT_VIDEO_POLL_TIMEOUT_MS,
}) {
  function normalizeOperation(rawOperation) {
    const operation = String(rawOperation ?? "")
      .trim()
      .toLowerCase();
    switch (operation) {
      case "text":
      case "chat":
        return "text";
      case "tts":
      case "audio-speech":
      case "speech":
        return "tts";
      case "image":
      case "image-create":
      case "images":
        return "image";
      case "music":
      case "music-create":
      case "audio":
        return "music";
      case "video":
      case "video-create":
      case "videos":
        return "video-create";
      default:
        return operation || "text";
    }
  }

  function defaultPromptForOperation(operation) {
    const marker = Date.now();
    switch (operation) {
      case "tts":
        return `BROWSERLESS_CANVAS_TTS_${marker} Read this sentence naturally: browserless Gemini Canvas TTS probe ok.`;
      case "image":
        return `BROWSERLESS_CANVAS_IMAGE_${marker} A high-contrast industrial terminal badge that says BROWSERLESS OK.`;
      case "music":
        return `BROWSERLESS_CANVAS_MUSIC_${marker} A short electronic cue with a clear pulse.`;
      case "video-create":
        return "A four second shot of a red cube slowly rotating on a white table.";
      case "text":
      default:
        return `BROWSERLESS_CANVAS_TEXT_${marker} Reply with exactly: ok`;
    }
  }

  function defaultModelForOperation(operation) {
    switch (operation) {
      case "tts":
        return DEFAULT_TTS_MODEL;
      case "image":
        return DEFAULT_IMAGE_MODEL;
      case "music":
        return DEFAULT_MUSIC_MODEL;
      case "video-create":
        return DEFAULT_VIDEO_MODEL;
      case "text":
      default:
        return DEFAULT_TEXT_MODEL;
    }
  }

  function inferBaseUrl(browserState) {
    return (
      normalizeString(browserState?.baseUrl) ??
      normalizeString(browserState?.base_url) ??
      originFromUrl(browserState?.shareUrl) ??
      originFromUrl(browserState?.canvasProgramUrl) ??
      originFromUrl(browserState?.pageUrl) ??
      DEFAULT_BASE_URL
    );
  }

  function inferApiBaseUrl(browserState, args) {
    return (
      normalizeString(args["api-base-url"]) ??
      normalizeString(browserState?.apiBaseUrl) ??
      normalizeString(browserState?.api_base_url) ??
      DEFAULT_API_BASE_URL
    );
  }

  function originFromUrl(rawUrl) {
    const normalized = normalizeString(rawUrl);
    if (!normalized) {
      return null;
    }
    try {
      return new URL(normalized).origin;
    } catch {
      return null;
    }
  }

  function resolveInvocationOptions(browserState, programHandle, args) {
    const baseUrl = normalizeString(args["base-url"]) ?? inferBaseUrl(browserState);
    const apiBaseUrl = inferApiBaseUrl(browserState, args);
    const pageReferer =
      normalizeString(args["page-referer"]) ??
      normalizeString(browserState.canvasProgramUrl) ??
      normalizeString(browserState.pageUrl) ??
      normalizeString(browserState.shareUrl) ??
      `${baseUrl.replace(/\/+$/, "")}/canvas`;
    const pageOrigin = originFromUrl(pageReferer) ?? originFromUrl(baseUrl) ?? DEFAULT_BASE_URL;
    const locale =
      normalizeString(args.locale) ??
      normalizeString(programHandle?.json?.bootstrap?.language) ??
      DEFAULT_LOCALE;
    const materialOperation = normalizeOperation(
      normalizeString(browserState?.canvasProgramInvokeContract?.operation),
    );
    const operation = normalizeOperation(
      normalizeString(args.operation) ??
        materialOperation ??
        "text",
    );
    const materialPrompt =
      materialOperation === operation
        ? normalizeString(browserState?.canvasProgramInvokeContract?.prompt)
        : null;
    const prompt =
      normalizeString(args.prompt) ??
      materialPrompt ??
      defaultPromptForOperation(operation);
    const model = normalizeString(args.model) ?? defaultModelForOperation(operation);
    const voiceName = normalizeString(args.voice);
    const aspectRatio =
      normalizeString(args["aspect-ratio"]) ??
      normalizeString(browserState?.canvasProgramInvokeContract?.aspectRatio) ??
      normalizeString(browserState?.canvasProgramInvokeContract?.aspect_ratio) ??
      (operation === "video-create" ? "16:9" : "1:1");
    const durationSeconds =
      Number.parseFloat(
        args["duration-seconds"] ??
          browserState?.canvasProgramInvokeContract?.durationSeconds ??
          browserState?.canvasProgramInvokeContract?.duration_seconds ??
          (operation === "video-create" ? "4" : ""),
      ) || null;
    const apiKey =
      normalizeString(args["api-key"]) ??
      normalizeString(browserState.googleApiKey) ??
      (Array.isArray(browserState.apiKeys)
        ? normalizeString(browserState.apiKeys.find((value) => normalizeString(value)))
        : null);
    const timeoutMs = Math.max(Number(args["timeout-ms"] || DEFAULT_TIMEOUT_MS), 5_000);
    const videoPollTimeoutMs = Math.max(
      Number(args["video-poll-timeout-ms"] || DEFAULT_VIDEO_POLL_TIMEOUT_MS),
      timeoutMs,
    );

    return {
      baseUrl,
      apiBaseUrl,
      pageReferer,
      pageOrigin,
      locale,
      operation,
      prompt,
      model,
      voiceName,
      aspectRatio,
      durationSeconds,
      apiKey,
      timeoutMs,
      videoPollTimeoutMs,
    };
  }

  return {
    normalizeOperation,
    defaultPromptForOperation,
    defaultModelForOperation,
    inferBaseUrl,
    inferApiBaseUrl,
    originFromUrl,
    resolveInvocationOptions,
  };
}
