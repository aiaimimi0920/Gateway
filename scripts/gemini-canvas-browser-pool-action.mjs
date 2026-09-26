import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function mergeActionContract(target, incoming) {
  if (!target || !incoming) {
    return;
  }
  if (!target.canvasProgramAction && incoming.canvasProgramAction) {
    target.canvasProgramAction = incoming.canvasProgramAction;
  }
  if (!target.canvasProgramActionInput && incoming.canvasProgramActionInput) {
    target.canvasProgramActionInput = incoming.canvasProgramActionInput;
  }
}

export function extractCanvasProgramActionContractFromText(rawText) {
  const text = normalizeString(rawText);
  if (!text) {
    return {
      canvasProgramAction: null,
      canvasProgramActionInput: null,
    };
  }
  const actionMatch = text.match(/"action"\s*:\s*"([^"\r\n]+)"/i);
  let actionInput = null;
  const actionInputMarker = '"action_input"';
  const actionInputIndex = text.indexOf(actionInputMarker);
  if (actionInputIndex >= 0) {
    const afterMarker = text.slice(actionInputIndex + actionInputMarker.length);
    const colonIndex = afterMarker.indexOf(":");
    if (colonIndex >= 0) {
      const line = afterMarker
        .slice(colonIndex + 1)
        .split(/\r?\n/, 1)[0]
        .trim()
        .replace(/,$/, "")
        .trim();
      if (line) {
        const normalizedLine =
          line.startsWith('"') && line.endsWith('"') && line.length >= 2
            ? line.slice(1, -1)
            : line;
        actionInput = normalizedLine.trim() || null;
      }
    }
  }
  return {
    canvasProgramAction: actionMatch?.[1]?.trim() || null,
    canvasProgramActionInput: actionInput,
  };
}

export function extractQuotedScalar(rawText, fieldNames) {
  const text = normalizeString(rawText)
    ?.replace(/\\"/g, "\"")
    ?.replace(/\\'/g, "'");
  if (!text) {
    return null;
  }
  for (const fieldName of fieldNames) {
    const regex = new RegExp(`[\"']${fieldName}[\"']\\s*:\\s*[\"']([^\"'\\r\\n]+)[\"']`, "i");
    const match = text.match(regex);
    if (match?.[1]?.trim()) {
      return match[1].trim();
    }
  }
  return null;
}

export function extractNumberScalar(rawText, fieldNames) {
  const text = normalizeString(rawText);
  if (!text) {
    return null;
  }
  for (const fieldName of fieldNames) {
    const regex = new RegExp(`[\"']${fieldName}[\"']\\s*:\\s*(\\d+(?:\\.\\d+)?)`, "i");
    const match = text.match(regex);
    if (match?.[1]) {
      const parsed = Number(match[1]);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return null;
}

export function extractDurationSecondsFromBodyText(rawText) {
  const text = normalizeString(rawText);
  if (!text) {
    return null;
  }
  const match = text.match(/(\d+):(\d{2})\s*\/\s*(\d+):(\d{2})/);
  if (!match) {
    return null;
  }
  const minutes = Number(match[3]);
  const seconds = Number(match[4]);
  if (!Number.isFinite(minutes) || !Number.isFinite(seconds)) {
    return null;
  }
  return minutes * 60 + seconds;
}

export function inferInvokeUiState(operation, snapshot) {
  const bodyText = normalizeString(snapshot?.bodyText) ?? "";
  const controls = [
    ...(snapshot?.buttons || []).flatMap((entry) => [entry.text, entry.ariaLabel, entry.title]),
  ]
    .filter(Boolean)
    .map((value) => String(value).trim());
  if (operation === "music") {
    if (/Generating your music/i.test(bodyText)) {
      return "music_generating";
    }
    if (
      controls.some((value) => value.includes("下载音乐作品")) &&
      /0:\d{2}\s*\/\s*0:\d{2}/.test(bodyText)
    ) {
      return "music_player_ready";
    }
  }
  if (operation === "video") {
    if (/Generating your video/i.test(bodyText)) {
      return "video_generating";
    }
    if (
      controls.some((value) =>
        /播放视频|下载视频|Play video|Download video/i.test(value),
      )
      || /Your video is ready|视频已准备好|视频已生成/i.test(bodyText)
    ) {
      return "video_player_ready";
    }
  }
  if (controls.some((value) => value.includes("不使用应用，再试一次"))) {
    return "retry_without_app_visible";
  }
  return null;
}
