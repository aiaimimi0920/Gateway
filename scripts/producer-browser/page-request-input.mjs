const defaultVideoPrompt = "Create a cinematic music video for this song.";

export const normalizeText = (value) =>
  typeof value === "string" && value.trim() ? value.trim() : null;

export const readStringFields = (value, fields) => {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const field of fields) {
    const direct = normalizeText(value[field]);
    if (direct) {
      return direct;
    }
    const nested =
      value.args && typeof value.args === "object"
        ? normalizeText(value.args[field])
        : null;
    if (nested) {
      return nested;
    }
  }
  return null;
};

export const readNumberFields = (value, fields) => {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const field of fields) {
    const direct = value[field];
    if (typeof direct === "number" && Number.isFinite(direct)) {
      return direct;
    }
    if (typeof direct === "string" && direct.trim()) {
      const parsed = Number(direct);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
    const nested = value.args && typeof value.args === "object" ? value.args[field] : null;
    if (typeof nested === "number" && Number.isFinite(nested)) {
      return nested;
    }
    if (typeof nested === "string" && nested.trim()) {
      const parsed = Number(nested);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return null;
};

export const buildClientContext = (value, clipId, modelName) => {
  const provided = value?.client_context;
  if (provided && typeof provided === "object" && !Array.isArray(provided)) {
    return {
      current_song_id: clipId,
      song_queue: Array.isArray(provided.song_queue) ? provided.song_queue : [{ id: clipId }],
      selected_model:
        typeof provided.selected_model === "string" && provided.selected_model.trim()
          ? provided.selected_model.trim()
          : modelName,
      lyrics_id_map:
        provided.lyrics_id_map && typeof provided.lyrics_id_map === "object"
          ? provided.lyrics_id_map
          : {},
      ghostwriter_version:
        normalizeText(provided.ghostwriter_version) ?? "standard",
      ...provided,
    };
  }
  return {
    current_song_id: clipId,
    song_queue: [{ id: clipId }],
    selected_model: modelName,
    lyrics_id_map: {},
    ghostwriter_version: "standard",
  };
};

export const buildCreativePrompt = (value) => {
  const lines = [];
  const prompt =
    readStringFields(value, ["user_message", "userMessage", "prompt", "input"]) ??
    defaultVideoPrompt;
  lines.push(prompt);

  const styleImageUrl = readStringFields(value, [
    "style_image_url",
    "styleImageUrl",
    "style_reference_image_url",
    "styleReferenceImageUrl",
  ]);
  if (styleImageUrl) {
    lines.push(`Use this style reference image: ${styleImageUrl}`);
  }

  const likenessImageUrl = readStringFields(value, [
    "likeness_image_url",
    "likenessImageUrl",
    "subject_image_url",
    "subjectImageUrl",
  ]);
  if (likenessImageUrl) {
    lines.push(`Preserve the subject likeness from this image: ${likenessImageUrl}`);
  }

  const aspectRatio = readStringFields(value, ["aspect_ratio", "aspectRatio", "size"]);
  if (aspectRatio) {
    lines.push(`Target aspect ratio: ${aspectRatio}.`);
  }

  const resolution = readStringFields(value, ["resolution"]);
  if (resolution) {
    lines.push(`Target resolution: ${resolution}.`);
  }

  const renderLyrics = value?.render_lyrics ?? value?.renderLyrics ?? value?.display_lyrics;
  if (typeof renderLyrics === "boolean") {
    lines.push(renderLyrics ? "Render lyrics on screen." : "Do not render lyrics on screen.");
  }

  const durationSeconds = readNumberFields(value, [
    "duration_s",
    "durationSeconds",
    "duration",
  ]);
  if (durationSeconds) {
    lines.push(`Keep the cut close to ${durationSeconds} seconds.`);
  }

  return lines.join("\n");
};

export const findToolReturn = (toolReturns, toolName) =>
  Array.isArray(toolReturns)
    ? toolReturns.find((entry) => entry?.toolName === toolName) ?? null
    : null;

export const readToolJobId = (toolReturn) => {
  if (!toolReturn || typeof toolReturn !== "object" || !toolReturn.content) {
    return null;
  }
  return normalizeText(toolReturn.content.job_id ?? toolReturn.content.jobId);
};
