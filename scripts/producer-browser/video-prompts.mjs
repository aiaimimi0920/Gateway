import { normalizeString, readStringFields, readNumberFields } from "./request-fields.mjs";

const DEFAULT_CONFIRM_PROMPT = "Create the video";

export function buildProducerProposalPrompt(requestBody, creativePrompt) {
  const aspectRatio =
    readStringFields(requestBody, ["aspect_ratio", "aspectRatio", "size"]) ?? "16:9";
  const styleImageUrl = readStringFields(requestBody, [
    "style_image_url",
    "styleImageUrl",
    "style_reference_image_url",
    "styleReferenceImageUrl",
  ]);
  const subjectImageUrl = readStringFields(requestBody, [
    "likeness_image_url",
    "likenessImageUrl",
    "subject_image_url",
    "subjectImageUrl",
  ]);
  const durationSeconds =
    readNumberFields(requestBody, ["duration_s", "durationSeconds", "duration"]) ?? null;
  const renderLyrics =
    typeof requestBody?.render_lyrics === "boolean"
      ? requestBody.render_lyrics
      : typeof requestBody?.renderLyrics === "boolean"
        ? requestBody.renderLyrics
        : typeof requestBody?.display_lyrics === "boolean"
          ? requestBody.display_lyrics
          : typeof requestBody?.displayLyrics === "boolean"
            ? requestBody.displayLyrics
            : false;

  return [
    "Please propose the music video.",
    `Vision: ${creativePrompt}`,
    `Use ${aspectRatio}.`,
    styleImageUrl
      ? `Style reference image: ${styleImageUrl}.`
      : "Style reference image: none.",
    subjectImageUrl
      ? `Subject image: ${subjectImageUrl}.`
      : "Subject image: none, generate one.",
    `Lyrics on screen: ${renderLyrics ? "yes" : "no"}.`,
    durationSeconds
      ? `Duration: use about ${durationSeconds} seconds.`
      : "Duration: choose the strongest section under 60 seconds.",
  ].join(" ");
}

export function chooseProducerConfirmPrompt(requestBody, suggestions, messageTexts, proposedInputs = null) {
  const explicit =
    normalizeString(requestBody?.confirm_prompt) ?? normalizeString(requestBody?.confirmPrompt);
  if (explicit) {
    return explicit;
  }

  const proposedStartSeconds =
    readNumberFields(proposedInputs, ["start_s", "startSeconds", "start"]) ??
    readNumberFields(requestBody, ["start_s", "startSeconds", "start"]);
  const proposedDurationSeconds =
    readNumberFields(proposedInputs, ["duration_s", "durationSeconds", "duration"]) ??
    readNumberFields(requestBody, ["duration_s", "durationSeconds", "duration"]);
  const proposedAspectRatio =
    readStringFields(proposedInputs, ["aspect_ratio", "aspectRatio", "size"]) ??
    readStringFields(requestBody, ["aspect_ratio", "aspectRatio", "size"]);
  const proposedResolution =
    readStringFields(proposedInputs, ["resolution"]) ??
    readStringFields(requestBody, ["resolution"]);

  if (proposedStartSeconds != null || proposedDurationSeconds != null || proposedAspectRatio) {
    return [
      "Create this exact proposed music video now.",
      proposedStartSeconds != null ? `Keep the current start time at ${proposedStartSeconds}s.` : null,
      proposedDurationSeconds != null ? `Keep the duration at ${proposedDurationSeconds}s.` : null,
      proposedAspectRatio ? `Keep the aspect ratio at ${proposedAspectRatio}.` : null,
      proposedResolution ? `Keep the resolution at ${proposedResolution}.` : null,
      "Do not ask follow-up questions or change the selected song section.",
    ]
      .filter(Boolean)
      .join(" ");
  }

  const candidateTexts = [
    ...suggestions,
    ...(Array.isArray(messageTexts) ? messageTexts : []),
  ];
  const preferred =
    candidateTexts.find((entry) => /create( the)? (music )?video/i.test(entry)) ??
    candidateTexts.find((entry) => /render/i.test(entry)) ??
    candidateTexts.find((entry) => /create/i.test(entry)) ??
    candidateTexts.find((entry) => /start/i.test(entry));
  return preferred ?? DEFAULT_CONFIRM_PROMPT;
}
