function bodyIndicatesMusicAcceptedProgress(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  const normalized = text.toLowerCase();
  return (
    (text.includes("music_generation") && text.includes("action_input")) ||
    normalized.includes("track details") ||
    normalized.includes("i've put together a 30-second electronic cue") ||
    normalized.includes("i’ve put together a 30-second electronic cue") ||
    normalized.includes("electronic cue for you")
  );
}

function bodyIndicatesMusicPendingOrBusy(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  const normalized = text.toLowerCase();
  return (
    bodyIndicatesMusicAcceptedProgress(text)
    || normalized.includes("i've hit a bit of a snag")
    || normalized.includes("i’ve hit a bit of a snag")
    || normalized.includes("please try again later")
    || normalized.includes("getting a lot of requests right now")
  );
}

function bodyIndicatesVideoAcceptedProgress(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  const normalized = text.toLowerCase();
  return (
    text.includes("video_placeholder") ||
    normalized.includes("getting a lot of requests right now") ||
    normalized.includes("please try again later")
  );
}

function invokeContractIsProxyOnlyCandidate(invokeContract) {
  return (
    invokeContract?.transportKind === "canvas_program_ws_candidate" &&
    invokeContract?.requestEnvelopeKind === "canvas_proxy_request"
  );
}

function invokeContractIndicatesConcreteProgress(operation, invokeContract, snapshot) {
  if (!invokeContract) {
    return false;
  }
  if (
    invokeContract.uiState === "music_player_ready" ||
    invokeContract.uiState === "video_player_ready"
  ) {
    return true;
  }
  if (invokeContract.target && !invokeContractIsProxyOnlyCandidate(invokeContract)) {
    return true;
  }
  if (operation === "music") {
    return (
      invokeContract.actionName === "music_generation" ||
      bodyIndicatesMusicAcceptedProgress(snapshot?.bodyText)
    );
  }
  if (operation === "video") {
    return bodyIndicatesVideoAcceptedProgress(snapshot?.bodyText);
  }
  return false;
}

function detectMediaProviderGate(operation, bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return null;
  }
  if (
    operation === "image" &&
    (
      text.includes("Image Creation Not Available") ||
      /can't create it right now/i.test(text) ||
      /can't seem to create any/i.test(text) ||
      /can't create any for you/i.test(text) ||
      (/search for images/i.test(text) && /can't create/i.test(text)) ||
      /image creation isn't available/i.test(text) ||
      text.includes("您登录了吗") ||
      text.includes("似乎无法为您创建任何图片") ||
      text.includes("所在的地区尚未开通图片创建功能")
    )
  ) {
    return {
      status: 503,
      code: "gemini_canvas_image_generation_unavailable",
      message: "Gemini Canvas image generation is unavailable for the current browser session or location.",
    };
  }
  if (
    operation === "video" &&
    (
      /出了点问题\s*\(13\)/.test(text) ||
      /出了点问题\s*\(1099\)/.test(text) ||
      /something went wrong\s*\(13\)/i.test(text) ||
      /something went wrong\s*\(1155\)/i.test(text)
    )
  ) {
    return {
      status: /1155/.test(text) ? 502 : 409,
      code: /1155/.test(text)
        ? "gemini_canvas_video_generation_transient_failure"
        : "gemini_canvas_video_mode_unavailable",
      message: /1155/.test(text)
        ? "Gemini Canvas video generation returned transient error 1155 after retries."
        : "Gemini Canvas video mode could not be activated.",
    };
  }
  if (
    operation === "video" &&
    (
      text.includes("已达到视频生成数量上限") ||
      text.includes("视频生成数量上限") ||
      /out of videos for now/i.test(text) ||
      /videos will be available again/i.test(text) ||
      /出了点问题\s*\(1053\)/.test(text) ||
      text.includes("1053")
    )
  ) {
    return {
      status: 429,
      code: "gemini_canvas_video_quota_reached",
      message: "Gemini Canvas video generation quota is currently exhausted for this account.",
    };
  }
  return null;
}

function shouldBlockMediaProviderGate(operation, providerGate, media, invokeContract, bodyText) {
  if (!providerGate) {
    return false;
  }
  if (operation !== "video") {
    return true;
  }
  const readySurface =
    media.length > 0
    || invokeContract?.uiState === "video_player_ready"
    || /Your video is ready|视频已准备好|视频已生成/i.test(String(bodyText || ""));
  return !readySurface;
}

function recentMediaProviderGateText(captureState) {
  if (!captureState || !Array.isArray(captureState.events)) {
    return "";
  }
  const texts = [];
  for (const event of captureState.events.slice(-8)) {
    if (event?.type !== "response" || typeof event.text !== "string" || !event.text.trim()) {
      continue;
    }
    texts.push(event.text);
  }
  return texts.join("\n");
}

const IMAGE_COMPOSER_BOOTSTRAP_RPC_IDS = new Set([
  "MyzX6c",
  "aPya6c",
  "L5adhe",
  "XhaU0b",
  "V8rlHe",
]);

function extractRpcIdsFromUrl(url) {
  const value = String(url || "");
  if (!value) {
    return [];
  }
  const parsed = value.match(/[?&]rpcids=([^&]+)/i)?.[1] ?? "";
  if (!parsed) {
    return [];
  }
  return parsed.split(",").map((entry) => entry.trim()).filter(Boolean);
}

function shouldRetryMediaPromptSubmission(operation, diagnostics = {}) {
  const bodyText = String(diagnostics.bodyText || "");
  if (operation === "video") {
    return (
      /Create videos|Try a template or describe a video in chat|创作视频|描述视频/i.test(bodyText)
      && !/You said|Gemini is typing|Generating your video|正在生成|Gemini replied/i.test(bodyText)
    );
  }
  if (operation !== "image") {
    return false;
  }
  const prompt = String(diagnostics.prompt || "").trim();
  const events = Array.isArray(diagnostics.events) ? diagnostics.events : [];
  if (!prompt || !bodyText) {
    return false;
  }
  const normalizedPrompt = prompt.replace(/\s+/g, " ").trim();
  const normalizedBodyText = bodyText.replace(/\s+/g, " ").trim();
  const promptAnchor = normalizedPrompt.split(" ").slice(0, 8).join(" ");
  const stillAtComposer =
    /Create images|Create with Nano Banana/i.test(bodyText)
    && /\bSubmit\b|提交/.test(bodyText)
    && (normalizedBodyText.includes(normalizedPrompt) || (promptAnchor && normalizedBodyText.includes(promptAnchor)))
    && !/You said|Gemini is typing|Creating|生成中|正在创建/i.test(bodyText);
  if (!stillAtComposer) {
    return false;
  }
  const recentEvents = events.slice(-8).filter((event) => /source-path=%2Fimages/i.test(String(event?.url || "")));
  if (recentEvents.length === 0) {
    return true;
  }
  return recentEvents.every((event) => {
    const rpcIds = extractRpcIdsFromUrl(event?.url);
    return rpcIds.length > 0 && rpcIds.every((rpcId) => IMAGE_COMPOSER_BOOTSTRAP_RPC_IDS.has(rpcId));
  });
}

export {
  bodyIndicatesMusicPendingOrBusy,
  invokeContractIndicatesConcreteProgress,
  detectMediaProviderGate,
  shouldBlockMediaProviderGate,
  recentMediaProviderGateText,
  shouldRetryMediaPromptSubmission,
};
